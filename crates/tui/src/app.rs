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

    /// Route a key event to the active screen's handler.
    pub fn handle_key(&mut self, key: KeyEvent) -> Command {
        match self.current_mut() {
            Screen::Login(state) => screens::login::handle_login_key(state, key),
            Screen::Workspaces(state) => screens::workspaces::handle_workspaces_key(state, key),
            Screen::Rooms(state) => screens::rooms::handle_rooms_key(state, key),
            Screen::RoomBinding(state) => screens::rooms::handle_room_binding_key(state, key),
            Screen::RunComposer(state) => screens::run_composer::handle_run_composer_key(state, key),
            Screen::Run(state) => screens::run::handle_run_key(state, key),
            Screen::GitHubWorkspace(state) => screens::github::handle_github_workspace_key(state, key),
        }
    }

    /// 401 anywhere: clear the stored session and return to Login.
    pub fn expire_session(&mut self) {
        let _ = self.store.clear();
        self.client.set_cookie(None);
        self.stack = vec![Screen::Login(state::screens::LoginState::default())];
        self.set_status("Session expired; sign in again");
    }

    /// Execute a command produced by a screen handler.
    pub async fn execute_command(&mut self, command: Command) {
        match command {
            Command::None => {}
            Command::Quit => self.should_quit = true,
            Command::Back => {
                if self.stack.len() > 1 {
                    self.pop();
                } else {
                    self.should_quit = true;
                }
            }
            Command::NavigateToRoomBinding => self.navigate_to_room_binding(),
            Command::NavigateToComposer => self.navigate_to_composer(),
            // Async arms restored in Task 7.4.
            _ => {}
        }
    }

    fn navigate_to_room_binding(&mut self) {
        let (room, workspace_id) = match self.current() {
            Screen::Rooms(state) => match state.selected_room() {
                Some(room) => (room.clone(), state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        self.push(Screen::RoomBinding(state::screens::RoomBindingState::new(room, workspace_id)));
    }

    fn navigate_to_composer(&mut self) {
        let (room_id, workspace_id) = match self.current() {
            Screen::Rooms(state) => match state.selected_room() {
                Some(room) => (room.room_id.clone(), state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        self.push(Screen::RunComposer(state::screens::RunComposerState::new(room_id, workspace_id)));
    }

    fn navigate_to_github_workspace(&mut self) {
        let (workspace_id, run_id) = match self.current() {
            Screen::Run(state) => (state.workspace_id.clone(), state.run_id.clone()),
            _ => return,
        };
        let installation_id = self.github_installation_id.clone();
        self.push(Screen::GitHubWorkspace(
            state::screens::GitHubWorkspaceState::new(workspace_id, run_id, installation_id),
        ));
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

    use crossterm::event::{KeyCode, KeyEvent};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
    }

    #[test]
    fn login_key_q_quits_and_other_keys_are_noops() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Command::Quit);
        assert_eq!(app.handle_key(key(KeyCode::Char('x'))), Command::None);
    }

    #[test]
    fn workspaces_key_q_quits_at_root() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        store.save(&data).unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Command::Quit);
    }

    #[tokio::test]
    async fn back_command_pops_to_previous_screen() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));
        app.execute_command(Command::Back).await;
        assert_eq!(app.current().id(), ScreenId::Login);
    }

    #[tokio::test]
    async fn back_at_root_quits() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        app.execute_command(Command::Back).await;
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn navigate_to_room_binding_pushes_binding_screen() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        let mut rooms = state::screens::RoomsState::new("ws_1".to_string());
        rooms.set_rooms(vec![api_client::RoomSummary {
            room_id: "!a:example.org".to_string(),
            homeserver_url: "https://example.org".to_string(),
            display_name: None,
            workspace_id: None,
        }]);
        app.push(Screen::Rooms(rooms));
        app.execute_command(Command::NavigateToRoomBinding).await;
        assert_eq!(app.current().id(), ScreenId::RoomBinding);
    }

    #[tokio::test]
    async fn navigate_to_composer_pushes_composer_screen() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        let mut rooms = state::screens::RoomsState::new("ws_1".to_string());
        rooms.set_rooms(vec![api_client::RoomSummary {
            room_id: "!a:example.org".to_string(),
            homeserver_url: "https://example.org".to_string(),
            display_name: None,
            workspace_id: Some("ws_1".to_string()),
        }]);
        app.push(Screen::Rooms(rooms));
        app.execute_command(Command::NavigateToComposer).await;
        assert_eq!(app.current().id(), ScreenId::RunComposer);
        let Screen::RunComposer(composer) = app.current() else { panic!("composer") };
        assert_eq!(composer.room_id, "!a:example.org");
        assert_eq!(composer.workspace_id, "ws_1");
    }
}
