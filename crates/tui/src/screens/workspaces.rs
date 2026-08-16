use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::WorkspacesState;

/// Stub — replaced in Group 8, Task 8.3.
pub fn handle_workspaces_key(_state: &mut WorkspacesState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.4.
pub fn render_workspaces(_state: &WorkspacesState, _frame: &mut Frame, _area: Rect) {}
