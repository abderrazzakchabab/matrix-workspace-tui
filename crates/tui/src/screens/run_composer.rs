use crate::app::Command;
use api_client::RunMode;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
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

pub fn render_run_composer(state: &RunComposerState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new(format!("Compose run — room {}", state.room_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let prompt = Paragraph::new(state.prompt.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Prompt (type to edit)")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(prompt, chunks[1]);

    let mode = match state.mode {
        Some(RunMode::Parallel) => "parallel (p)",
        Some(RunMode::Sequential) => "sequential (s)",
        None => "unset — press p or s",
    };
    frame.render_widget(Paragraph::new(format!("Mode: {mode}")), chunks[2]);

    let items: Vec<ListItem> = SPECIALISTS
        .iter()
        .enumerate()
        .map(|(index, (id, name))| {
            let marker = if index == state.specialist_cursor {
                ">"
            } else {
                " "
            };
            let selected = if state.selected_specialists.iter().any(|value| value == id) {
                "[x]"
            } else {
                "[ ]"
            };
            ListItem::new(format!("{marker} {selected} {name}"))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Specialists (space to toggle)"),
    );
    frame.render_widget(list, chunks[3]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[4]);

    let hints = Paragraph::new(
        "type: prompt   p/s: mode   j/k+space: specialists   Enter: launch   q: back",
    );
    frame.render_widget(hints, chunks[5]);
}

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
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('h'))),
            Command::None
        );
        assert_eq!(state.prompt, "h");
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('p'))),
            Command::None
        );
        assert_eq!(state.mode, Some(RunMode::Parallel));
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('s'))),
            Command::None
        );
        assert_eq!(state.mode, Some(RunMode::Sequential));
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char(' '))),
            Command::None
        );
        assert_eq!(state.selected_specialists, vec![SPECIALISTS[0].0]);
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Backspace)),
            Command::None
        );
        assert_eq!(state.prompt, "");
    }

    #[test]
    fn composer_enter_launches_when_valid() {
        let mut state = composer();
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Enter)),
            Command::None,
            "invalid form"
        );
        state.set_prompt("Go".to_string());
        state.toggle_mode(RunMode::Parallel);
        state.toggle_specialist("repo-reader");
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Enter)),
            Command::LaunchRun
        );
    }

    #[test]
    fn composer_cursor_moves_with_jk() {
        let mut state = composer();
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('j'))),
            Command::None
        );
        assert_eq!(state.specialist_cursor, 1);
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('k'))),
            Command::None
        );
        assert_eq!(state.specialist_cursor, 0);
        assert_eq!(
            handle_run_composer_key(&mut state, key(KeyCode::Char('q'))),
            Command::Back
        );
    }

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn composer_render_shows_prompt_mode_and_specialists() {
        let mut state = composer();
        state.set_prompt("Summarize the PRs".to_string());
        state.toggle_mode(RunMode::Parallel);
        state.toggle_specialist("pr-reader");

        let backend = TestBackend::new(80, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_run_composer(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("Summarize the PRs"), "{rendered}");
        assert!(rendered.contains("parallel"), "{rendered}");
        assert!(rendered.contains("Pull Request reader"), "{rendered}");
    }
}
