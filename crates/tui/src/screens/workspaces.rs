use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use state::screens::WorkspacesState;

pub fn handle_workspaces_key(state: &mut WorkspacesState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char(c) if state.creating => {
            let mut value = state.name_input.clone();
            value.push(c);
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Backspace if state.creating => {
            let mut value = state.name_input.clone();
            value.pop();
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Esc if state.creating => {
            state.creating = false;
            state.name_input.clear();
            Command::None
        }
        KeyCode::Enter if state.creating => Command::CreateWorkspace,
        KeyCode::Char('q') => Command::Quit,
        KeyCode::Char('n') => {
            state.creating = !state.creating;
            Command::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Enter => {
            if state.selected().is_some() {
                Command::NavigateToRooms
            } else {
                Command::None
            }
        }
        _ => Command::None,
    }
}

pub fn render_workspaces(state: &WorkspacesState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            // Min(3), not Min(4): ratatui 0.29.0's layout solver is
            // nondeterministic when constraints oversubscribe the area
            // (13 > 12 rows here), which made this render test flaky.
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Workspaces").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = state
        .workspaces
        .iter()
        .enumerate()
        .map(|(index, workspace)| {
            let marker = if index == state.selected { ">" } else { " " };
            ListItem::new(format!(
                "{marker} {:<24} {}",
                workspace.name, workspace.status
            ))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Known workspaces"),
    );
    frame.render_widget(list, chunks[1]);

    let create = if state.creating {
        Paragraph::new(state.name_input.as_str()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("New workspace name (Enter to create)")
                .border_style(Style::default().fg(Color::Cyan)),
        )
    } else {
        Paragraph::new("Press n to create a workspace")
    };
    frame.render_widget(create, chunks[2]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);

    let hints = Paragraph::new("j/k: move   Enter: open   n: new workspace   q: quit");
    frame.render_widget(hints, chunks[4]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_workspaces() -> WorkspacesState {
        let mut state = WorkspacesState::new();
        state.add_workspace(api_client::WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "Alpha".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        state
    }

    #[test]
    fn workspaces_navigation_and_create_mode() {
        let mut state = with_workspaces();
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Char('j'))),
            Command::None
        );
        assert_eq!(state.selected, 0);
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Enter)),
            Command::NavigateToRooms
        );
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Char('n'))),
            Command::None
        );
        assert!(state.creating);
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Char('o'))),
            Command::None
        );
        assert_eq!(state.name_input, "o");
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Backspace)),
            Command::None
        );
        assert_eq!(state.name_input, "");
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Char('p'))),
            Command::None
        );
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Enter)),
            Command::CreateWorkspace
        );
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Esc)),
            Command::None
        );
        assert!(!state.creating);
        assert_eq!(state.name_input, "", "Esc clears the draft name");
    }

    #[test]
    fn workspaces_q_quits() {
        let mut state = with_workspaces();
        assert_eq!(
            handle_workspaces_key(&mut state, key(KeyCode::Char('q'))),
            Command::Quit
        );
    }

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn workspaces_render_lists_workspaces_and_create_input() {
        let mut state = with_workspaces();
        state.creating = true;
        state.set_name_input("ops".to_string());

        let backend = TestBackend::new(70, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_workspaces(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("Alpha"), "{rendered}");
        assert!(rendered.contains("ops"), "{rendered}");
        assert!(rendered.contains("Workspaces"), "{rendered}");
    }
}
