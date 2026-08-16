use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
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

pub fn handle_room_binding_key(state: &mut RoomBindingState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Command::Back,
        KeyCode::Char('y') | KeyCode::Enter => Command::BindRoom,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_rooms(_state: &RoomsState, _frame: &mut Frame, _area: Rect) {}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_room_binding(_state: &RoomBindingState, _frame: &mut Frame, _area: Rect) {}

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
        assert_eq!(handle_rooms_key(&mut bound, key(KeyCode::Enter)), Command::NavigateToComposer);

        let mut unbound = RoomsState::new("ws_1".to_string());
        unbound.set_rooms(vec![room("!a:example.org", None)]);
        assert_eq!(handle_rooms_key(&mut unbound, key(KeyCode::Enter)), Command::NavigateToRoomBinding);
    }

    #[test]
    fn rooms_refresh_and_back() {
        let mut state = RoomsState::new("ws_1".to_string());
        assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshRooms);
        assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }

    #[test]
    fn room_binding_enter_confirms_bind_and_q_goes_back() {
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('y'))), Command::BindRoom);
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Enter)), Command::BindRoom);
    }
}
