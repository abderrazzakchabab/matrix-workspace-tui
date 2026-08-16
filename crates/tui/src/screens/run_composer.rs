use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::RunComposerState;

/// Stub — replaced in Group 8, Task 8.7.
pub fn handle_run_composer_key(_state: &mut RunComposerState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.8.
pub fn render_run_composer(_state: &RunComposerState, _frame: &mut Frame, _area: Rect) {}
