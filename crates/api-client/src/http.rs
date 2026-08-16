use crate::error::{ApiErrorBody, ControlPlaneError};
use reqwest::StatusCode;

/// Percent-encode a path segment the same way JavaScript's
/// `encodeURIComponent` does (used for room ids, run ids, owner/repo names).
pub(crate) fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The control-plane client: a base URL plus the session cookie. All
/// authenticated methods mirror `ControlPlaneApi` in
/// apps/mobile/src/api/control-plane.ts.
pub struct ControlPlaneApi {
    http: reqwest::Client,
    base_url: String,
    cookie: Option<String>,
}

impl ControlPlaneApi {
    pub fn new(base_url: impl Into<String>) -> Result<Self, ControlPlaneError> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(ControlPlaneError::InvalidBaseUrl);
        }
        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            cookie: None,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn set_cookie(&mut self, cookie: Option<String>) {
        self.cookie = cookie;
    }

    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Send an authenticated request and return the response status + parsed
    /// JSON body. A 401 anywhere means the session cookie is no longer valid
    /// and maps to the `SessionExpired` signal.
    pub(crate) async fn authenticated_response(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(StatusCode, serde_json::Value), ControlPlaneError> {
        let cookie = self.cookie.as_deref().ok_or(ControlPlaneError::NoSession)?;
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header(reqwest::header::COOKIE, cookie)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(body);
        }
        let response = request.send().await.map_err(ControlPlaneError::Http)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ControlPlaneError::SessionExpired);
        }
        let status = response.status();
        let bytes = response.bytes().await.map_err(ControlPlaneError::Http)?;
        if status.is_success() {
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
            Ok((status, value))
        } else {
            let api_error: Option<ApiErrorBody> = serde_json::from_slice(&bytes).ok();
            let (code, message) = match api_error {
                Some(body) => (body.error.code, body.error.message.unwrap_or_default()),
                None => (None, format!("Control plane request failed ({status})")),
            };
            Err(ControlPlaneError::Api {
                status: status.as_u16(),
                code,
                message,
            })
        }
    }

    /// Send an authenticated request and deserialize the JSON body into `T`.
    pub(crate) async fn authenticated_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ControlPlaneError> {
        let (_, value) = self.authenticated_response(method, path, body).await?;
        serde_json::from_value(value).map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))
    }

    /// POST with a JSON body but no cookie (only used by
    /// `create_matrix_session`). Returns the parsed body plus the `Set-Cookie`
    /// value if the server issued one.
    pub(crate) async fn login_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(T, Option<String>), ControlPlaneError> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(ControlPlaneError::Http)?;
        let status = response.status();
        let set_cookie = response
            .headers()
            .get(reqwest::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or("").trim().to_string())
            .filter(|cookie| !cookie.is_empty());
        let bytes = response.bytes().await.map_err(ControlPlaneError::Http)?;
        if status.is_success() {
            let value = serde_json::from_slice(&bytes)
                .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
            Ok((value, set_cookie))
        } else {
            let api_error: Option<ApiErrorBody> = serde_json::from_slice(&bytes).ok();
            let (code, message) = match api_error {
                Some(body) => (body.error.code, body.error.message.unwrap_or_default()),
                None => (None, format!("Control plane request failed ({status})")),
            };
            Err(ControlPlaneError::Api {
                status: status.as_u16(),
                code,
                message,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn new_normalizes_base_url_and_rejects_empty() {
        let client = ControlPlaneApi::new("  http://localhost:3000/// ").unwrap();
        assert_eq!(client.base_url(), "http://localhost:3000");
        assert!(ControlPlaneApi::new("   ").is_err());
    }

    #[test]
    fn cookie_accessors_default_to_none() {
        let mut client = ControlPlaneApi::new("http://localhost:3000").unwrap();
        assert_eq!(client.cookie(), None);
        client.set_cookie(Some("sid=abc".to_string()));
        assert_eq!(client.cookie(), Some("sid=abc"));
        client.set_cookie(None);
        assert_eq!(client.cookie(), None);
    }

    #[test]
    fn urlencode_matches_js_encodeuricomponent_semantics() {
        assert_eq!(urlencode("!room:example.org"), "%21room%3Aexample.org");
        assert_eq!(urlencode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(urlencode("plain-id_1.2~3"), "plain-id_1.2~3");
    }

    #[tokio::test]
    async fn authenticated_request_sends_cookie_and_parses_body() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/rooms")
                    .header("cookie", "sid=abc");
                then.status(200).json_body(json!({ "rooms": [] }));
            })
            .await;
        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("sid=abc".to_string()));
        let value: serde_json::Value = client
            .authenticated_request(reqwest::Method::GET, "/api/rooms", None)
            .await
            .unwrap();
        assert_eq!(value["rooms"], json!([]));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn authenticated_request_maps_401_to_session_expired() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/rooms");
                then.status(401).json_body(json!({
                    "error": { "code": "SESSION_REQUIRED", "message": "Sign in again", "requestId": "req_1" }
                }));
            })
            .await;
        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("sid=abc".to_string()));
        let error = client
            .authenticated_request::<serde_json::Value>(reqwest::Method::GET, "/api/rooms", None)
            .await
            .unwrap_err();
        assert!(error.is_session_expired());
    }

    #[tokio::test]
    async fn authenticated_request_maps_non_ok_status_to_typed_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/workspaces");
                then.status(422).json_body(json!({
                    "error": { "code": "VALIDATION_ERROR", "message": "Invalid workspace", "requestId": "req_1" }
                }));
            })
            .await;
        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("sid=abc".to_string()));
        let error = client
            .authenticated_request::<serde_json::Value>(
                reqwest::Method::GET,
                "/api/workspaces",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(error.status(), Some(422));
        assert_eq!(error.code(), Some("VALIDATION_ERROR"));
        assert_eq!(
            error.to_string(),
            "control plane returned 422: Invalid workspace"
        );
    }

    #[tokio::test]
    async fn authenticated_request_without_session_is_no_session() {
        let client = ControlPlaneApi::new("http://localhost:9").unwrap();
        let error = client
            .authenticated_request::<serde_json::Value>(reqwest::Method::GET, "/api/rooms", None)
            .await
            .unwrap_err();
        assert!(matches!(error, ControlPlaneError::NoSession));
    }

    #[tokio::test]
    async fn authenticated_request_posts_json_body() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces")
                    .header("content-type", "application/json")
                    .body_contains(r#""name":"my workspace""#)
                    .body_contains(r#""readOnly":true"#);
                then.status(201).json_body(json!({
                    "requestId": "req_1",
                    "workspaceId": "ws_1",
                    "name": "my workspace",
                    "ownerId": "@u:example.org",
                    "status": "active",
                    "createdAt": "2026-08-15T00:00:00.000Z"
                }));
            })
            .await;
        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("sid=abc".to_string()));
        let body = json!({ "name": "my workspace", "policy": { "readOnly": true } });
        let value: serde_json::Value = client
            .authenticated_request(reqwest::Method::POST, "/api/workspaces", Some(&body))
            .await
            .unwrap();
        assert_eq!(value["workspaceId"], "ws_1");
        mock.assert_async().await;
    }
}
