use crate::contract::{GithubPage, GithubPullRequestSummary, GithubRepositorySummary};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};
use serde_json::json;

impl ControlPlaneApi {
    /// Build the shared `?workspaceId=..&installationId=..[&cursor=..]` query
    /// suffix for the GitHub read routes.
    fn github_read_path(
        &self,
        workspace_id: &str,
        installation_id: &str,
        suffix: &str,
        cursor: Option<&str>,
    ) -> String {
        let mut path = format!(
            "{suffix}?workspaceId={}&installationId={}",
            urlencode(workspace_id),
            urlencode(installation_id)
        );
        if let Some(cursor) = cursor {
            path.push_str(&format!("&cursor={}", urlencode(cursor)));
        }
        path
    }

    /// GET /api/github/repositories — list the installation's repositories.
    pub async fn list_github_repositories(
        &self,
        workspace_id: &str,
        installation_id: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<GithubRepositorySummary>, ControlPlaneError> {
        let path = self.github_read_path(
            workspace_id,
            installation_id,
            "/api/github/repositories",
            cursor,
        );
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }

    /// GET /api/github/repositories/:owner/:repo/issues
    pub async fn list_github_issues(
        &self,
        workspace_id: &str,
        installation_id: &str,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<crate::contract::GithubIssueSummary>, ControlPlaneError> {
        let suffix = format!(
            "/api/github/repositories/{}/{}/issues",
            urlencode(owner),
            urlencode(repo)
        );
        let path = self.github_read_path(workspace_id, installation_id, &suffix, cursor);
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }

    /// GET /api/github/repositories/:owner/:repo/pulls
    pub async fn list_github_pull_requests(
        &self,
        workspace_id: &str,
        installation_id: &str,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<GithubPullRequestSummary>, ControlPlaneError> {
        let suffix = format!(
            "/api/github/repositories/{}/{}/pulls",
            urlencode(owner),
            urlencode(repo)
        );
        let path = self.github_read_path(workspace_id, installation_id, &suffix, cursor);
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }

    /// POST /api/workspaces/:workspaceId/github-grants — request a separate
    /// repository+scope write grant (Phase B read auth never implies write).
    pub async fn request_github_write_grant(
        &self,
        workspace_id: &str,
        repository: &str,
        scope: crate::contract::GithubWriteScope,
    ) -> Result<crate::contract::GithubWriteGrantResult, ControlPlaneError> {
        use crate::contract::CreateGrantRequest;
        let body = json!(CreateGrantRequest {
            repository: repository.to_string(),
            scope,
        });
        let path = format!("/api/workspaces/{}/github-grants", urlencode(workspace_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }

    /// POST /api/workspaces/:workspaceId/github/mutations — enqueue an
    /// approval-gated, idempotent mutation. 202 = newly queued, 200 = replay
    /// of the same idempotency key (mirrors the mobile's replayed flag).
    pub async fn enqueue_github_mutation(
        &self,
        workspace_id: &str,
        request: &crate::contract::EnqueueMutationRequest,
    ) -> Result<crate::contract::GithubMutationResult, ControlPlaneError> {
        use crate::contract::{GithubMutationResult, MutationStatus};
        let body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        let path = format!("/api/workspaces/{}/github/mutations", urlencode(workspace_id));
        let (status, value) = self
            .authenticated_response(reqwest::Method::POST, &path, Some(&body))
            .await?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct MutationBody {
            command_id: String,
            status: MutationStatus,
        }
        let parsed: MutationBody = serde_json::from_value(value)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        Ok(GithubMutationResult {
            command_id: parsed.command_id,
            status: parsed.status,
            replayed: status == reqwest::StatusCode::OK,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::GithubRepositorySummary;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_github_repositories_sends_query_params() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9")
                    .query_param("cursor", "p2");
                then.status(200).json_body(json!({
                    "items": [
                        {
                            "id": 1,
                            "name": "repo",
                            "fullName": "octo/repo",
                            "owner": "octo",
                            "private": false,
                            "defaultBranch": "main",
                            "description": "A repo",
                            "htmlUrl": "https://github.com/octo/repo",
                            "archived": false
                        }
                    ],
                    "nextCursor": "p3"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<GithubRepositorySummary> = client
            .list_github_repositories("ws_1", "inst_9", Some("p2"))
            .await
            .unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].full_name, "octo/repo");
        assert_eq!(page.items[0].private, false);
        assert_eq!(page.next_cursor.as_deref(), Some("p3"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_github_repositories_without_cursor_omits_param() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9");
                then.status(200).json_body(json!({ "items": [] }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<GithubRepositorySummary> = client
            .list_github_repositories("ws_1", "inst_9", None)
            .await
            .unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_github_issues_uses_owner_repo_path_and_cursor() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories/octo/repo/issues")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9")
                    .query_param("cursor", "p2");
                then.status(200).json_body(json!({
                    "items": [
                        {
                            "id": 11,
                            "number": 42,
                            "title": "Fix the bug",
                            "state": "open",
                            "author": "octo",
                            "labels": ["bug"],
                            "htmlUrl": "https://github.com/octo/repo/issues/42",
                            "createdAt": "2026-08-01T00:00:00.000Z",
                            "updatedAt": "2026-08-02T00:00:00.000Z"
                        }
                    ]
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<crate::contract::GithubIssueSummary> = client
            .list_github_issues("ws_1", "inst_9", "octo", "repo", Some("p2"))
            .await
            .unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].number, 42);
        assert_eq!(page.items[0].title, "Fix the bug");
        assert_eq!(page.items[0].labels, vec!["bug"]);
        assert_eq!(page.next_cursor, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_github_pull_requests_parses_draft_and_head_base() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories/octo/repo/pulls")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9");
                then.status(200).json_body(json!({
                    "items": [
                        {
                            "id": 22,
                            "number": 7,
                            "title": "Add docs",
                            "state": "open",
                            "draft": true,
                            "author": null,
                            "head": "octo:docs",
                            "base": "main",
                            "htmlUrl": "https://github.com/octo/repo/pull/7",
                            "createdAt": "2026-08-01T00:00:00.000Z",
                            "updatedAt": "2026-08-02T00:00:00.000Z"
                        }
                    ]
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<crate::contract::GithubPullRequestSummary> = client
            .list_github_pull_requests("ws_1", "inst_9", "octo", "repo", None)
            .await
            .unwrap();

        assert_eq!(page.items[0].draft, true);
        assert_eq!(page.items[0].author, None);
        assert_eq!(page.items[0].head, "octo:docs");
        assert_eq!(page.items[0].base, "main");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn request_github_write_grant_posts_repository_and_scope() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces/ws_1/github-grants")
                    .body_contains(r#""repository":"octo/repo""#)
                    .body_contains(r#""scope":"issues:write""#);
                then.status(201).json_body(json!({
                    "grantId": "gr_1",
                    "status": "pending",
                    "repository": "octo/repo",
                    "scope": "issues:write"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let result = client
            .request_github_write_grant("ws_1", "octo/repo", crate::contract::GithubWriteScope::IssuesWrite)
            .await
            .unwrap();

        assert_eq!(result.grant_id, "gr_1");
        assert_eq!(result.status, crate::contract::GrantStatus::Pending);
        assert_eq!(result.scope, crate::contract::GithubWriteScope::IssuesWrite);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn enqueue_github_mutation_202_is_new_command() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces/ws_1/github/mutations")
                    .body_contains(r#""idempotencyKey":"key_1""#)
                    .body_contains(r#""approvalId":"apr_1""#)
                    .body_contains(r#""repository":"octo/repo""#)
                    .body_contains(r#""runId":"r1""#)
                    .body_contains(r#""operation":"create_issue""#)
                    .body_contains(r#""title":"Fix the bug""#);
                then.status(202).json_body(json!({
                    "commandId": "cmd_1",
                    "status": "queued"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = crate::contract::EnqueueMutationRequest {
            idempotency_key: "key_1".to_string(),
            approval_id: "apr_1".to_string(),
            repository: "octo/repo".to_string(),
            run_id: Some("r1".to_string()),
            operation: crate::contract::GithubMutationOperation::CreateIssue,
            arguments: serde_json::json!({ "title": "Fix the bug" }),
        };
        let result = client.enqueue_github_mutation("ws_1", &request).await.unwrap();

        assert_eq!(result.command_id, "cmd_1");
        assert_eq!(result.status, crate::contract::MutationStatus::Queued);
        assert!(!result.replayed);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn enqueue_github_mutation_200_is_idempotent_replay() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/api/workspaces/ws_1/github/mutations");
                then.status(200).json_body(json!({
                    "commandId": "cmd_1",
                    "status": "completed"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = crate::contract::EnqueueMutationRequest {
            idempotency_key: "key_1".to_string(),
            approval_id: "apr_1".to_string(),
            repository: "octo/repo".to_string(),
            run_id: None,
            operation: crate::contract::GithubMutationOperation::CreateIssue,
            arguments: serde_json::json!({ "title": "Fix the bug" }),
        };
        let result = client.enqueue_github_mutation("ws_1", &request).await.unwrap();

        assert_eq!(result.status, crate::contract::MutationStatus::Completed);
        assert!(result.replayed);
        mock.assert_async().await;
    }
}

