// use crate::contract::{EventVisibility, RunEvent, RunEventType}; // uncommented in Task 4.2
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
}
