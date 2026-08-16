//! Session persistence and the screen state machine for matrix-workspace-tui.

pub mod screens;
pub mod session_store;

pub use session_store::{SessionData, SessionStore, StateError};
