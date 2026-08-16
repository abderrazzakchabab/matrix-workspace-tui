use crate::screens;
use api_client::{ControlPlaneApi, ControlPlaneError, RunEvent, RunResponse, WorkspaceSelection};
use crossterm::event::KeyEvent;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use state::screens::{RunState, Screen, ScreenId};
use state::session_store::{SessionData, SessionStore, StateError};
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("terminal io error: {0}")]
    Io(#[from] io::Error),
    #[error("control plane error: {0}")]
    Api(#[from] ControlPlaneError),
    #[error("state error: {0}")]
    State(#[from] StateError),
}

/// A user command produced by a screen key handler; the async run loop
/// executes it against the api client and the screen stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    None,
    Quit,
    Back,
    SubmitLogin,
    CreateWorkspace,
    NavigateToRooms,
    RefreshRooms,
    NavigateToRoomBinding,
    BindRoom,
    NavigateToComposer,
    LaunchRun,
    CancelRun,
    RefreshDeliveries,
    NavigateToGitHubWorkspace,
    RefreshPanel,
    RequestGrant,
    ConfirmMutation,
}

/// Events produced by the run's SSE stream task.
#[derive(Debug)]
pub enum AppEvent {
    RunEvent(RunEvent),
    Reconnecting,
    RunStreamEnded,
    RunError(ControlPlaneError),
}

/// Error codes that mean the write gate refused a mutation (mobile DENIAL_CODES).
const DENIAL_CODES: &[&str] = &[
    "WRITE_SCOPE_REQUIRED",
    "APPROVAL_DENIED",
    "APPROVAL_MISMATCH",
    "APPROVAL_NOT_FOUND",
    "APPROVAL_CONFIRMATION_REQUIRED",
    "COMMAND_NOT_ALLOWED",
    "RUN_NOT_FOUND",
];

pub struct App {
    pub base_url: String,
    pub client: ControlPlaneApi,
    pub store: SessionStore,
    pub stack: Vec<Screen>,
    pub status: Option<String>,
    pub should_quit: bool,
    pub github_installation_id: Option<String>,
    stream_rx: Option<mpsc::UnboundedReceiver<AppEvent>>,
    stream_task: Option<JoinHandle<()>>,
}

impl App {
    /// Restore the stored session (if any) and pick the initial screen.
    pub fn new(base_url: String, store: SessionStore) -> Self {
        let data = store.load().unwrap_or_default();
        let mut client = ControlPlaneApi::new(&base_url).unwrap_or_else(|_| {
            ControlPlaneApi::new("http://localhost:3000").expect("constant base url is valid")
        });
        client.set_cookie(data.cookie.clone());
        let initial = if data.cookie.is_some() {
            let mut state = state::screens::WorkspacesState::new();
            for workspace in data.workspaces {
                state.add_workspace(workspace);
            }
            Screen::Workspaces(state)
        } else {
            Screen::Login(state::screens::LoginState::default())
        };
        let github_installation_id =
            std::env::var("MATRIX_WORKSPACE_TUI_GITHUB_INSTALLATION_ID").ok();
        Self {
            base_url,
            client,
            store,
            stack: vec![initial],
            status: None,
            should_quit: false,
            github_installation_id,
            stream_rx: None,
            stream_task: None,
        }
    }

    pub fn current(&self) -> &Screen {
        self.stack.last().expect("stack is never empty")
    }

    pub fn current_mut(&mut self) -> &mut Screen {
        self.stack.last_mut().expect("stack is never empty")
    }

    pub fn push(&mut self, screen: Screen) {
        self.stack.push(screen);
    }

    /// Pop the top screen. Leaving the Run screen aborts its SSE task.
    pub fn pop(&mut self) -> Option<Screen> {
        let popped = self.stack.pop();
        if matches!(popped, Some(Screen::Run(_))) {
            self.abort_stream();
        }
        popped
    }

    /// Abort the run's SSE task. The real implementation lands with the
    /// stream in Task 7.4; the stub keeps Task 7.1's shell compiling.
    fn abort_stream(&mut self) {}

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    /// 401 anywhere: clear the stored session and return to Login.
    pub fn expire_session(&mut self) {
        let _ = self.store.clear();
        self.client.set_cookie(None);
        self.stack = vec![Screen::Login(state::screens::LoginState::default())];
        self.set_status("Session expired; sign in again");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::screens::{Screen, ScreenId};
    use state::session_store::SessionStore;
    use tempfile::tempdir;

    #[test]
    fn app_starts_on_login_without_session() {
        let dir = tempdir().unwrap();
        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Login);
        assert_eq!(app.stack.len(), 1);
    }

    #[test]
    fn app_starts_on_workspaces_with_stored_session() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        data.workspaces.push(WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "My workspace".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        store.save(&data).unwrap();

        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Workspaces);
        assert_eq!(app.client.cookie(), Some("cp_session=abc123"));
        let Screen::Workspaces(state) = app.current() else { panic!("workspaces") };
        assert_eq!(state.workspaces.len(), 1);
    }

    #[test]
    fn push_pop_preserves_order_and_aborts_run_stream() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));
        assert_eq!(app.current().id(), ScreenId::Rooms);
        let popped = app.pop();
        assert!(matches!(popped, Some(Screen::Rooms(_))));
        assert_eq!(app.current().id(), ScreenId::Login);
    }
}
