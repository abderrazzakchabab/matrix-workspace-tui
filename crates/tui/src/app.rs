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
            Command::NavigateToRooms => self.navigate_to_rooms().await,
            Command::NavigateToGitHubWorkspace => self.navigate_to_github_workspace(),
            Command::SubmitLogin => self.submit_login().await,
            Command::CreateWorkspace => self.create_workspace().await,
            Command::RefreshRooms => self.refresh_rooms().await,
            Command::BindRoom => self.bind_room().await,
            Command::LaunchRun => self.launch_run().await,
            // Command::CancelRun => self.cancel_run().await,
            // Command::RefreshDeliveries => self.refresh_deliveries().await,
            // Command::RefreshPanel => self.refresh_github_panel().await,
            // Command::RequestGrant => self.request_grant().await,
            // Command::ConfirmMutation => self.confirm_mutation().await,
            // Async arms restored in Task 7.5.
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

    async fn submit_login(&mut self) {
        let (homeserver_url, access_token) = match self.current() {
            Screen::Login(state) => (
                state.homeserver_url.trim().to_string(),
                state.access_token.trim().to_string(),
            ),
            _ => return,
        };
        match self.client.create_matrix_session(&homeserver_url, &access_token).await {
            Ok(_) => {
                let cookie = self.client.cookie().map(|value| value.to_string());
                let data = SessionData {
                    cookie,
                    workspaces: Vec::new(),
                };
                if let Err(error) = self.store.save(&data) {
                    self.set_status(format!("Could not save session: {error}"));
                    return;
                }
                self.push(Screen::Workspaces(state::screens::WorkspacesState::new()));
                self.set_status("Signed in");
            }
            Err(error) => {
                if let Screen::Login(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn create_workspace(&mut self) {
        let name = match self.current() {
            Screen::Workspaces(state) => state.name_input.trim().to_string(),
            _ => return,
        };
        if name.is_empty() {
            self.set_status("Workspace name is required");
            return;
        }
        match self.client.create_workspace(&name).await {
            Ok(workspace) => {
                if let Screen::Workspaces(state) = self.current_mut() {
                    state.add_workspace(workspace.clone());
                    state.creating = false;
                    state.set_name_input(String::new());
                }
                let mut data = self.store.load().unwrap_or_default();
                data.workspaces.push(workspace);
                match self.store.save(&data) {
                    Ok(()) => self.set_status("Workspace created"),
                    Err(error) => self.set_status(format!("Could not persist workspace: {error}")),
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Workspaces(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn navigate_to_rooms(&mut self) {
        let workspace_id = match self.current() {
            Screen::Workspaces(state) => state.selected().map(|workspace| workspace.workspace_id.clone()),
            _ => None,
        };
        let Some(workspace_id) = workspace_id else {
            return;
        };
        self.push(Screen::Rooms(state::screens::RoomsState::new(workspace_id.clone())));
        if let Screen::Rooms(state) = self.current_mut() {
            state.loading = true;
        }
        self.refresh_rooms().await;
    }

    async fn refresh_rooms(&mut self) {
        match self.client.get_rooms().await {
            Ok(rooms) => {
                if let Screen::Rooms(state) = self.current_mut() {
                    state.set_rooms(rooms);
                    state.loading = false;
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Rooms(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                    state.loading = false;
                }
            }
        }
    }

    async fn bind_room(&mut self) {
        let (room_id, workspace_id) = match self.current() {
            Screen::RoomBinding(state) => (state.room.room_id.clone(), state.workspace_id.clone()),
            _ => return,
        };
        match self.client.bind_room(&room_id, &workspace_id).await {
            Ok(_) => {
                if let Screen::RoomBinding(state) = self.current_mut() {
                    state.mark_bound();
                }
                if let Screen::RoomBinding(state) = self.current() {
                    if state.done {
                        self.pop();
                        if let Screen::Rooms(state) = self.current_mut() {
                            state.mark_room_bound(&room_id);
                        }
                        self.set_status("Room bound");
                    }
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::RoomBinding(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn launch_run(&mut self) {
        let (request, workspace_id) = match self.current() {
            Screen::RunComposer(state) => match state.request() {
                Some(request) => (request, state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        match self.client.launch_run(&workspace_id, &request, &idempotency_key).await {
            Ok(run) => {
                self.enter_run(run, workspace_id);
                self.set_status("Run launched");
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::RunComposer(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    /// Push the Run screen and start the SSE stream task.
    fn enter_run(&mut self, run: RunResponse, workspace_id: String) {
        let run_id = run.run_id.clone();
        let stream_run_id = run_id.clone();
        let after = run.next_sequence;
        let cookie = self.client.cookie().unwrap_or("").to_string();
        let base_url = self.base_url.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            use api_client::sse::{EventStream, StreamEvent};
            let mut stream = EventStream::new(&base_url, &cookie, &stream_run_id, after);
            loop {
                match stream.next().await {
                    Some(Ok(StreamEvent::Run(event))) => {
                        if tx.send(AppEvent::RunEvent(event)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(StreamEvent::Reconnecting { .. })) => {
                        if tx.send(AppEvent::Reconnecting).is_err() {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = tx.send(AppEvent::RunError(error));
                        break;
                    }
                    None => {
                        let _ = tx.send(AppEvent::RunStreamEnded);
                        break;
                    }
                }
            }
        });
        self.stream_rx = Some(rx);
        self.stream_task = Some(task);
        self.push(Screen::Run(RunState::new(run_id, workspace_id)));
    }

    /// Drain pending stream events into the Run screen state.
    pub fn drain_stream_events(&mut self) {
        let events: Vec<AppEvent> = {
            let rx = match &mut self.stream_rx {
                Some(rx) => rx,
                None => return,
            };
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            events
        };
        for event in events {
            match self.current_mut() {
                Screen::Run(state) => match event {
                    AppEvent::RunEvent(event) => {
                        state.set_reconnecting(false);
                        state.accept(event);
                    }
                    AppEvent::Reconnecting => state.set_reconnecting(true),
                    AppEvent::RunStreamEnded => {
                        // The server closed the stream (terminal run or end of
                        // replay). The terminal state is already visible via
                        // the accepted events.
                    }
                    AppEvent::RunError(error) => {
                        if error.is_session_expired() {
                            self.expire_session();
                            return;
                        }
                        state.error = Some(error.to_string());
                    }
                },
                _ => return,
            }
        }
    }

    fn abort_stream(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        self.stream_rx = None;
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

    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn submit_login_stores_cookie_and_opens_workspaces() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/auth/matrix/session");
                then.status(200)
                    .header("set-cookie", "cp_session=abc123; Path=/")
                    .json_body(json!({
                        "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                        "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                    }));
            })
            .await;

        let dir = tempdir().unwrap();
        let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
        let Screen::Login(state) = app.current_mut() else { panic!("login") };
        state.set_homeserver_url("https://matrix.example.org".to_string());
        state.set_access_token("tok_1".to_string());

        app.execute_command(Command::SubmitLogin).await;

        assert_eq!(app.current().id(), ScreenId::Workspaces);
        assert_eq!(app.client.cookie(), Some("cp_session=abc123"));
        let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
        assert_eq!(stored.cookie.as_deref(), Some("cp_session=abc123"));
    }

    #[tokio::test]
    async fn create_workspace_appends_and_persists() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/workspaces");
                then.status(201).json_body(json!({
                    "requestId": "req_1",
                    "workspaceId": "ws_new",
                    "name": "ops",
                    "ownerId": "@u:matrix.example.org",
                    "status": "active",
                    "createdAt": "2026-08-15T00:00:00.000Z"
                }));
            })
            .await;

        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        store.save(&data).unwrap();

        let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
        let Screen::Workspaces(state) = app.current_mut() else { panic!("workspaces") };
        state.creating = true;
        state.set_name_input("ops".to_string());

        app.execute_command(Command::CreateWorkspace).await;

        let Screen::Workspaces(state) = app.current() else { panic!("workspaces") };
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.workspaces[0].workspace_id, "ws_new");
        let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
        assert_eq!(stored.workspaces.len(), 1);
        assert_eq!(stored.workspaces[0].name, "ops");
    }

    #[tokio::test]
    async fn session_expired_anywhere_clears_session_and_returns_to_login() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/api/rooms");
                then.status(401);
            })
            .await;

        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=stale".to_string());
        store.save(&data).unwrap();

        let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
        app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));

        app.execute_command(Command::RefreshRooms).await;

        assert_eq!(app.current().id(), ScreenId::Login);
        assert_eq!(app.client.cookie(), None);
        let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
        assert_eq!(stored.cookie, None, "stale session cleared from disk");
        assert!(app.status.as_deref().unwrap_or("").contains("Session expired"));
    }

    #[tokio::test]
    async fn launch_run_opens_run_screen_and_streams_terminal_event() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/workspaces/ws_1/runs");
                then.status(202).json_body(json!({
                    "runId": "r1",
                    "status": "queued",
                    "roomId": "!a:example.org",
                    "nextSequence": 1
                }));
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/runs/r1/events")
                    .query_param("after", "1");
                then.status(200)
                    .header("content-type", "text/event-stream")
                    .body(
                        // `after=1` means the server replays events AFTER sequence 1,
                        // so the mocked stream starts at sequence 2.
                        "id: 2\nevent: run.queued\ndata: {\"id\":\"ev_2\",\"runId\":\"r1\",\"sequence\":2,\"type\":\"run.queued\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n"
                            .to_string()
                            + "id: 3\nevent: run.completed\ndata: {\"id\":\"ev_3\",\"runId\":\"r1\",\"sequence\":3,\"type\":\"run.completed\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n",
                    );
            })
            .await;

        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        store.save(&data).unwrap();

        let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
        let mut composer = state::screens::RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
        composer.set_prompt("Go".to_string());
        composer.toggle_mode(api_client::RunMode::Parallel);
        composer.toggle_specialist("repo-reader");
        app.push(Screen::RunComposer(composer));

        app.execute_command(Command::LaunchRun).await;
        assert_eq!(app.current().id(), ScreenId::Run);

        // Give the SSE task a moment to deliver the terminal event.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        app.drain_stream_events();

        let Screen::Run(state) = app.current() else { panic!("run") };
        assert_eq!(state.run_id, "r1");
        assert!(state.is_terminal());
        assert_eq!(state.events().len(), 2);
        assert_eq!(state.highest_sequence(), 3);
    }
}
