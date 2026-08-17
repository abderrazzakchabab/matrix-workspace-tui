use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use state::screens::{RoomBindingState, RoomsState};

pub fn handle_rooms_key(state: &mut RoomsState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('r') => Command::RefreshRooms,
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Enter => {
            if state.selected_room().is_none() {
                Command::None
            } else if state.room_is_bound_to_workspace() {
                Command::NavigateToComposer
            } else {
                Command::NavigateToRoomBinding
            }
        }
        _ => Command::None,
    }
}

pub fn handle_room_binding_key(_state: &mut RoomBindingState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Command::Back,
        KeyCode::Char('y') | KeyCode::Enter => Command::BindRoom,
        _ => Command::None,
    }
}

pub fn render_rooms(state: &RoomsState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new(format!("Rooms — workspace {}", state.workspace_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = state
        .rooms
        .iter()
        .enumerate()
        .map(|(index, room)| {
            let marker = if index == state.selected { ">" } else { " " };
            let binding = match room.workspace_id.as_deref() {
                Some(workspace_id) if workspace_id == state.workspace_id => {
                    "bound to this workspace"
                }
                Some(_) => "bound elsewhere",
                None => "unbound",
            };
            ListItem::new(format!(
                "{marker} {:<40} {binding}",
                room.display_name.as_deref().unwrap_or(&room.room_id)
            ))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Rooms"));
    frame.render_widget(list, chunks[1]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[2]);

    let hints = Paragraph::new("j/k: move   Enter: compose (bound) or bind   r: refresh   q: back");
    frame.render_widget(hints, chunks[3]);
}

pub fn render_room_binding(state: &RoomBindingState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Bind room to workspace")
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);
    frame.render_widget(
        Paragraph::new(format!("Room:      {}", state.room.room_id)),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(format!("Workspace: {}", state.workspace_id)),
        chunks[2],
    );

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn room(room_id: &str, workspace_id: Option<&str>) -> api_client::RoomSummary {
        api_client::RoomSummary {
            room_id: room_id.to_string(),
            homeserver_url: "https://example.org".to_string(),
            display_name: Some(room_id.to_string()),
            workspace_id: workspace_id.map(|value| value.to_string()),
        }
    }

    #[test]
    fn rooms_enter_opens_composer_when_bound_else_binding() {
        let mut bound = RoomsState::new("ws_1".to_string());
        bound.set_rooms(vec![room("!a:example.org", Some("ws_1"))]);
        assert_eq!(
            handle_rooms_key(&mut bound, key(KeyCode::Enter)),
            Command::NavigateToComposer
        );

        let mut unbound = RoomsState::new("ws_1".to_string());
        unbound.set_rooms(vec![room("!a:example.org", None)]);
        assert_eq!(
            handle_rooms_key(&mut unbound, key(KeyCode::Enter)),
            Command::NavigateToRoomBinding
        );
    }

    #[test]
    fn rooms_refresh_and_back() {
        let mut state = RoomsState::new("ws_1".to_string());
        assert_eq!(
            handle_rooms_key(&mut state, key(KeyCode::Char('r'))),
            Command::RefreshRooms
        );
        assert_eq!(
            handle_rooms_key(&mut state, key(KeyCode::Char('q'))),
            Command::Back
        );
    }

    #[test]
    fn room_binding_enter_confirms_bind_and_q_goes_back() {
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(
            handle_room_binding_key(&mut state, key(KeyCode::Char('y'))),
            Command::BindRoom
        );
        assert_eq!(
            handle_room_binding_key(&mut state, key(KeyCode::Char('q'))),
            Command::Back
        );
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(
            handle_room_binding_key(&mut state, key(KeyCode::Enter)),
            Command::BindRoom
        );
    }

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn rooms_render_shows_rooms_and_binding_state() {
        let mut state = RoomsState::new("ws_1".to_string());
        state.set_rooms(vec![
            room("!a:example.org", Some("ws_1")),
            room("!b:example.org", None),
        ]);
        let backend = TestBackend::new(70, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_rooms(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("!a:example.org"), "{rendered}");
        assert!(rendered.contains("!b:example.org"), "{rendered}");
        assert!(rendered.contains("bound"), "{rendered}");
    }

    #[test]
    fn room_binding_render_shows_room_and_workspace() {
        let state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        let backend = TestBackend::new(70, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_room_binding(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("!a:example.org"), "{rendered}");
        assert!(rendered.contains("ws_1"), "{rendered}");
    }
}
