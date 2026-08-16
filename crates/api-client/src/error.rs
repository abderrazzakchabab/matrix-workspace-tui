use thiserror::Error;

/// Structured error body returned by control-plane endpoints
/// (mirrors `ApiError` in packages/contracts/src/errors.ts; fields kept
/// optional because some endpoints return partial bodies).
#[derive(Debug, serde::Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiErrorDetail {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Error)]
pub enum ControlPlaneError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("no control-plane session stored; sign in first")]
    NoSession,

    #[error("session expired; sign in again")]
    SessionExpired,

    #[error("the control plane did not return a session reference")]
    SessionReferenceMissing,

    #[error("control plane returned {status}: {message}")]
    Api {
        status: u16,
        code: Option<String>,
        message: String,
    },

    #[error("unexpected response from control plane: {0}")]
    InvalidResponse(String),

    #[error("invalid control plane base url")]
    InvalidBaseUrl,
}

impl ControlPlaneError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ControlPlaneError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            ControlPlaneError::Api { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    pub fn is_session_expired(&self) -> bool {
        matches!(self, ControlPlaneError::SessionExpired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_error(status: u16, code: &str, message: &str) -> ControlPlaneError {
        ControlPlaneError::Api {
            status,
            code: Some(code.to_string()),
            message: message.to_string(),
        }
    }

    #[test]
    fn api_error_exposes_status_code_and_message() {
        let error = api_error(422, "VALIDATION_ERROR", "Invalid workspace");
        assert_eq!(error.status(), Some(422));
        assert_eq!(error.code(), Some("VALIDATION_ERROR"));
        assert_eq!(error.to_string(), "control plane returned 422: Invalid workspace");
    }

    #[test]
    fn api_error_without_code_formats_cleanly() {
        let error = ControlPlaneError::Api {
            status: 500,
            code: None,
            message: "boom".to_string(),
        };
        assert_eq!(error.to_string(), "control plane returned 500: boom");
    }

    #[test]
    fn session_expired_is_a_distinct_signal() {
        let error = ControlPlaneError::SessionExpired;
        assert!(error.is_session_expired());
        assert!(!api_error(401, "SESSION_REQUIRED", "Sign in again").is_session_expired());
        assert_eq!(error.to_string(), "session expired; sign in again");
    }

    #[test]
    fn error_body_parses_optional_fields() {
        let json = r#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid workspace","requestId":"req_1"}}"#;
        let body: ApiErrorBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.error.code.as_deref(), Some("VALIDATION_ERROR"));
        assert_eq!(body.error.message.as_deref(), Some("Invalid workspace"));
        let body: ApiErrorBody = serde_json::from_str(r#"{"error":{}}"#).unwrap();
        assert_eq!(body.error.code, None);
    }
}
