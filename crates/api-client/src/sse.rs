use crate::contract::{EventVisibility, RunEvent, RunEventType};
use crate::error::{ApiErrorBody, ControlPlaneError};
use crate::http::urlencode;
use futures_util::StreamExt;
use reqwest::StatusCode;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A single parsed SSE frame. `data` is the joined `data:` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct SseFrame {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: String,
}

/// Parse one SSE frame (a block of lines ending in a blank line). Comment
/// lines (`: ...`) are skipped; frames without a `data:` field are ignored
/// (heartbeats). Mirrors `parseSseFrame` in apps/mobile/src/api/run-events.ts.
pub fn parse_sse_frame(raw: &str) -> Option<SseFrame> {
    let mut id = None;
    let mut event = None;
    let mut data = Vec::new();
    for raw_line in raw.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "id" if !value.contains('\0') => id = Some(value.to_string()),
            "event" => event = Some(value.to_string()),
            "data" => data.push(value.to_string()),
            _ => {}
        }
    }
    if data.is_empty() {
        return None;
    }
    Some(SseFrame {
        id,
        event,
        data: data.join("\n"),
    })
}

/// Terminal event types. Once one of these is accepted, the run is over and
/// the mobile/TUI stops consuming further events (same policy as the mobile's
/// TERMINAL_TYPES set).
pub const TERMINAL_EVENT_TYPES: &[RunEventType] = &[
    RunEventType::RunCompleted,
    RunEventType::RunPartial,
    RunEventType::RunFailed,
    RunEventType::RunCancelled,
];

pub fn is_terminal_event(event: &RunEvent) -> bool {
    TERMINAL_EVENT_TYPES.contains(&event.event_type)
}

impl RunEvent {
    /// Validate a candidate parsed from an SSE frame. Returns None when the
    /// event is malformed, belongs to a different run, or the frame's
    /// `event:` name disagrees with the JSON `type` — the stream skips it.
    /// Mirrors `eventFromFrame` + the zod `RunEvent` schema.
    pub fn from_sse_frame(frame: &SseFrame, expected_run_id: &str) -> Option<RunEvent> {
        let id = frame.id.as_deref()?;
        let event_name = frame.event.as_deref()?;
        if !id.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let sequence: u64 = id.parse().ok()?;
        let data: serde_json::Value = serde_json::from_str(&frame.data).ok()?;
        let event = Self::validate(data, expected_run_id, sequence)?;
        if event.event_type.as_str() != event_name {
            return None;
        }
        Some(event)
    }

    /// Validate a candidate event object against the wire contract.
    pub fn validate(value: serde_json::Value, expected_run_id: &str, expected_sequence: u64) -> Option<RunEvent> {
        let event: RunEvent = serde_json::from_value(value).ok()?;
        if event.id.is_empty() || event.run_id.is_empty() {
            return None;
        }
        if event.run_id != expected_run_id {
            return None;
        }
        if event.sequence != expected_sequence {
            return None;
        }
        if !event.payload.is_object() {
            return None;
        }
        Some(event)
    }
}

/// Ordered buffer of accepted run events. Once a terminal event is accepted,
/// later events are ignored — stale terminal events and duplicates never
/// re-enter the timeline (same policy as the mobile's run store).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunEventBuffer {
    events: Vec<RunEvent>,
    terminal: bool,
}

impl RunEventBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[RunEvent] {
        &self.events
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn highest_sequence(&self) -> u64 {
        self.events.last().map(|event| event.sequence).unwrap_or(0)
    }

    /// Try to accept an event. Returns true when accepted; false when the
    /// buffer is already terminal or the sequence is not strictly greater
    /// than the last accepted one.
    pub fn accept(&mut self, event: RunEvent) -> bool {
        if self.terminal {
            return false;
        }
        if event.sequence <= self.highest_sequence() {
            return false;
        }
        if is_terminal_event(&event) {
            self.terminal = true;
        }
        self.events.push(event);
        true
    }
}

