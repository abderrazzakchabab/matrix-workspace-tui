use crate::contract::{AuditRecordItem, GithubPage};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};

impl ControlPlaneApi {
    /// GET /api/workspaces/:workspaceId/audit — keyset-paginated audit trail.
    pub async fn list_audit_records(
        &self,
        workspace_id: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<AuditRecordItem>, ControlPlaneError> {
        let mut path = format!("/api/workspaces/{}/audit", urlencode(workspace_id));
        if let Some(cursor) = cursor {
            path.push_str(&format!("?cursor={}", urlencode(cursor)));
        }
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_audit_records_parses_items_and_cursor() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/workspaces/ws_1/audit")
                    .query_param("cursor", "p2");
                then.status(200).json_body(json!({
                    "requestId": "req_1",
                    "items": [
                        {
                            "id": "au_1",
                            "actorMatrixId": "@u:matrix.example.org",
                            "scope": "issues:write",
                            "repository": "octo/repo",
                            "operation": "create_issue",
                            "approvalId": "apr_1",
                            "commandId": "cmd_1",
                            "outcome": "completed",
                            "details": { "title": "Fix the bug" },
                            "createdAt": "2026-08-15T00:00:00.000Z"
                        }
                    ],
                    "nextCursor": "p3"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<AuditRecordItem> =
            client.list_audit_records("ws_1", Some("p2")).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "au_1");
        assert_eq!(page.items[0].operation.as_deref(), Some("create_issue"));
        assert_eq!(page.items[0].details["title"], "Fix the bug");
        assert_eq!(page.next_cursor.as_deref(), Some("p3"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_audit_records_without_cursor_has_no_query() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/workspaces/ws_1/audit");
                then.status(200)
                    .json_body(json!({ "requestId": "req_1", "items": [] }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<AuditRecordItem> =
            client.list_audit_records("ws_1", None).await.unwrap();
        assert!(page.items.is_empty());
        mock.assert_async().await;
    }
}
