use crate::contract::{MatrixSessionRequest, MatrixSessionResponse};
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/auth/matrix/session — exchange a homeserver URL + Matrix
    /// access token for a control-plane session cookie. The cookie is stored
    /// on this client (the caller persists it via the SessionStore).
    pub async fn create_matrix_session(
        &mut self,
        homeserver_url: &str,
        access_token: &str,
    ) -> Result<MatrixSessionResponse, ControlPlaneError> {
        let request = MatrixSessionRequest {
            homeserver_url: homeserver_url.to_string(),
            access_token: access_token.to_string(),
        };
        let body = json!(request);
        let (response, cookie) = self.login_request("/api/auth/matrix/session", &body).await?;
        match cookie {
            Some(cookie) => {
                self.set_cookie(Some(cookie));
                Ok(response)
            }
            None => Err(ControlPlaneError::SessionReferenceMissing),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn create_matrix_session_stores_set_cookie() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/auth/matrix/session")
                    .body_contains(r#""homeserverUrl":"https://matrix.example.org""#)
                    .body_contains(r#""accessToken":"tok_123""#);
                then.status(200)
                    .header("set-cookie", "cp_session=abc123; Path=/; HttpOnly")
                    .json_body(json!({
                        "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                        "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                    }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let session: MatrixSessionResponse = client
            .create_matrix_session("https://matrix.example.org", "tok_123")
            .await
            .unwrap();

        assert_eq!(session.user.id, "@u:matrix.example.org");
        assert_eq!(session.session_expires_at, "2026-08-15T01:00:00.000Z");
        assert_eq!(client.cookie(), Some("cp_session=abc123"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_matrix_session_without_cookie_is_reference_missing() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/auth/matrix/session");
                then.status(200).json_body(json!({
                    "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                    "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let error = client
            .create_matrix_session("https://matrix.example.org", "tok_123")
            .await
            .unwrap_err();
        assert!(matches!(error, crate::ControlPlaneError::SessionReferenceMissing));
        assert_eq!(client.cookie(), None);
    }

    #[tokio::test]
    async fn create_matrix_session_invalid_credentials_surfaces_api_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/auth/matrix/session");
                then.status(401).json_body(json!({
                    "error": { "code": "MATRIX_AUTH_FAILED", "message": "Invalid homeserver or token", "requestId": "req_1" }
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let error = client
            .create_matrix_session("https://matrix.example.org", "bad")
            .await
            .unwrap_err();
        assert_eq!(error.status(), Some(401));
        assert_eq!(error.code(), Some("MATRIX_AUTH_FAILED"));
    }
}
