use crate::contract::{EventVisibility, RunEvent, RunEventType};
use crate::error::ControlPlaneError;
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

#[cfg(test)]
mod tests {
    use super::*;

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
