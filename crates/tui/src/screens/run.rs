use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::RunState;

pub fn handle_run_key(state: &mut RunState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('c') => Command::CancelRun,
        KeyCode::Char('r') => Command::RefreshDeliveries,
        KeyCode::Char('g') => Command::NavigateToGitHubWorkspace,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.10.
pub fn render_run(_state: &RunState, _frame: &mut Frame, _area: Rect) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn run_keys_map_to_commands() {
        let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('c'))), Command::CancelRun);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshDeliveries);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('g'))), Command::NavigateToGitHubWorkspace);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }
}