/// What the stream yields: a validated run event, or a notification that the
/// connection dropped and the stream is reconnecting from `after`.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Run(RunEvent),
    Reconnecting { attempt: u32, after: u64 },
}

/// A resumable SSE stream over GET /api/runs/:runId/events. Yields validated
/// `RunEvent`s; on a dropped connection it reconnects with `?after=<last
/// sequence>` and exponential backoff. Ends (returns None) after a terminal
/// event or a fatal error is returned via `Err`.
pub struct EventStream {
    http: reqwest::Client,
    base_url: String,
    cookie: String,
    run_id: String,
    after: u64,
    terminal: bool,
    fatal: bool,
    reconnect_attempt: u32,
    next_reconnect: Option<Instant>,
    base_delay_ms: u64,
    max_delay_ms: u64,
    pending: VecDeque<RunEvent>,
    buffer: String,
}

impl EventStream {
    pub fn new(base_url: &str, cookie: &str, run_id: &str, after: u64) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            cookie: cookie.to_string(),
            run_id: run_id.to_string(),
            after,
            terminal: false,
            fatal: false,
            reconnect_attempt: 0,
            next_reconnect: None,
            base_delay_ms: 500,
            max_delay_ms: 8_000,
            pending: VecDeque::new(),
            buffer: String::new(),
        }
    }

    /// Override reconnect delays (tests use short values).
    pub fn with_reconnect_delays(mut self, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        self.base_delay_ms = base_delay_ms.max(10);
        self.max_delay_ms = self.base_delay_ms.max(max_delay_ms);
        self
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn reconnect_delay(&self) -> Duration {
        let exponential = (self.base_delay_ms as f64 * 2f64.powi(self.reconnect_attempt as i32))
            .min(self.max_delay_ms as f64);
        Duration::from_millis(exponential.max(10.0) as u64)
    }

    fn schedule_reconnect(&mut self) {
        if self.next_reconnect.is_none() {
            self.next_reconnect = Some(Instant::now() + self.reconnect_delay());
        }
    }

    /// Read one chunk of the body and validate any complete frames.
    fn consume(&mut self, chunk: &[u8]) -> Result<(), ControlPlaneError> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk).replace("\r\n", "\n"));
        while let Some(boundary) = self.buffer.find("\n\n") {
            let frame_text = self.buffer[..boundary].to_string();
            self.buffer.drain(..boundary + 2);
            if self.terminal {
                continue;
            }
            if let Some(frame) = parse_sse_frame(&frame_text) {
                if let Some(event) = RunEvent::from_sse_frame(&frame, &self.run_id) {
                    if event.sequence > self.after {
                        self.pending.push_back(event);
                    }
                }
            }
            if self.terminal {
                break;
            }
        }
        Ok(())
    }

    fn flush(&mut self) {
        if self.terminal || self.buffer.is_empty() {
            return;
        }
        let frame_text = std::mem::take(&mut self.buffer);
        if let Some(frame) = parse_sse_frame(&frame_text) {
            if let Some(event) = RunEvent::from_sse_frame(&frame, &self.run_id) {
                if event.sequence > self.after {
                    self.pending.push_back(event);
                }
            }
        }
    }

    async fn open_and_read(&mut self) -> Result<(), ControlPlaneError> {
        self.buffer.clear();
        let url = format!(
            "{}/api/runs/{}/events?after={}",
            self.base_url,
            urlencode(&self.run_id),
            self.after
        );
        let response = self
            .http
            .get(url)
            .header(reqwest::header::COOKIE, &self.cookie)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(ControlPlaneError::Http)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ControlPlaneError::SessionExpired);
        }
        if response.status() == StatusCode::NOT_FOUND {
            return Err(ControlPlaneError::Api {
                status: 404,
                code: Some("RUN_NOT_FOUND".to_string()),
                message: "Run not found".to_string(),
            });
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS || response.status().is_server_error() {
            self.schedule_reconnect();
            return Ok(());
        }
        if !response.status().is_success() {
            let status = response.status();
            let bytes = response.bytes().await.map_err(ControlPlaneError::Http)?;
            let api_error: Option<ApiErrorBody> = serde_json::from_slice(&bytes).ok();
            let (code, message) = match api_error {
                Some(body) => (body.error.code, body.error.message.unwrap_or_default()),
                None => (None, format!("Control plane request failed ({status})")),
            };
            return Err(ControlPlaneError::Api {
                status: status.as_u16(),
                code,
                message,
            });
        }
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(ControlPlaneError::Http)?;
            self.consume(&chunk)?;
            if self.terminal {
                return Ok(());
            }
        }
        if self.terminal {
            return Ok(());
        }
        self.flush();
        self.schedule_reconnect();
        Ok(())
    }

    /// Pull the next item from the stream.
    pub async fn next(&mut self) -> Option<Result<StreamEvent, ControlPlaneError>> {
        loop {
            if self.terminal || self.fatal {
                return None;
            }
            if let Some(event) = self.pending.pop_front() {
                self.after = event.sequence;
                if is_terminal_event(&event) {
                    self.terminal = true;
                }
                return Some(Ok(StreamEvent::Run(event)));
            }
            if let Some(wait) = self.next_reconnect.take() {
                tokio::time::sleep_until(wait.into()).await;
                self.reconnect_attempt += 1;
                return Some(Ok(StreamEvent::Reconnecting {
                    attempt: self.reconnect_attempt,
                    after: self.after,
                }));
            }
            match self.open_and_read().await {
                Ok(()) => {}
                Err(ControlPlaneError::Http(_)) => {
                    self.schedule_reconnect();
                }
                Err(error) => {
                    self.fatal = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn parses_id_event_and_data_lines() {
        let frame = parse_sse_frame("id: 3\nevent: specialist.started\ndata: {\"a\":1}\n\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("3"));
        assert_eq!(frame.event.as_deref(), Some("specialist.started"));
        assert_eq!(frame.data, "{\"a\":1}");
    }

    #[test]
    fn skips_comment_lines_and_blank_fields() {
        let frame = parse_sse_frame(": heartbeat\nid: 2\nevent: run.started\ndata: x\n\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("2"));
        assert_eq!(frame.data, "x");
    }

    #[test]
    fn handles_windows_line_endings_and_missing_field_value() {
        let frame = parse_sse_frame("id: 1\r\nevent: run.started\r\ndata: y\r\n\r\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("1"));
        assert_eq!(frame.data, "y");

        // missing field value: `data:` with an empty value still yields a frame
        let bare = parse_sse_frame("id: 1\ndata:\n\n").unwrap();
        assert_eq!(bare.id.as_deref(), Some("1"));
        assert_eq!(bare.data, "");
    }

    #[test]
    fn joins_multiple_data_lines_with_newline() {
        let frame = parse_sse_frame("id: 5\ndata: line1\ndata: line2\n\n").unwrap();
        assert_eq!(frame.data, "line1\nline2");
    }

    #[test]
    fn returns_none_when_no_data_field() {
        assert!(parse_sse_frame(": heartbeat\n\n").is_none());
        assert!(parse_sse_frame("").is_none());
    }

    #[test]
    fn run_event_round_trips_camel_case_json() {
        let json = serde_json::json!({
            "id": "ev_1",
            "runId": "r1",
            "sequence": 1,
            "type": "run.started",
            "version": 1,
            "occurredAt": "2026-08-15T00:00:00.000Z",
            "visibility": "room_and_owner",
            "payload": { "note": "x" }
        });
        let event: RunEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.id, "ev_1");
        assert_eq!(event.run_id, "r1");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.event_type, RunEventType::RunStarted);
        assert_eq!(event.version, 1);
        assert_eq!(event.visibility, EventVisibility::RoomAndOwner);
        assert_eq!(event.event_type.as_str(), "run.started");
    }

    #[test]
    fn unknown_event_type_is_rejected() {
        let json = serde_json::json!({
            "id": "ev_1",
            "runId": "r1",
            "sequence": 1,
            "type": "run.teleported",
            "version": 1,
            "occurredAt": "2026-08-15T00:00:00.000Z",
            "visibility": "room_and_owner",
            "payload": {}
        });
        assert!(serde_json::from_value::<RunEvent>(json).is_err());
    }

    #[test]
    fn all_contract_event_types_parse() {
        let types = [
            "run.queued", "run.started", "specialist.started", "specialist.progress",
            "specialist.completed", "specialist.failed", "run.partial", "run.checkpointed",
            "run.retry_scheduled", "run.cancellation_requested", "run.cancelled",
            "run.completed", "run.failed", "approval.requested", "approval.recorded",
            "mutation.queued", "mutation.completed", "mutation.failed",
        ];
        for name in types {
            let json = serde_json::json!({
                "id": "ev_1", "runId": "r1", "sequence": 1, "type": name, "version": 1,
                "occurredAt": "2026-08-15T00:00:00.000Z", "visibility": "room_and_owner", "payload": {}
            });
            let event: RunEvent = serde_json::from_value(json).unwrap();
            assert_eq!(event.event_type.as_str(), name);
        }
    }

    #[test]
    fn from_sse_frame_accepts_valid_frame_for_expected_run() {
        let frame = SseFrame {
            id: Some("1".to_string()),
            event: Some("run.started".to_string()),
            data: r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{"note":"x"}}"#.to_string(),
        };
        let event = RunEvent::from_sse_frame(&frame, "r1").expect("valid event");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.event_type, RunEventType::RunStarted);
    }

    #[test]
    fn from_sse_frame_rejects_wrong_run_and_mismatched_event_name() {
        let data = r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#;
        let wrong_run = SseFrame {
            id: Some("1".to_string()),
            event: Some("run.started".to_string()),
            data: data.to_string(),
        };
        assert!(RunEvent::from_sse_frame(&wrong_run, "r2").is_none());

        let mismatched = SseFrame {
            id: Some("1".to_string()),
            event: Some("run.completed".to_string()),
            data: data.to_string(),
        };
        assert!(RunEvent::from_sse_frame(&mismatched, "r1").is_none());
    }

    #[test]
    fn from_sse_frame_rejects_malformed_wire_data() {
        let cases = [
            SseFrame { id: Some("abc".to_string()), event: Some("run.started".to_string()), data: "{}".to_string() },
            SseFrame { id: Some("1".to_string()), event: None, data: "{}".to_string() },
            SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: "not json".to_string() },
            SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"ev_1","runId":"r1","sequence":9,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#.to_string() },
            SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#.to_string() },
            SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":"not-an-object"}"#.to_string() },
        ];
        for frame in cases {
            assert!(RunEvent::from_sse_frame(&frame, "r1").is_none(), "expected rejection for {frame:?}");
        }
    }

    fn sample_event(sequence: u64, event_type: RunEventType) -> RunEvent {
        RunEvent {
            id: format!("ev_{sequence}"),
            run_id: "r1".to_string(),
            sequence,
            event_type,
            version: 1,
            occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
            visibility: EventVisibility::RoomAndOwner,
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn buffer_accepts_strictly_increasing_sequences() {
        let mut buffer = RunEventBuffer::new();
        assert!(buffer.accept(sample_event(1, RunEventType::RunQueued)));
        assert!(buffer.accept(sample_event(2, RunEventType::RunStarted)));
        assert_eq!(buffer.highest_sequence(), 2);
        assert_eq!(buffer.events().len(), 2);
        assert!(!buffer.is_terminal());
    }

    #[test]
    fn buffer_rejects_duplicate_and_out_of_order_events() {
        let mut buffer = RunEventBuffer::new();
        assert!(buffer.accept(sample_event(2, RunEventType::RunStarted)));
        assert!(!buffer.accept(sample_event(2, RunEventType::RunStarted)), "duplicate rejected");
        assert!(!buffer.accept(sample_event(1, RunEventType::RunQueued)), "stale rejected");
        assert_eq!(buffer.events().len(), 1);
    }

    #[test]
    fn terminal_event_stops_further_acceptance() {
        let mut buffer = RunEventBuffer::new();
        assert!(buffer.accept(sample_event(1, RunEventType::RunStarted)));
        assert!(buffer.accept(sample_event(2, RunEventType::RunCompleted)));
        assert!(buffer.is_terminal());
        assert!(!buffer.accept(sample_event(3, RunEventType::RunStarted)), "post-terminal ignored");
        assert!(!buffer.accept(sample_event(3, RunEventType::RunCompleted)), "stale terminal ignored");
        assert_eq!(buffer.events().len(), 2);
    }

    fn event_frame(sequence: u64, event_type: &str) -> String {
        let json = serde_json::json!({
            "id": format!("ev_{sequence}"),
            "runId": "r1",
            "sequence": sequence,
            "type": event_type,
            "version": 1,
            "occurredAt": "2026-08-15T00:00:00.000Z",
            "visibility": "room_and_owner",
            "payload": { "note": "x" }
        });
        format!("id: {sequence}\nevent: {event_type}\ndata: {json}\n\n")
    }

    #[tokio::test]
    async fn stream_yields_events_in_order_then_ends_at_terminal() {
        let server = MockServer::start_async().await;
        let body = event_frame(1, "run.queued")
            + &event_frame(2, "specialist.started")
            + &event_frame(3, "run.completed");
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "0")
                    .header("cookie", "cp_session=abc123");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(body);
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
        let mut sequences = Vec::new();
        for _ in 0..8 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(event))) => {
                    sequences.push(event.sequence);
                    if stream.is_terminal() {
                        break;
                    }
                }
                Some(Ok(StreamEvent::Reconnecting { .. })) => panic!("no reconnect expected"),
                Some(Err(error)) => panic!("unexpected error: {error}"),
                None => break,
            }
        }
        assert_eq!(sequences, vec![1, 2, 3]);
        assert!(stream.is_terminal());
        assert!(stream.next().await.is_none());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn stream_resumes_from_last_sequence_after_drop() {
        let server = MockServer::start_async().await;
        let first = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "0");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(event_frame(1, "run.queued") + &event_frame(2, "specialist.started"));
            })
            .await;
        let second = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "2");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(event_frame(3, "run.completed"));
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let mut sequences = Vec::new();
        let mut reconnect_afters = Vec::new();
        for _ in 0..12 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(event))) => {
                    sequences.push(event.sequence);
                    if stream.is_terminal() {
                        break;
                    }
                }
                Some(Ok(StreamEvent::Reconnecting { after, .. })) => {
                    reconnect_afters.push(after);
                }
                Some(Err(error)) => panic!("unexpected error: {error}"),
                None => break,
            }
        }
        assert_eq!(sequences, vec![1, 2, 3]);
        assert_eq!(reconnect_afters, vec![2], "resume cursor must be the last sequence");
        first.assert_async().await;
        second.assert_async().await;
    }

    #[tokio::test]
    async fn stream_skips_malformed_frames_without_stopping() {
        let server = MockServer::start_async().await;
        let garbage = ": heartbeat\n\n".to_string()
            + "id: not-a-number\nevent: run.started\ndata: {}\n\n"
            + "id: 1\nevent: run.started\ndata: not json\n\n"
            + &event_frame(1, "run.queued")
            + "id: 2\nevent: run.started\ndata: {\"id\":\"ev_x\",\"runId\":\"OTHER_RUN\",\"sequence\":2,\"type\":\"run.started\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n"
            + &event_frame(3, "run.completed");
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(garbage);
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
        let mut sequences = Vec::new();
        for _ in 0..8 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(event))) => {
                    sequences.push(event.sequence);
                    if stream.is_terminal() {
                        break;
                    }
                }
                Some(Ok(StreamEvent::Reconnecting { .. })) => panic!("no reconnect expected"),
                Some(Err(error)) => panic!("unexpected error: {error}"),
                None => break,
            }
        }
        assert_eq!(sequences, vec![1, 3], "only valid events for this run are accepted");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn stream_maps_401_to_session_expired() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(401);
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=stale", "r1", 0);
        let error = stream.next().await.expect("yields an error").unwrap_err();
        assert!(error.is_session_expired());
    }

    #[tokio::test]
    async fn stream_maps_404_to_run_not_found() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(404).json_body(serde_json::json!({
                    "error": { "code": "RUN_NOT_FOUND", "message": "Run not found", "requestId": "req_1" }
                }));
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
        let error = stream.next().await.expect("yields an error").unwrap_err();
        assert_eq!(error.status(), Some(404));
        assert_eq!(error.code(), Some("RUN_NOT_FOUND"));
    }

    #[tokio::test]
    async fn stream_flushes_trailing_frame_without_blank_line() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(event_frame(1, "run.queued").trim_end_matches("\n\n"));
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let mut sequences = Vec::new();
        for _ in 0..8 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(event))) => {
                    sequences.push(event.sequence);
                    if stream.is_terminal() {
                        break;
                    }
                }
                Some(Ok(StreamEvent::Reconnecting { .. })) => break,
                Some(Err(error)) => panic!("unexpected error: {error}"),
                None => break,
            }
        }
        assert_eq!(sequences, vec![1], "trailing frame without blank line must be delivered");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn stream_resets_stale_buffer_between_connections() {
        let server = MockServer::start_async().await;
        let first = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "0");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(
                        event_frame(1, "run.queued")
                            + &event_frame(2, "specialist.started")
                            + "id: 3\nevent: run.completed\ndata: {\"id\":\"ev_3\"",
                    );
            })
            .await;
        let second = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "2");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(event_frame(3, "run.completed"));
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let mut sequences = Vec::new();
        let mut reconnect_afters = Vec::new();
        for _ in 0..12 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(event))) => {
                    sequences.push(event.sequence);
                    if stream.is_terminal() {
                        break;
                    }
                }
                Some(Ok(StreamEvent::Reconnecting { after, .. })) => {
                    reconnect_afters.push(after);
                }
                Some(Err(error)) => panic!("unexpected error: {error}"),
                None => break,
            }
        }
        assert_eq!(
            sequences,
            vec![1, 2, 3],
            "retransmitted frame must not be corrupted by a stale partial buffer"
        );
        assert_eq!(reconnect_afters, vec![2]);
        first.assert_async().await;
        second.assert_async().await;
    }

    #[tokio::test]
    async fn stream_stops_on_unhandled_client_error_status() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(403);
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let mut statuses = Vec::new();
        for _ in 0..4 {
            match stream.next().await {
                Some(Ok(StreamEvent::Run(_))) => panic!("no events expected"),
                Some(Ok(StreamEvent::Reconnecting { .. })) => {
                    panic!("fatal client status must not trigger a reconnect")
                }
                Some(Err(error)) => statuses.push(error.status()),
                None => break,
            }
        }
        assert_eq!(
            statuses,
            vec![Some(403)],
            "fatal status surfaces exactly once, then the stream ends"
        );
        assert!(
            stream.next().await.is_none(),
            "stream must end after the fatal error, not re-issue the request"
        );
    }

    #[tokio::test]
    async fn stream_reconnects_on_transport_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let mut stream = EventStream::new(&format!("http://{addr}"), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let item = stream
            .next()
            .await
            .expect("a transport failure must not end the stream");
        assert!(
            matches!(item, Ok(StreamEvent::Reconnecting { attempt: 1, after: 0 })),
            "expected a reconnect notification, got {item:?}"
        );
    }

    #[tokio::test]
    async fn stream_reconnects_on_too_many_requests() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1/events");
                then.status(429);
            })
            .await;

        let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
            .with_reconnect_delays(10, 50);
        let item = stream.next().await.expect("429 must schedule a reconnect");
        assert!(
            matches!(item, Ok(StreamEvent::Reconnecting { attempt: 1, after: 0 })),
            "expected a reconnect notification, got {item:?}"
        );
        mock.assert_async().await;
    }
}
