use crate::contract::{RunRequest, RunResponse};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/workspaces/:workspaceId/runs — launch a run. The caller
    /// always passes a fresh idempotency key (mirrors the mobile composer).
    pub async fn launch_run(
        &self,
        workspace_id: &str,
        request: &RunRequest,
        idempotency_key: &str,
    ) -> Result<RunResponse, ControlPlaneError> {
        let mut body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "idempotencyKey".to_string(),
                serde_json::Value::String(idempotency_key.to_string()),
            );
        }
        let path = format!("/api/workspaces/{}/runs", urlencode(workspace_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{RunMode, RunStatus};
    use httpmock::prelude::*;

    #[tokio::test]
    async fn launch_run_sends_request_plus_idempotency_key() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces/ws_1/runs")
                    .body_contains(r#""prompt":"Summarize the PRs""#)
                    .body_contains(r#""mode":"parallel""#)
                    .body_contains(r#""specialistIds":["pr-reader"]"#)
                    .body_contains(r#""roomId":"!a:matrix.example.org""#)
                    .body_contains(r#""idempotencyKey":"key_42""#);
                then.status(202).json_body(json!({
                    "runId": "r1",
                    "status": "queued",
                    "roomId": "!a:matrix.example.org",
                    "nextSequence": 1
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = RunRequest {
            prompt: "Summarize the PRs".to_string(),
            mode: RunMode::Parallel,
            specialist_ids: vec!["pr-reader".to_string()],
            room_id: Some("!a:matrix.example.org".to_string()),
            github_context: None,
        };
        let run: RunResponse = client.launch_run("ws_1", &request, "key_42").await.unwrap();

        assert_eq!(run.run_id, "r1");
        assert_eq!(run.status, RunStatus::Queued);
        assert_eq!(run.next_sequence, 1);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn launch_run_omits_optional_fields_when_absent() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/api/workspaces/ws_1/runs");
                then.status(202).json_body(json!({
                    "runId": "r2",
                    "status": "queued",
                    "nextSequence": 0
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = RunRequest {
            prompt: "hi".to_string(),
            mode: RunMode::Sequential,
            specialist_ids: vec!["repo-reader".to_string()],
            room_id: None,
            github_context: None,
        };
        let run: RunResponse = client.launch_run("ws_1", &request, "key_43").await.unwrap();
        assert_eq!(run.room_id, None);
        assert_eq!(run.status, RunStatus::Queued);
        mock.assert_async().await;
    }
}
