use crate::contract::RoomSummary;
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;
use serde_json::json;

impl ControlPlaneApi {
    /// GET /api/rooms — list joined rooms with their workspace bindings.
    pub async fn get_rooms(&self) -> Result<Vec<RoomSummary>, ControlPlaneError> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RoomsResponse {
            rooms: Vec<RoomSummary>,
        }
        let body: RoomsResponse =
            self.authenticated_request(reqwest::Method::GET, "/api/rooms", None).await?;
        Ok(body.rooms)
    }
}

impl ControlPlaneApi {
    /// POST /api/rooms/:roomId/binding — bind a room to a workspace.
    pub async fn bind_room(
        &self,
        room_id: &str,
        workspace_id: &str,
    ) -> Result<crate::contract::RoomBinding, ControlPlaneError> {
        use crate::contract::BindRoomRequest;
        let body = json!(BindRoomRequest {
            workspace_id: workspace_id.to_string(),
        });
        let path = format!("/api/rooms/{}/binding", crate::http::urlencode(room_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn get_rooms_returns_the_rooms_array() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/rooms").header("cookie", "cp_session=abc123");
                then.status(200).json_body(json!({
                    "requestId": "req_1",
                    "rooms": [
                        {
                            "roomId": "!a:matrix.example.org",
                            "homeserverUrl": "https://matrix.example.org",
                            "displayName": "Engineering",
                            "workspaceId": "ws_1"
                        },
                        {
                            "roomId": "!b:matrix.example.org",
                            "homeserverUrl": "https://matrix.example.org",
                            "displayName": null,
                            "workspaceId": null
                        }
                    ]
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let rooms: Vec<RoomSummary> = client.get_rooms().await.unwrap();

        assert_eq!(rooms.len(), 2);
        assert_eq!(rooms[0].room_id, "!a:matrix.example.org");
        assert_eq!(rooms[0].display_name.as_deref(), Some("Engineering"));
        assert_eq!(rooms[0].workspace_id.as_deref(), Some("ws_1"));
        assert_eq!(rooms[1].workspace_id, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn bind_room_posts_workspace_id_and_returns_binding() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/rooms/%21a%3Amatrix.example.org/binding")
                    .body_contains(r#""workspaceId":"ws_1""#);
                then.status(200).json_body(json!({
                    "roomId": "!a:matrix.example.org",
                    "workspaceId": "ws_1"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let binding: crate::contract::RoomBinding = client
            .bind_room("!a:matrix.example.org", "ws_1")
            .await
            .unwrap();

        assert_eq!(binding.room_id, "!a:matrix.example.org");
        assert_eq!(binding.workspace_id, "ws_1");
        mock.assert_async().await;
    }
}
