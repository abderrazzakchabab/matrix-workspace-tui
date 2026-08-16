use crate::contract::{GithubPage, GithubRepositorySummary};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};

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
}
