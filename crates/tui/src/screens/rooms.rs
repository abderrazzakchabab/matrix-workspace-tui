use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::{RoomBindingState, RoomsState};

/// Stub — replaced in Group 8, Task 8.5.
pub fn handle_rooms_key(_state: &mut RoomsState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_rooms(_state: &RoomsState, _frame: &mut Frame, _area: Rect) {}

/// Stub — replaced in Group 8, Task 8.5.
pub fn handle_room_binding_key(_state: &mut RoomBindingState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_room_binding(_state: &RoomBindingState, _frame: &mut Frame, _area: Rect) {}
