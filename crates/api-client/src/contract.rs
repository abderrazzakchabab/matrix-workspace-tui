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
