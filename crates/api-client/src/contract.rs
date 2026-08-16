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

/// POST /api/rooms/:roomId/binding response (mirrors RoomBinding).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomBinding {
    pub room_id: String,
    pub workspace_id: String,
}

/// POST /api/rooms/:roomId/binding request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindRoomRequest {
    pub workspace_id: String,
}

/// Run execution mode (mirrors RunRequest.mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    Parallel,
    Sequential,
}

/// Launch body minus the idempotency key (mirrors RunRequest in packages/contracts/src/run.ts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub prompt: String,
    pub mode: RunMode,
    pub specialist_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_context: Option<GithubContext>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubContext {
    pub repository: String,
}

/// Run lifecycle status (mirrors RunResponse.status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
    Partial,
}

/// POST /api/workspaces/:workspaceId/runs response (mirrors RunResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    pub run_id: String,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    pub next_sequence: u64,
}

/// POST /api/runs/:runId/cancel response (mirrors CancellationResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancellationResponse {
    pub run_id: String,
    pub status: CancellationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationStatus {
    CancellationRequested,
}

/// Authoritative Matrix delivery status from GET /api/runs/:runId
/// (mirrors MatrixDeliveryStatus). Never inferred from the event stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatrixDeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDelivery {
    pub sequence: u64,
    pub status: MatrixDeliveryStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMatrixDeliveriesResponse {
    pub run_id: String,
    pub deliveries: Vec<MatrixDelivery>,
}

/// Paginated GitHub read result (mirrors GithubPage<T>).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Mirrors GithubRepositorySummary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepositorySummary {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub private: bool,
    pub default_branch: String,
    pub description: Option<String>,
    pub html_url: String,
    pub archived: bool,
}
