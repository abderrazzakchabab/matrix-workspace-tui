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
}
