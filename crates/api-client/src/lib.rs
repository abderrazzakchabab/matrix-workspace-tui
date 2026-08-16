//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod api;
pub mod contract;
pub mod error;
pub mod http;
pub mod sse;

pub use sse::{
    is_terminal_event, EventStream, RunEventBuffer, SseFrame, StreamEvent, TERMINAL_EVENT_TYPES,
};

pub use api::*;
pub use contract::*;
pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
pub use http::ControlPlaneApi;

#[cfg(test)]
mod tests {
    #[test]
    fn sse_surface_is_public() {
        let _: fn(&str) -> Option<crate::sse::SseFrame> = crate::sse::parse_sse_frame;
        let _ = crate::sse::is_terminal_event;
        let _: Option<crate::sse::RunEventBuffer> = None;
        let _: Option<crate::sse::EventStream> = None;
        let _ = crate::sse::StreamEvent::Reconnecting {
            attempt: 1,
            after: 0,
        };
        let _ = crate::sse::TERMINAL_EVENT_TYPES;
    }
}
