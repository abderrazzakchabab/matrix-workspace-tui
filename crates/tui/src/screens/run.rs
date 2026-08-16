use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::RunState;

/// Stub — replaced in Group 8, Task 8.9.
pub fn handle_run_key(_state: &mut RunState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.10.
pub fn render_run(_state: &RunState, _frame: &mut Frame, _area: Rect) {}
