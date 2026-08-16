use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use state::screens::LoginField;
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

fn masked_token(token: &str) -> String {
    if token.is_empty() {
        "(empty)".to_string()
    } else {
        "•".repeat(token.len().min(12))
    }
}

pub fn render_login(state: &LoginState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

    let title = Paragraph::new("Matrix Agent Workspace — Sign in")
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let url_focus = state.focus == LoginField::HomeserverUrl;
    let url_border = if url_focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let url = Paragraph::new(state.homeserver_url.as_str())
        .block(Block::default().borders(Borders::ALL).title(if url_focus { "Homeserver URL *" } else { "Homeserver URL" }).border_style(url_border));
    frame.render_widget(url, chunks[1]);

    let token_focus = state.focus == LoginField::AccessToken;
    let token_border = if token_focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let token = Paragraph::new(masked_token(&state.access_token))
        .block(Block::default().borders(Borders::ALL).title(if token_focus { "Access token *" } else { "Access token" }).border_style(token_border));
    frame.render_widget(token, chunks[2]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);

    let hints = Paragraph::new("Tab: switch field   Enter: sign in   q: quit");
    frame.render_widget(hints, chunks[4]);
}

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

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn login_render_shows_fields_and_error() {
        let mut state = LoginState::default();
        state.set_homeserver_url("https://matrix.example.org".to_string());
        state.set_access_token("secret".to_string());
        state.error = Some("Invalid token".to_string());

        let backend = TestBackend::new(70, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_login(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal.backend().buffer().content().iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("Matrix Agent Workspace"), "{rendered}");
        assert!(rendered.contains("https://matrix.example.org"), "{rendered}");
        assert!(rendered.contains("Invalid token"), "{rendered}");
        // The token must never be rendered verbatim.
        assert!(!rendered.contains("secret"), "token is masked: {rendered}");
    }
}
