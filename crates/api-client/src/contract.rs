use serde::{Deserialize, Serialize};

/// POST /api/auth/matrix/session response (mirrors MatrixSessionResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionResponse {
    pub user: MatrixUser,
    pub session_expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixUser {
    pub id: String,
    pub homeserver_url: String,
}

/// POST /api/auth/matrix/session request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionRequest {
    pub homeserver_url: String,
    pub access_token: String,
}

/// POST /api/workspaces response (mirrors WorkspaceSelection).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSelection {
    pub workspace_id: String,
    pub name: String,
    pub owner_id: String,
    pub status: String,
    pub created_at: String,
}

/// POST /api/workspaces request body. The policy mirrors what the mobile app
/// always sends (read-only runs, partial failure, fail run on prompt injection).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub policy: WorkspacePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicy {
    pub read_only: bool,
    pub failure_policy: String,
    pub prompt_injection_mode: String,
}

/// GET /api/rooms item (mirrors RoomSummary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomSummary {
    pub room_id: String,
    pub homeserver_url: String,
    pub display_name: Option<String>,
    pub workspace_id: Option<String>,
}
