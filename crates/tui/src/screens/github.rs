use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use state::screens::{GitHubWorkspaceState, GithubPanel, MutationFlowStatus};

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
        KeyCode::Esc if state.mutation_mode => {
            state.mutation_mode = false;
            state.mutation_title.clear();
            Command::None
        }
        KeyCode::Enter if state.mutation_mode => {
            if state.selected_repository().is_none() {
                state.error = Some("Select a repository before composing a mutation".to_string());
                return Command::None;
            }
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
            let has_grant = matches!(
                state.grant,
                Some(ref g) if g.status != api_client::GrantStatus::Revoked
            );
            if has_grant {
                state.mutation_mode = !state.mutation_mode;
            } else {
                state.error =
                    Some("Request a write grant (g) before composing a mutation".to_string());
            }
            Command::None
        }
        _ => Command::None,
    }
}

fn panel_title(panel: GithubPanel) -> &'static str {
    match panel {
        GithubPanel::Repositories => "Repositories (1)",
        GithubPanel::Issues => "Issues (2)",
        GithubPanel::PullRequests => "Pull requests (3)",
        GithubPanel::Audit => "Audit (4)",
    }
}

fn panel_items(state: &GitHubWorkspaceState) -> Vec<String> {
    match state.panel {
        GithubPanel::Repositories => state
            .repositories
            .iter()
            .map(|repo| {
                format!(
                    "{} ({}branch {})",
                    repo.full_name,
                    if repo.private { "private " } else { "" },
                    repo.default_branch
                )
            })
            .collect(),
        GithubPanel::Issues => state
            .issues
            .iter()
            .map(|issue| format!("#{} {} [{}]", issue.number, issue.title, issue.state))
            .collect(),
        GithubPanel::PullRequests => state
            .pulls
            .iter()
            .map(|pull| {
                format!(
                    "#{} {} [{}]{}",
                    pull.number,
                    pull.title,
                    pull.state,
                    if pull.draft { " draft" } else { "" }
                )
            })
            .collect(),
        GithubPanel::Audit => state
            .audit
            .iter()
            .map(|record| {
                format!(
                    "{} {} {}",
                    record.created_at,
                    record.operation.as_deref().unwrap_or("-"),
                    record.outcome
                )
            })
            .collect(),
    }
}

