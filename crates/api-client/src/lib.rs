//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod api;
pub mod contract;
pub mod error;
pub mod http;
pub mod sse;

pub use api::*;
pub use contract::*;
pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
pub use http::ControlPlaneApi;
