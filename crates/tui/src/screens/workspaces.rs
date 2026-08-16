use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::WorkspacesState;

pub fn handle_workspaces_key(state: &mut WorkspacesState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        KeyCode::Char('n') => {
            state.creating = !state.creating;
            Command::None
        }
        KeyCode::Char(c) if state.creating => {
            let mut value = state.name_input.clone();
            value.push(c);
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Backspace if state.creating => {
            let mut value = state.name_input.clone();
            value.pop();
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Enter if state.creating => Command::CreateWorkspace,
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Enter => {
            if state.selected().is_some() {
                Command::NavigateToRooms
            } else {
                Command::None
            }
        }
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.4.
pub fn render_workspaces(_state: &WorkspacesState, _frame: &mut Frame, _area: Rect) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_workspaces() -> WorkspacesState {
        let mut state = WorkspacesState::new();
        state.add_workspace(api_client::WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "Alpha".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        state
    }

    #[test]
    fn workspaces_navigation_and_create_mode() {
        let mut state = with_workspaces();
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('j'))), Command::None);
        assert_eq!(state.selected, 0);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::NavigateToRooms);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('n'))), Command::None);
        assert!(state.creating);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('o'))), Command::None);
        assert_eq!(state.name_input, "o");
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.name_input, "");
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('p'))), Command::None);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::CreateWorkspace);
    }

    #[test]
    fn workspaces_q_quits() {
        let mut state = with_workspaces();
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
    }
}
