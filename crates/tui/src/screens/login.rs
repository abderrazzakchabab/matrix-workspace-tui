use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::LoginState;

pub fn handle_login_key(state: &mut LoginState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        KeyCode::Char('\t') => {
            state.toggle_focus();
            Command::None
        }
        KeyCode::Char(c) => {
            state.insert_char(c);
            Command::None
        }
        KeyCode::Backspace => {
            state.backspace();
            Command::None
        }
        KeyCode::Enter => match state.validation_error() {
            None => Command::SubmitLogin,
            Some(error) => {
                state.error = Some(error);
                Command::None
            }
        },
        _ => Command::None,
    }
}

/// Stub — replaced by the full render in Group 8, Task 8.2.
pub fn render_login(_state: &LoginState, _frame: &mut Frame, _area: Rect) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use state::screens::LoginField;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_goes_to_the_focused_field() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('h'))), Command::None);
        assert_eq!(state.homeserver_url, "h");
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('\t'))), Command::None);
        assert_eq!(state.focus, LoginField::AccessToken);
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('t'))), Command::None);
        assert_eq!(state.access_token, "t");
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.access_token, "");
    }

    #[test]
    fn enter_submits_only_when_valid() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::None);
        assert!(state.error.is_some(), "invalid form shows an error");
        state.set_homeserver_url("https://matrix.example.org".to_string());
        state.set_access_token("tok".to_string());
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::SubmitLogin);
    }

    #[test]
    fn q_quits() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
    }
}
