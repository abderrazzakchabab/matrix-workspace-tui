use api_client::{
    AuditRecordItem, GithubMutationOperation, GithubPullRequestSummary, GithubRepositorySummary,
    GithubWriteGrantResult, GithubWriteScope, MatrixDelivery, RunEvent, RunMode, RunRequest,
    RoomSummary, WorkspaceSelection,
};
use api_client::sse::RunEventBuffer;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Specialist options offered by the composer (mirrors the mobile
/// navigator's `specialists` list).
pub const SPECIALISTS: &[(&str, &str)] = &[
    ("repo-reader", "Repository reader"),
    ("issue-reader", "Issue reader"),
    ("pr-reader", "Pull Request reader"),
];

/// Which login field receives typed characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LoginField {
    #[default]
    HomeserverUrl,
    AccessToken,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoginState {
    pub homeserver_url: String,
    pub access_token: String,
    pub focus: LoginField,
    pub error: Option<String>,
    pub submitting: bool,
}

impl LoginState {
    pub fn set_homeserver_url(&mut self, value: String) {
        self.homeserver_url = value;
        self.error = None;
    }

    pub fn set_access_token(&mut self, value: String) {
        self.access_token = value;
        self.error = None;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            LoginField::HomeserverUrl => LoginField::AccessToken,
            LoginField::AccessToken => LoginField::HomeserverUrl,
        };
    }

    /// Insert one character into the focused field.
    pub fn insert_char(&mut self, c: char) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.push(c);
        self.error = None;
    }

    /// Append a whole string (bracketed paste) into the focused field.
    pub fn insert_text(&mut self, text: &str) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.push_str(text);
        self.error = None;
    }

    pub fn backspace(&mut self) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.pop();
        self.error = None;
    }

    pub fn validation_error(&self) -> Option<String> {
        let url = self.homeserver_url.trim();
        if url.is_empty() {
            return Some("Homeserver URL is required".to_string());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Some("Homeserver URL must start with http:// or https://".to_string());
        }
        if self.access_token.trim().is_empty() {
            return Some("Access token is required".to_string());
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspacesState {
    pub workspaces: Vec<WorkspaceSelection>,
    pub selected: usize,
    pub error: Option<String>,
    pub creating: bool,
    pub name_input: String,
}

impl WorkspacesState {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            selected: 0,
            error: None,
            creating: false,
            name_input: String::new(),
        }
    }

    pub fn add_workspace(&mut self, workspace: WorkspaceSelection) {
        self.workspaces.push(workspace);
    }

    pub fn selected(&self) -> Option<&WorkspaceSelection> {
        self.workspaces.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.workspaces.is_empty() && self.selected + 1 < self.workspaces.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn set_name_input(&mut self, value: String) {
        self.name_input = value;
        self.error = None;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoomsState {
    pub rooms: Vec<RoomSummary>,
    pub workspace_id: String,
    pub selected: usize,
    pub error: Option<String>,
    pub loading: bool,
}

impl RoomsState {
    pub fn new(workspace_id: String) -> Self {
        Self {
            rooms: Vec::new(),
            workspace_id,
            selected: 0,
            error: None,
            loading: false,
        }
    }

    pub fn set_rooms(&mut self, rooms: Vec<RoomSummary>) {
        self.rooms = rooms;
        if !self.rooms.is_empty() && self.selected >= self.rooms.len() {
            self.selected = self.rooms.len() - 1;
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_room(&self) -> Option<&RoomSummary> {
        self.rooms.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.rooms.is_empty() && self.selected + 1 < self.rooms.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// True when the selected room is bound to this screen's workspace.
    pub fn room_is_bound_to_workspace(&self) -> bool {
        matches!(
            self.selected_room(),
            Some(room) if room.workspace_id.as_deref() == Some(self.workspace_id.as_str())
        )
    }

    /// Reflect a successful bind (POST binding) without refetching.
    pub fn mark_room_bound(&mut self, room_id: &str) {
        for room in &mut self.rooms {
            if room.room_id == room_id {
                room.workspace_id = Some(self.workspace_id.clone());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoomBindingState {
    pub room: RoomSummary,
    pub workspace_id: String,
    pub error: Option<String>,
    pub binding: bool,
    /// Set when the bind succeeded; the TUI pops back to Rooms on seeing it.
    pub done: bool,
}

impl RoomBindingState {
    pub fn new(room: RoomSummary, workspace_id: String) -> Self {
        Self {
            room,
            workspace_id,
            error: None,
            binding: false,
            done: false,
        }
    }

    pub fn mark_bound(&mut self) {
        self.done = true;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunComposerState {
    pub prompt: String,
    pub mode: Option<RunMode>,
    pub selected_specialists: Vec<String>,
    pub room_id: String,
    pub workspace_id: String,
    /// Index into `SPECIALISTS` for space-to-toggle selection.
    pub specialist_cursor: usize,
    pub error: Option<String>,
    pub launching: bool,
}

impl RunComposerState {
    pub fn new(room_id: String, workspace_id: String) -> Self {
        Self {
            prompt: String::new(),
            mode: None,
            selected_specialists: Vec::new(),
            room_id,
            workspace_id,
            specialist_cursor: 0,
            error: None,
            launching: false,
        }
    }

    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = prompt;
        self.error = None;
    }

    pub fn toggle_mode(&mut self, mode: RunMode) {
        self.mode = Some(mode);
    }

    pub fn toggle_specialist(&mut self, id: &str) {
        if let Some(position) = self.selected_specialists.iter().position(|value| value == id) {
            self.selected_specialists.remove(position);
        } else {
            self.selected_specialists.push(id.to_string());
        }
    }

    pub fn move_specialist_cursor_next(&mut self) {
        if self.specialist_cursor + 1 < SPECIALISTS.len() {
            self.specialist_cursor += 1;
        }
    }

    pub fn move_specialist_cursor_prev(&mut self) {
        self.specialist_cursor = self.specialist_cursor.saturating_sub(1);
    }

    pub fn toggle_specialist_at_cursor(&mut self) {
        if let Some((id, _)) = SPECIALISTS.get(self.specialist_cursor) {
            self.toggle_specialist(id);
        }
    }

    pub fn validation_error(&self) -> Option<String> {
        if self.prompt.trim().is_empty() {
            return Some("Prompt is required".to_string());
        }
        if self.mode.is_none() {
            return Some("Choose a mode (parallel or sequential)".to_string());
        }
        if self.selected_specialists.is_empty() {
            return Some("Select at least one specialist".to_string());
        }
        None
    }

    /// The validated launch request, or None when the form is invalid.
    pub fn request(&self) -> Option<RunRequest> {
        if self.validation_error().is_some() {
            return None;
        }
        Some(RunRequest {
            prompt: self.prompt.trim().to_string(),
            mode: self.mode.unwrap(),
            specialist_ids: self.selected_specialists.clone(),
            room_id: Some(self.room_id.clone()),
            github_context: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunState {
    pub run_id: String,
    pub workspace_id: String,
    pub buffer: RunEventBuffer,
    pub deliveries: Vec<MatrixDelivery>,
    pub cancel_requested: bool,
    pub reconnecting: bool,
    pub error: Option<String>,
}

impl RunState {
    pub fn new(run_id: String, workspace_id: String) -> Self {
        Self {
            run_id,
            workspace_id,
            buffer: RunEventBuffer::new(),
            deliveries: Vec::new(),
            cancel_requested: false,
            reconnecting: false,
            error: None,
        }
    }

    pub fn accept(&mut self, event: RunEvent) -> bool {
        self.buffer.accept(event)
    }

    pub fn is_terminal(&self) -> bool {
        self.buffer.is_terminal()
    }

    pub fn events(&self) -> &[RunEvent] {
        self.buffer.events()
    }

    pub fn highest_sequence(&self) -> u64 {
        self.buffer.highest_sequence()
    }

    pub fn set_reconnecting(&mut self, value: bool) {
        self.reconnecting = value;
    }

    pub fn set_deliveries(&mut self, deliveries: Vec<MatrixDelivery>) {
        self.deliveries = deliveries;
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubPanel {
    Repositories,
    Issues,
    PullRequests,
    Audit,
}

/// The mutation flow status mirror (mobile MutationConfirmationStatus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationFlowStatus {
    Idle,
    Submitting,
    Submitted,
    Succeeded,
    Denied,
    Expired,
    Failed,
    Duplicate,
}

/// Everything shown on the explicit confirmation screen before enqueue.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationConfirmationDraft {
    pub operation: GithubMutationOperation,
    pub repository: String,
    pub arguments: serde_json::Value,
    pub scope: GithubWriteScope,
    pub idempotency_key: String,
    pub command_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitHubWorkspaceState {
    pub workspace_id: String,
    pub run_id: String,
    pub installation_id: Option<String>,
    pub panel: GithubPanel,
    pub repositories: Vec<GithubRepositorySummary>,
    pub issues: Vec<api_client::GithubIssueSummary>,
    pub pulls: Vec<GithubPullRequestSummary>,
    pub audit: Vec<AuditRecordItem>,
    pub selected_index: usize,
    pub error: Option<String>,
    pub loading: bool,
    pub grant: Option<GithubWriteGrantResult>,
    pub mutation_title: String,
    pub mutation_mode: bool,
    pub confirmation: Option<MutationConfirmationDraft>,
    pub mutation_status: MutationFlowStatus,
    pub command_id: Option<String>,
}

impl GitHubWorkspaceState {
    pub fn new(workspace_id: String, run_id: String, installation_id: Option<String>) -> Self {
        Self {
            workspace_id,
            run_id,
            installation_id,
            panel: GithubPanel::Repositories,
            repositories: Vec::new(),
            issues: Vec::new(),
            pulls: Vec::new(),
            audit: Vec::new(),
            selected_index: 0,
            error: None,
            loading: false,
            grant: None,
            mutation_title: String::new(),
            mutation_mode: false,
            confirmation: None,
            mutation_status: MutationFlowStatus::Idle,
            command_id: None,
        }
    }

    pub fn switch_panel(&mut self, panel: GithubPanel) {
        self.panel = panel;
        self.selected_index = 0;
    }

    pub fn set_repositories(&mut self, repositories: Vec<GithubRepositorySummary>) {
        self.repositories = repositories;
        self.clamp_selection();
    }

    pub fn set_issues(&mut self, issues: Vec<api_client::GithubIssueSummary>) {
        self.issues = issues;
        self.clamp_selection();
    }

    pub fn set_pull_requests(&mut self, pulls: Vec<GithubPullRequestSummary>) {
        self.pulls = pulls;
        self.clamp_selection();
    }

    pub fn set_audit(&mut self, audit: Vec<AuditRecordItem>) {
        self.audit = audit;
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let len = self.panel_items_len();
        if len > 0 && self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    fn panel_items_len(&self) -> usize {
        match self.panel {
            GithubPanel::Repositories => self.repositories.len(),
            GithubPanel::Issues => self.issues.len(),
            GithubPanel::PullRequests => self.pulls.len(),
            GithubPanel::Audit => self.audit.len(),
        }
    }

    pub fn select_next(&mut self) {
        let len = self.panel_items_len();
        if len > 0 && self.selected_index + 1 < len {
            self.selected_index += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// The currently selected repository (its full `owner/repo` name).
    pub fn selected_repository(&self) -> Option<String> {
        self.repositories
            .get(self.selected_index)
            .map(|repo| repo.full_name.clone())
    }

    pub fn set_grant(&mut self, grant: GithubWriteGrantResult) {
        self.grant = Some(grant);
    }

    pub fn set_mutation_title(&mut self, title: String) {
        self.mutation_title = title;
        self.error = None;
    }

    pub fn set_mutation_status(&mut self, status: MutationFlowStatus) {
        self.mutation_status = status;
    }

    pub fn set_command_id(&mut self, command_id: Option<String>) {
        self.command_id = command_id;
    }

    /// Build the explicit confirmation draft. Requires a selected repository
    /// and a non-empty title. The operation is always `create_issue` with
    /// scope `issues:write` (the mobile's WRITE_SCOPE/OPERATION constants).
    pub fn begin_mutation(&mut self, title: String) -> Option<MutationConfirmationDraft> {
        let repository = self.selected_repository()?;
        if title.trim().is_empty() {
            return None;
        }
        let operation = GithubMutationOperation::CreateIssue;
        let scope = GithubWriteScope::IssuesWrite;
        let arguments = serde_json::json!({ "title": title.trim() });
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        let command_hash = command_hash(operation, &arguments);
        Some(MutationConfirmationDraft {
            operation,
            repository,
            arguments,
            scope,
            idempotency_key,
            command_hash,
        })
    }
}

/// The exact confirmation sentence the mobile sends as `confirmationText`
/// (mirrors confirmationSentence in
/// apps/mobile/src/components/MutationConfirmation.tsx).
pub fn confirmation_sentence(
    operation: GithubMutationOperation,
    repository: &str,
    scope: GithubWriteScope,
) -> String {
    let label = match operation {
        GithubMutationOperation::CreateIssue => "create issue",
        GithubMutationOperation::UpdateIssue => "update issue",
        GithubMutationOperation::CommentIssue => "comment on issue",
        GithubMutationOperation::CreatePrComment => "comment on pull request",
    };
    let scope_name = match scope {
        GithubWriteScope::IssuesWrite => "issues:write",
        GithubWriteScope::PullRequestsWrite => "pull_requests:write",
    };
    format!("I confirm {label} on {repository} ({scope_name})")
}

/// Recursively sort object keys; must match the control-plane
/// canonicalization (mutation-command.ts `canonicalize`).
pub fn canonicalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize).collect())
        }
        serde_json::Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in map {
                sorted.insert(key.clone(), canonicalize(value));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        other => other.clone(),
    }
}

fn operation_name(operation: GithubMutationOperation) -> &'static str {
    match operation {
        GithubMutationOperation::CreateIssue => "create_issue",
        GithubMutationOperation::UpdateIssue => "update_issue",
        GithubMutationOperation::CommentIssue => "comment_issue",
        GithubMutationOperation::CreatePrComment => "create_pr_comment",
    }
}

/// SHA-256 of the canonical `{"operation": ..., "arguments": ...}` JSON —
/// byte-identical to the server's computeCommandHash.
pub fn command_hash(
    operation: GithubMutationOperation,
    arguments: &serde_json::Value,
) -> String {
    let value = canonicalize(&serde_json::json!({
        "operation": operation_name(operation),
        "arguments": arguments,
    }));
    let canonical = serde_json::to_string(&value).expect("command is serializable");
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Login,
    Workspaces,
    Rooms,
    RoomBinding,
    RunComposer,
    Run,
    GitHubWorkspace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Login(LoginState),
    Workspaces(WorkspacesState),
    Rooms(RoomsState),
    RoomBinding(RoomBindingState),
    RunComposer(RunComposerState),
    Run(RunState),
    GitHubWorkspace(GitHubWorkspaceState),
}

impl Screen {
    pub fn id(&self) -> ScreenId {
        match self {
            Screen::Login(_) => ScreenId::Login,
            Screen::Workspaces(_) => ScreenId::Workspaces,
            Screen::Rooms(_) => ScreenId::Rooms,
            Screen::RoomBinding(_) => ScreenId::RoomBinding,
            Screen::RunComposer(_) => ScreenId::RunComposer,
            Screen::Run(_) => ScreenId::Run,
            Screen::GitHubWorkspace(_) => ScreenId::GitHubWorkspace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_client::WorkspaceSelection;

    fn workspace(id: &str) -> WorkspaceSelection {
        WorkspaceSelection {
            workspace_id: id.to_string(),
            name: format!("ws {id}"),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn login_state_defaults_empty_and_validates() {
        let mut state = LoginState::default();
        assert_eq!(state.validation_error().as_deref(), Some("Homeserver URL is required"));
        state.set_homeserver_url("https://matrix.example.org".to_string());
        assert_eq!(state.validation_error().as_deref(), Some("Access token is required"));
        state.set_access_token("tok".to_string());
        assert!(state.validation_error().is_none());
    }

    #[test]
    fn login_state_rejects_non_http_urls() {
        let mut state = LoginState::default();
        state.set_homeserver_url("matrix.example.org".to_string());
        state.set_access_token("tok".to_string());
        assert_eq!(
            state.validation_error().as_deref(),
            Some("Homeserver URL must start with http:// or https://")
        );
    }

    #[test]
    fn login_state_editing_clears_previous_error() {
        let mut state = LoginState::default();
        state.error = Some("boom".to_string());
        state.set_homeserver_url("https://matrix.example.org".to_string());
        assert_eq!(state.error, None);
    }

    #[test]
    fn login_state_edits_the_focused_field() {
        let mut state = LoginState::default();
        state.insert_char('h');
        assert_eq!(state.homeserver_url, "h");
        state.toggle_focus();
        state.insert_text("tok_1");
        assert_eq!(state.access_token, "tok_1");
        state.backspace();
        assert_eq!(state.access_token, "tok_");
        state.toggle_focus();
        state.backspace();
        assert_eq!(state.homeserver_url, "");
    }

    #[test]
    fn workspaces_state_adds_and_selects() {
        let mut state = WorkspacesState::new();
        assert!(state.selected().is_none());
        state.add_workspace(workspace("ws_1"));
        state.add_workspace(workspace("ws_2"));
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
        state.select_next();
        assert_eq!(state.selected().unwrap().workspace_id, "ws_2");
        state.select_next(); // clamps at the end
        assert_eq!(state.selected().unwrap().workspace_id, "ws_2");
        state.select_prev();
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
        state.select_prev(); // clamps at the start
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
    }

    fn room(id: &str, workspace_id: Option<&str>) -> RoomSummary {
        RoomSummary {
            room_id: id.to_string(),
            homeserver_url: "https://example.org".to_string(),
            display_name: Some(id.to_string()),
            workspace_id: workspace_id.map(|value| value.to_string()),
        }
    }

    #[test]
    fn rooms_state_tracks_selection_and_binding() {
        let mut state = RoomsState::new("ws_1".to_string());
        assert!(state.selected_room().is_none());
        state.set_rooms(vec![room("!a:example.org", Some("ws_1")), room("!b:example.org", None)]);
        assert!(state.room_is_bound_to_workspace(), "first room is bound to ws_1");
        state.select_next();
        assert!(!state.room_is_bound_to_workspace(), "second room is unbound");
        assert_eq!(state.selected_room().unwrap().room_id, "!b:example.org");
    }

    #[test]
    fn rooms_state_clamps_selection_when_list_shrinks() {
        let mut state = RoomsState::new("ws_1".to_string());
        state.set_rooms(vec![room("!a:example.org", None), room("!b:example.org", None)]);
        state.select_next();
        state.set_rooms(vec![room("!a:example.org", None)]);
        assert_eq!(state.selected(), 0);
        assert_eq!(state.selected_room().unwrap().room_id, "!a:example.org");
    }

    #[test]
    fn rooms_state_marks_binding_after_bind() {
        let mut state = RoomsState::new("ws_1".to_string());
        state.set_rooms(vec![room("!a:example.org", None)]);
        assert!(!state.room_is_bound_to_workspace());
        state.mark_room_bound("!a:example.org");
        assert!(state.room_is_bound_to_workspace());
    }

    #[test]
    fn room_binding_state_starts_pending_and_marks_bound() {
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert!(!state.done);
        assert_eq!(state.room.room_id, "!a:example.org");
        assert_eq!(state.workspace_id, "ws_1");
        state.mark_bound();
        assert!(state.done);
    }

    #[test]
    fn composer_requires_prompt_mode_and_specialists() {
        let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
        assert_eq!(state.validation_error().as_deref(), Some("Prompt is required"));
        state.set_prompt("Do the thing".to_string());
        assert_eq!(state.validation_error().as_deref(), Some("Choose a mode (parallel or sequential)"));
        state.toggle_mode(RunMode::Parallel);
        assert_eq!(state.validation_error().as_deref(), Some("Select at least one specialist"));
        state.toggle_specialist("pr-reader");
        assert!(state.validation_error().is_none());
    }

    #[test]
    fn composer_toggles_specialists_and_mode() {
        let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
        state.toggle_specialist("repo-reader");
        state.toggle_specialist("pr-reader");
        assert_eq!(state.selected_specialists, vec!["repo-reader", "pr-reader"]);
        state.toggle_specialist("repo-reader"); // toggle off
        assert_eq!(state.selected_specialists, vec!["pr-reader"]);
        state.toggle_mode(RunMode::Sequential);
        assert_eq!(state.mode, Some(RunMode::Sequential));
    }

    #[test]
    fn composer_request_requires_valid_input_and_carries_room_id() {
        let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
        assert!(state.request().is_none());
        state.set_prompt("  Do the thing  ".to_string());
        state.toggle_mode(RunMode::Parallel);
        state.toggle_specialist("repo-reader");
        let request = state.request().unwrap();
        assert_eq!(request.prompt, "Do the thing");
        assert_eq!(request.mode, RunMode::Parallel);
        assert_eq!(request.specialist_ids, vec!["repo-reader"]);
        assert_eq!(request.room_id.as_deref(), Some("!a:example.org"));
        assert_eq!(request.github_context, None);
        assert_eq!(state.workspace_id, "ws_1");
    }

    #[test]
    fn composer_moves_the_specialist_cursor_and_toggles_at_cursor() {
        let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
        assert_eq!(state.specialist_cursor, 0);
        state.move_specialist_cursor_next();
        state.move_specialist_cursor_next();
        assert_eq!(state.specialist_cursor, 2);
        state.move_specialist_cursor_next(); // clamps
        assert_eq!(state.specialist_cursor, 2);
        state.toggle_specialist_at_cursor();
        assert_eq!(state.selected_specialists, vec!["pr-reader"]);
        state.move_specialist_cursor_prev();
        assert_eq!(state.specialist_cursor, 1);
    }

    fn event(sequence: u64, event_type: api_client::RunEventType) -> RunEvent {
        RunEvent {
            id: format!("ev_{sequence}"),
            run_id: "r1".to_string(),
            sequence,
            event_type,
            version: 1,
            occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
            visibility: api_client::EventVisibility::RoomAndOwner,
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn run_state_accepts_events_and_detects_terminal() {
        let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
        assert_eq!(state.highest_sequence(), 0);
        assert!(state.accept(event(1, api_client::RunEventType::RunStarted)));
        assert!(!state.is_terminal());
        assert!(state.accept(event(2, api_client::RunEventType::RunCompleted)));
        assert!(state.is_terminal());
        assert!(!state.accept(event(3, api_client::RunEventType::RunStarted)), "post-terminal rejected");
        assert_eq!(state.events().len(), 2);
    }

    #[test]
    fn run_state_tracks_deliveries_cancel_and_reconnect() {
        let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
        state.set_reconnecting(true);
        assert!(state.reconnecting);
        state.set_deliveries(vec![
            MatrixDelivery { sequence: 1, status: api_client::MatrixDeliveryStatus::Delivered },
            MatrixDelivery { sequence: 2, status: api_client::MatrixDeliveryStatus::Pending },
        ]);
        assert_eq!(state.deliveries.len(), 2);
        assert_eq!(state.deliveries[1].status, api_client::MatrixDeliveryStatus::Pending);
        state.request_cancel();
        assert!(state.cancel_requested);
        assert_eq!(state.error, None);
    }

    #[test]
    fn github_state_starts_on_repositories_panel() {
        let state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
        assert_eq!(state.panel, GithubPanel::Repositories);
        assert_eq!(state.installation_id.as_deref(), Some("inst_9"));
    }

    #[test]
    fn github_state_switches_panels_and_clamps_selection() {
        let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
        state.set_repositories(vec![GithubRepositorySummary {
            id: 1,
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            owner: "octo".to_string(),
            private: false,
            default_branch: "main".to_string(),
            description: None,
            html_url: "https://github.com/octo/repo".to_string(),
            archived: false,
        }]);
        assert_eq!(state.selected_repository().as_deref(), Some("octo/repo"));
        state.switch_panel(GithubPanel::Audit);
        state.select_next(); // clamps to empty list
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn github_state_builds_mutation_confirmation_draft() {
        let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
        state.set_repositories(vec![GithubRepositorySummary {
            id: 1,
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            owner: "octo".to_string(),
            private: false,
            default_branch: "main".to_string(),
            description: None,
            html_url: "https://github.com/octo/repo".to_string(),
            archived: false,
        }]);
        let draft = state.begin_mutation("Test issue".to_string()).expect("draft");
        assert_eq!(draft.operation, GithubMutationOperation::CreateIssue);
        assert_eq!(draft.repository, "octo/repo");
        assert_eq!(draft.scope, GithubWriteScope::IssuesWrite);
        assert_eq!(draft.arguments["title"], "Test issue");
        assert!(!draft.idempotency_key.is_empty());
        assert_eq!(
            draft.command_hash,
            "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8",
            "must match the mobile/server canonical hash for this command"
        );
    }

    #[test]
    fn github_state_begin_mutation_requires_repository_and_title() {
        let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
        assert!(state.begin_mutation("Test issue".to_string()).is_none(), "no repository selected");
        state.set_repositories(vec![GithubRepositorySummary {
            id: 1,
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            owner: "octo".to_string(),
            private: false,
            default_branch: "main".to_string(),
            description: None,
            html_url: "https://github.com/octo/repo".to_string(),
            archived: false,
        }]);
        assert!(state.begin_mutation("   ".to_string()).is_none(), "empty title rejected");
    }

    #[test]
    fn confirmation_sentence_matches_mobile_format() {
        assert_eq!(
            confirmation_sentence(
                GithubMutationOperation::CreateIssue,
                "octo/repo",
                GithubWriteScope::IssuesWrite,
            ),
            "I confirm create issue on octo/repo (issues:write)"
        );
        assert_eq!(
            confirmation_sentence(
                GithubMutationOperation::CreatePrComment,
                "octo/repo",
                GithubWriteScope::PullRequestsWrite,
            ),
            "I confirm comment on pull request on octo/repo (pull_requests:write)"
        );
    }

    #[test]
    fn canonicalize_sorts_keys_recursively() {
        let value = serde_json::json!({
            "operation": "create_issue",
            "arguments": { "title": "x", "body": "y" },
        });
        let canonical = canonicalize(&value);
        assert_eq!(
            serde_json::to_string(&canonical).unwrap(),
            r#"{"arguments":{"body":"y","title":"x"},"operation":"create_issue"}"#
        );
    }

    #[test]
    fn command_hash_matches_the_control_plane_vector() {
        let hash = command_hash(
            GithubMutationOperation::CreateIssue,
            &serde_json::json!({ "title": "Test issue" }),
        );
        assert_eq!(hash, "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8");

        let hash_with_body = command_hash(
            GithubMutationOperation::CreateIssue,
            &serde_json::json!({ "body": "Details", "title": "Test issue" }),
        );
        assert_eq!(hash_with_body, "8c8a0ab437a3a0c5760a8179ab81bcc9b84b31878cf2dede2888c63fa8b4d2b9");
    }

    #[test]
    fn every_screen_reports_its_id() {
        assert_eq!(Screen::Login(LoginState::default()).id(), ScreenId::Login);
        assert_eq!(Screen::Workspaces(WorkspacesState::new()).id(), ScreenId::Workspaces);
        assert_eq!(Screen::Rooms(RoomsState::new("ws_1".to_string())).id(), ScreenId::Rooms);
        assert_eq!(
            Screen::RoomBinding(RoomBindingState::new(
                room("!a:example.org", None),
                "ws_1".to_string(),
            ))
            .id(),
            ScreenId::RoomBinding
        );
        assert_eq!(Screen::RunComposer(RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string())).id(), ScreenId::RunComposer);
        assert_eq!(Screen::Run(RunState::new("r1".to_string(), "ws_1".to_string())).id(), ScreenId::Run);
        assert_eq!(
            Screen::GitHubWorkspace(GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None)).id(),
            ScreenId::GitHubWorkspace
        );
    }
}
