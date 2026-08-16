use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::{GithubPanel, GitHubWorkspaceState};

pub fn handle_github_workspace_key(state: &mut GitHubWorkspaceState, key: KeyEvent) -> Command {
    if state.confirmation.is_some() {
        return match key.code {
            KeyCode::Char('y') => Command::ConfirmMutation,
            KeyCode::Char('n') | KeyCode::Char('q') => {
                state.confirmation = None;
                Command::None
            }
            _ => Command::None,
        };
    }
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('1') => {
            state.switch_panel(GithubPanel::Repositories);
            Command::None
        }
        KeyCode::Char('2') => {
            state.switch_panel(GithubPanel::Issues);
            Command::None
        }
        KeyCode::Char('3') => {
            state.switch_panel(GithubPanel::PullRequests);
            Command::None
        }
        KeyCode::Char('4') => {
            state.switch_panel(GithubPanel::Audit);
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
        KeyCode::Char('r') => Command::RefreshPanel,
        KeyCode::Char('g') => Command::RequestGrant,
        KeyCode::Char('m') => {
            state.mutation_mode = !state.mutation_mode;
            Command::None
        }
        KeyCode::Char(c) if state.mutation_mode => {
            let mut value = state.mutation_title.clone();
            value.push(c);
            state.set_mutation_title(value);
            Command::None
        }
        KeyCode::Backspace if state.mutation_mode => {
            let mut value = state.mutation_title.clone();
            value.pop();
            state.set_mutation_title(value);
            Command::None
        }
        KeyCode::Enter if state.mutation_mode => {
            let title = state.mutation_title.clone();
            match state.begin_mutation(title) {
                Some(draft) => {
                    state.confirmation = Some(draft);
                    state.mutation_mode = false;
                    Command::None
                }
                None => {
                    state.error = Some("Provide an issue title before confirming".to_string());
                    Command::None
                }
            }
        }
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.12.
pub fn render_github_workspace(_state: &GitHubWorkspaceState, _frame: &mut Frame, _area: Rect) {}

#[cfg(test)]
mod tests {
    use super::*;
    use api_client::GithubRepositorySummary;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn repo() -> GithubRepositorySummary {
        GithubRepositorySummary {
            id: 1,
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            owner: "octo".to_string(),
            private: false,
            default_branch: "main".to_string(),
            description: None,
            html_url: "https://github.com/octo/repo".to_string(),
            archived: false,
        }
    }

    fn github() -> GitHubWorkspaceState {
        let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
        state.set_repositories(vec![repo()]);
        state
    }

    #[test]
    fn github_panel_switching_and_navigation() {
        let mut state = github();
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('2'))), Command::None);
        assert_eq!(state.panel, GithubPanel::Issues);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('4'))), Command::None);
        assert_eq!(state.panel, GithubPanel::Audit);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshPanel);
    }

    #[test]
    fn github_grant_and_mutation_flow() {
        let mut state = github();
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('g'))), Command::RequestGrant);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('m'))), Command::None);
        assert!(state.mutation_mode);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('t'))), Command::None);
        assert_eq!(state.mutation_title, "t");
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Enter)), Command::None);
        assert!(state.confirmation.is_some(), "confirmation draft shown");
    }

    #[test]
    fn github_confirmation_keys_confirm_or_dismiss() {
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('y'))), Command::ConfirmMutation);
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('n'))), Command::None);
        assert!(state.confirmation.is_none(), "dismissed");
    }
}
