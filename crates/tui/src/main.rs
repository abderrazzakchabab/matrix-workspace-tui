mod app;
mod screens;

use crate::app::{App, AppError};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use state::session_store::SessionStore;
use std::io;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let base_url = std::env::var("MATRIX_WORKSPACE_TUI_CONTROL_PLANE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let path = SessionStore::default_path().map_err(AppError::State)?;
    let store = SessionStore::at_path(path);

    let mut terminal = setup_terminal()?;
    let outcome = App::new(base_url, store).run(&mut terminal).await;
    restore_terminal(&mut terminal)?;
    outcome
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, AppError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), AppError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
