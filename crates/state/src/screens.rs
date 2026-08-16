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
}
