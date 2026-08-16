//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod api;
pub mod contract;
pub mod error;
pub mod http;

pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
pub use http::ControlPlaneApi;