fn mutation_status_line(status: MutationFlowStatus, command_id: Option<&String>) -> String {
    match status {
        MutationFlowStatus::Idle => String::new(),
        MutationFlowStatus::Submitting => "Submitting the approved command…".to_string(),
        MutationFlowStatus::Submitted => format!("Mutation queued. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
        MutationFlowStatus::Succeeded => format!("Mutation completed. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
        MutationFlowStatus::Denied => "Mutation denied. The write grant is missing or the approval does not match this exact command.".to_string(),
        MutationFlowStatus::Expired => "The approval expired. Confirm again to record a fresh approval.".to_string(),
        MutationFlowStatus::Failed => "The mutation failed. Review the audit history before retrying.".to_string(),
        MutationFlowStatus::Duplicate => format!("This exact command was already submitted; showing the recorded result. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
    }
}

pub fn render_github_workspace(state: &GitHubWorkspaceState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            // Min(5), not Min(6), and the confirmation row is Length(6), not
            // Length(2): ratatui 0.29.0's layout solver is nondeterministic
            // when constraints oversubscribe the area, and a 2-row area can
            // never show the multi-line confirmation paragraph.
            Constraint::Min(5),
            Constraint::Length(2),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(area);

    let title =
        Paragraph::new("GitHub workspace").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);
    frame.render_widget(Paragraph::new(panel_title(state.panel)), chunks[1]);

    let items: Vec<ListItem> = panel_items(state)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let marker = if index == state.selected_index {
                ">"
            } else {
                " "
            };
            ListItem::new(format!("{marker} {line}"))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL));
    frame.render_widget(list, chunks[2]);

    let status_line = mutation_status_line(state.mutation_status, state.command_id.as_ref());
    let status = if status_line.is_empty() {
        match &state.error {
            Some(message) => {
                Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red))
            }
            None => Paragraph::new(""),
        }
    } else {
        Paragraph::new(status_line)
    };
    frame.render_widget(status, chunks[3]);

    if let Some(draft) = &state.confirmation {
        let operation_name = match draft.operation {
            api_client::GithubMutationOperation::CreateIssue => "create_issue",
            api_client::GithubMutationOperation::UpdateIssue => "update_issue",
            api_client::GithubMutationOperation::CommentIssue => "comment_issue",
            api_client::GithubMutationOperation::CreatePrComment => "create_pr_comment",
        };
        let confirmation = Paragraph::new(format!(
            "Confirm mutation — operation: {}   scope: {}   repository: {}\narguments: {}\nPress y to confirm, n to dismiss",
            operation_name,
            match draft.scope {
                api_client::GithubWriteScope::IssuesWrite => "issues:write",
                api_client::GithubWriteScope::PullRequestsWrite => "pull_requests:write",
            },
            draft.repository,
            draft.arguments,
        ))
        .block(Block::default().borders(Borders::ALL).title("Confirm mutation").border_style(Style::default().fg(Color::Yellow)));
        frame.render_widget(confirmation, chunks[4]);
    } else {
        let hints = if state.mutation_mode {
            Paragraph::new(format!(
                "Issue title: {}   (Enter: review, Esc: cancel)",
                state.mutation_title
            ))
        } else if matches!(
            state.grant,
            Some(ref g) if g.status != api_client::GrantStatus::Revoked
        ) {
            Paragraph::new(
                "1-4: panels   r: refresh   g: request write grant   m: compose mutation   q: back",
            )
        } else {
            Paragraph::new("1-4: panels   r: refresh   g: request write grant   q: back")
        };
        frame.render_widget(hints, chunks[4]);
    }

    let grant_line = match &state.grant {
        Some(grant) => format!(
            "Grant {}{}",
            grant.grant_id,
            if grant.status == api_client::GrantStatus::Pending {
                " (pending approval)"
            } else {
                ""
            }
        ),
        None => "No write grant requested yet".to_string(),
    };
    frame.render_widget(Paragraph::new(grant_line), chunks[5]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_client::{
        GithubRepositorySummary, GithubWriteGrantResult, GithubWriteScope, GrantStatus,
    };
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
        let mut state = GitHubWorkspaceState::new(
            "ws_1".to_string(),
            "r1".to_string(),
            Some("inst_9".to_string()),
        );
        state.set_repositories(vec![repo()]);
        state
    }

    #[test]
    fn github_panel_switching_and_navigation() {
        let mut state = github();
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('2'))),
            Command::None
        );
        assert_eq!(state.panel, GithubPanel::Issues);
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('4'))),
            Command::None
        );
        assert_eq!(state.panel, GithubPanel::Audit);
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('q'))),
            Command::Back
        );
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('r'))),
            Command::RefreshPanel
        );
    }

    #[test]
    fn github_grant_and_mutation_flow() {
        let mut state = github();
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('g'))),
            Command::RequestGrant
        );
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('m'))),
            Command::None
        );
        assert!(!state.mutation_mode, "m is gated on a write grant");
        assert_eq!(
            state.error.as_deref(),
            Some("Request a write grant (g) before composing a mutation")
        );
        state.set_grant(GithubWriteGrantResult {
            grant_id: "grant_1".to_string(),
            status: GrantStatus::Pending,
            repository: "octo/repo".to_string(),
            scope: GithubWriteScope::IssuesWrite,
        });
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('m'))),
            Command::None
        );
        assert!(state.mutation_mode);
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('t'))),
            Command::None
        );
        assert_eq!(state.mutation_title, "t");
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Enter)),
            Command::None
        );
        assert!(state.confirmation.is_some(), "confirmation draft shown");
    }

    #[test]
    fn github_mutation_mode_esc_clears_title_and_no_repo_reports_distinct_error() {
        let mut state = github();
        state.set_grant(GithubWriteGrantResult {
            grant_id: "grant_1".to_string(),
            status: GrantStatus::Approved,
            repository: "octo/repo".to_string(),
            scope: GithubWriteScope::IssuesWrite,
        });
        handle_github_workspace_key(&mut state, key(KeyCode::Char('m')));
        handle_github_workspace_key(&mut state, key(KeyCode::Char('x')));
        assert_eq!(state.mutation_title, "x");
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Esc)),
            Command::None
        );
        assert!(!state.mutation_mode);
        assert_eq!(state.mutation_title, "", "Esc clears the draft title");

        state.set_repositories(vec![]);
        handle_github_workspace_key(&mut state, key(KeyCode::Char('m')));
        handle_github_workspace_key(&mut state, key(KeyCode::Char('t')));
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Enter)),
            Command::None
        );
        assert_eq!(
            state.error.as_deref(),
            Some("Select a repository before composing a mutation")
        );
        assert!(state.confirmation.is_none());
    }

    #[test]
    fn github_confirmation_keys_confirm_or_dismiss() {
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('y'))),
            Command::ConfirmMutation
        );
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(
            handle_github_workspace_key(&mut state, key(KeyCode::Char('n'))),
            Command::None
        );
        assert!(state.confirmation.is_none(), "dismissed");
    }

    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn github_render_shows_panels_and_confirmation() {
        let mut state = github();
        state.switch_panel(GithubPanel::Repositories);
        state.confirmation = state.begin_mutation("Test issue".to_string());

        let backend = TestBackend::new(90, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_github_workspace(&state, frame, area);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(rendered.contains("octo/repo"), "{rendered}");
        assert!(rendered.contains("Confirm mutation"), "{rendered}");
        assert!(rendered.contains("create_issue"), "{rendered}");
        assert!(rendered.contains("Test issue"), "{rendered}");
    }
}
