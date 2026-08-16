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

    /// POST /api/runs/:runId/cancel — request cancellation.
    pub async fn cancel_run(&self, run_id: &str) -> Result<crate::contract::CancellationResponse, ControlPlaneError> {
        let path = format!("/api/runs/{}/cancel", urlencode(run_id));
        self.authenticated_request(reqwest::Method::POST, &path, None)
            .await
    }

    /// GET /api/runs/:runId — read the authoritative Matrix delivery statuses.
    pub async fn get_run_matrix_deliveries(
        &self,
        run_id: &str,
    ) -> Result<crate::contract::RunMatrixDeliveriesResponse, ControlPlaneError> {
        use crate::contract::MatrixDelivery;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RunDetailBody {
            run_id: String,
            matrix_deliveries: Vec<MatrixDelivery>,
        }
        let path = format!("/api/runs/{}", urlencode(run_id));
        let body: RunDetailBody =
            self.authenticated_request(reqwest::Method::GET, &path, None).await?;
        Ok(crate::contract::RunMatrixDeliveriesResponse {
            run_id: body.run_id,
            deliveries: body.matrix_deliveries,
        })
    }

    /// POST /api/runs/:runId/approvals — record the explicit human approval.
    /// Only ever called from the explicit mutation confirmation action.
    pub async fn create_run_approval(
        &self,
        run_id: &str,
        request: &crate::contract::CreateApprovalRequest,
    ) -> Result<crate::contract::RunApprovalResult, ControlPlaneError> {
        let body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        let path = format!("/api/runs/{}/approvals", urlencode(run_id));
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

    #[tokio::test]
    async fn cancel_run_posts_and_returns_cancellation_requested() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/api/runs/r1/cancel");
                then.status(202).json_body(json!({
                    "requestId": "req_1",
                    "runId": "r1",
                    "status": "cancellation_requested"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let response = client.cancel_run("r1").await.unwrap();

        assert_eq!(response.run_id, "r1");
        assert_eq!(response.status, crate::contract::CancellationStatus::CancellationRequested);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn get_run_matrix_deliveries_reads_authoritative_status() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/runs/r1");
                then.status(200).json_body(json!({
                    "requestId": "req_1",
                    "runId": "r1",
                    "status": "running",
                    "mode": "parallel",
                    "workspaceId": "ws_1",
                    "roomId": null,
                    "specialists": [],
                    "lastSequence": 5,
                    "matrixDeliveries": [
                        { "sequence": 1, "status": "delivered" },
                        { "sequence": 2, "status": "pending" }
                    ],
                    "cancelRequestedAt": null
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let deliveries = client.get_run_matrix_deliveries("r1").await.unwrap();

        assert_eq!(deliveries.run_id, "r1");
        assert_eq!(deliveries.deliveries.len(), 2);
        assert_eq!(deliveries.deliveries[0].sequence, 1);
        assert_eq!(
            deliveries.deliveries[0].status,
            crate::contract::MatrixDeliveryStatus::Delivered
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_run_approval_posts_exact_confirmation() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/runs/r1/approvals")
                    .body_contains(r#""approvalType":"github_mutation""#)
                    .body_contains(r#""scope":"issues:write""#)
                    .body_contains(r#""decision":"approved""#)
                    .body_contains(r#""confirmationText":"I confirm create issue on octo/repo (issues:write)""#)
                    .body_contains(r#""commandHash":"22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8""#);
                then.status(200).json_body(json!({
                    "approvalId": "apr_1",
                    "status": "approved",
                    "expiresAt": "2026-08-15T01:00:00.000Z",
                    "scope": "issues:write"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = crate::contract::CreateApprovalRequest {
            approval_type: crate::contract::ApprovalType::GithubMutation,
            scope: crate::contract::GithubWriteScope::IssuesWrite,
            decision: crate::contract::ApprovalDecision::Approved,
            confirmation_text: "I confirm create issue on octo/repo (issues:write)".to_string(),
            command_hash: "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8".to_string(),
        };
        let result = client.create_run_approval("r1", &request).await.unwrap();

        assert_eq!(result.approval_id, "apr_1");
        assert_eq!(result.status, crate::contract::ApprovalStatus::Approved);
        mock.assert_async().await;
    }
}
