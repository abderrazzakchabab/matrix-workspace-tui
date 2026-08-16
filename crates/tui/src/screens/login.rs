use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::LoginState;

/// Stub — replaced by the full handler in Group 8, Task 8.1.
pub fn handle_login_key(_state: &mut LoginState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        _ => Command::None,
    }
}

/// Stub — replaced by the full render in Group 8, Task 8.2.
pub fn render_login(_state: &LoginState, _frame: &mut Frame, _area: Rect) {}
