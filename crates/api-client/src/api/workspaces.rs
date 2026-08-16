use crate::contract::{CreateWorkspaceRequest, WorkspacePolicy, WorkspaceSelection};
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/workspaces — create a workspace with the same policy the
    /// mobile app uses.
    pub async fn create_workspace(&self, name: &str) -> Result<WorkspaceSelection, ControlPlaneError> {
        let request = CreateWorkspaceRequest {
            name: name.trim().to_string(),
            policy: WorkspacePolicy {
                read_only: true,
                failure_policy: "partial".to_string(),
                prompt_injection_mode: "fail_run".to_string(),
            },
        };
        let body = json!(request);
        self.authenticated_request(reqwest::Method::POST, "/api/workspaces", Some(&body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn create_workspace_sends_policy_and_returns_selection() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces")
                    .header("cookie", "cp_session=abc123")
                    .body_contains(r#""name":"my workspace""#)
                    .body_contains(r#""readOnly":true"#)
                    .body_contains(r#""failurePolicy":"partial""#)
                    .body_contains(r#""promptInjectionMode":"fail_run""#);
                then.status(201).json_body(json!({
                    "requestId": "req_1",
                    "workspaceId": "ws_1",
                    "name": "my workspace",
                    "ownerId": "@u:matrix.example.org",
                    "status": "active",
                    "createdAt": "2026-08-15T00:00:00.000Z"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let workspace: WorkspaceSelection = client.create_workspace("  my workspace  ").await.unwrap();

        assert_eq!(workspace.workspace_id, "ws_1");
        assert_eq!(workspace.name, "my workspace");
        assert_eq!(workspace.status, "active");
        mock.assert_async().await;
    }
}
