use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::GitHubWorkspaceState;

/// Stub — replaced in Group 8, Task 8.11.
pub fn handle_github_workspace_key(_state: &mut GitHubWorkspaceState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.12.
pub fn render_github_workspace(_state: &GitHubWorkspaceState, _frame: &mut Frame, _area: Rect) {}
