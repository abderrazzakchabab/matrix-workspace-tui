use crate::contract::RoomSummary;
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;

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
}
