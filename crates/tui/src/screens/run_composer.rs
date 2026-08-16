use crate::app::Command;
use api_client::RunMode;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::{RunComposerState, SPECIALISTS};

pub fn handle_run_composer_key(state: &mut RunComposerState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('p') => {
            state.toggle_mode(RunMode::Parallel);
            Command::None
        }
        KeyCode::Char('s') => {
            state.toggle_mode(RunMode::Sequential);
            Command::None
        }
        KeyCode::Char(' ') => {
            state.toggle_specialist_at_cursor();
            Command::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.move_specialist_cursor_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.move_specialist_cursor_prev();
            Command::None
        }
        KeyCode::Char(c) => {
            let mut value = state.prompt.clone();
            value.push(c);
            state.set_prompt(value);
            Command::None
        }
        KeyCode::Backspace => {
            let mut value = state.prompt.clone();
            value.pop();
            state.set_prompt(value);
            Command::None
        }
        KeyCode::Enter => match state.validation_error() {
            None => Command::LaunchRun,
            Some(error) => {
                state.error = Some(error);
                Command::None
            }
        },
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.8.
pub fn render_run_composer(_state: &RunComposerState, _frame: &mut Frame, _area: Rect) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn composer() -> RunComposerState {
        RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string())
    }

    #[test]
    fn composer_types_prompt_and_toggles_mode_and_specialists() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('h'))), Command::None);
        assert_eq!(state.prompt, "h");
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('p'))), Command::None);
        assert_eq!(state.mode, Some(RunMode::Parallel));
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('s'))), Command::None);
        assert_eq!(state.mode, Some(RunMode::Sequential));
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char(' '))), Command::None);
        assert_eq!(state.selected_specialists, vec![SPECIALISTS[0].0]);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.prompt, "");
    }

    #[test]
    fn composer_enter_launches_when_valid() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::None, "invalid form");
        state.set_prompt("Go".to_string());
        state.toggle_mode(RunMode::Parallel);
        state.toggle_specialist("repo-reader");
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::LaunchRun);
    }

    #[test]
    fn composer_cursor_moves_with_jk() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('j'))), Command::None);
        assert_eq!(state.specialist_cursor, 1);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('k'))), Command::None);
        assert_eq!(state.specialist_cursor, 0);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }
}
