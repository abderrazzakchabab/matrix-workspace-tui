use crate::app::Command;
use api_client::MatrixDeliveryStatus;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use state::screens::RunState;

pub fn handle_run_key(_state: &mut RunState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('c') => Command::CancelRun,
        KeyCode::Char('r') => Command::RefreshDeliveries,
        KeyCode::Char('g') => Command::NavigateToGitHubWorkspace,
        _ => Command::None,
    }
}

fn delivery_label(status: MatrixDeliveryStatus) -> &'static str {
    match status {
        MatrixDeliveryStatus::Pending => "pending",
        MatrixDeliveryStatus::Delivered => "delivered",
        MatrixDeliveryStatus::Failed => "failed",
        MatrixDeliveryStatus::Dead => "dead",
    }
}

pub fn render_run(state: &RunState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            // Min(5), not Min(6): ratatui 0.29.0's layout solver is
            // nondeterministic when constraints oversubscribe the area
            // (19 > 18 rows here), which made this render test flaky.
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let terminal_line = if state.is_terminal() {
        "Run finished (terminal state)".to_string()
    } else if state.cancel_requested {
        "Cancellation requested".to_string()
    } else {
        "Running".to_string()
    };
    let title = Paragraph::new(format!("Run {} — {terminal_line}", state.run_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let reconnect = if state.reconnecting {
        Paragraph::new("reconnecting…").style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new("")
    };
    frame.render_widget(reconnect, chunks[1]);

    let items: Vec<ListItem> = state
        .events()
        .iter()
        .map(|event| {
            ListItem::new(format!(
                "#{} {} {}",
                event.sequence,
                event.event_type.as_str(),
                event.payload
            ))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Timeline"));
    frame.render_widget(list, chunks[2]);

    let deliveries: Vec<ListItem> = state
        .deliveries
        .iter()
        .map(|delivery| {
            ListItem::new(format!(
                "sequence {}: {}",
                delivery.sequence,
                delivery_label(delivery.status)
            ))
        })
        .collect();
    let deliveries = List::new(deliveries).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Matrix delivery (authoritative)"),
    );
    frame.render_widget(deliveries, chunks[3]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[4]);

    let hints = Paragraph::new("c: cancel   r: refresh deliveries   g: GitHub workspace   q: back");
    frame.render_widget(hints, chunks[5]);
}

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
        assert_eq!(
            handle_run_key(&mut state, key(KeyCode::Char('c'))),
            Command::CancelRun
        );
        assert_eq!(
            handle_run_key(&mut state, key(KeyCode::Char('r'))),
            Command::RefreshDeliveries
        );
        assert_eq!(
            handle_run_key(&mut state, key(KeyCode::Char('g'))),
            Command::NavigateToGitHubWorkspace
        );
        assert_eq!(
            handle_run_key(&mut state, key(KeyCode::Char('q'))),
            Command::Back
        );
    }

    use api_client::{
        EventVisibility, MatrixDelivery, MatrixDeliveryStatus, RunEvent, RunEventType,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn run_event(sequence: u64, event_type: RunEventType) -> RunEvent {
        RunEvent {
            id: format!("ev_{sequence}"),
            run_id: "r1".to_string(),
            sequence,
            event_type,
            version: 1,
            occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
            visibility: EventVisibility::RoomAndOwner,
            payload: serde_json::json!({ "note": format!("step {sequence}") }),
        }
    }

    #[test]
    fn run_render_shows_timeline_deliveries_and_reconnect_banner() {
        let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
        state.accept(run_event(1, RunEventType::RunStarted));
        state.accept(run_event(2, RunEventType::SpecialistProgress));
        state.set_deliveries(vec![MatrixDelivery {
            sequence: 1,
            status: MatrixDeliveryStatus::Delivered,
        }]);
        state.set_reconnecting(true);

        let backend = TestBackend::new(90, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_run(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("Run r1"), "{rendered}");
        assert!(rendered.contains("specialist.progress"), "{rendered}");
        assert!(rendered.contains("delivered"), "{rendered}");
        assert!(rendered.contains("reconnecting"), "{rendered}");
    }
}
