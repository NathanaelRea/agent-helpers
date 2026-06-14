use std::io::{self, Write};

use crate::agent::output_tail;
use crate::config::Config;
use crate::repo::Repository;
use crate::session::Session;
use crate::terminal::terminal_size;
use crate::util::truncate_line;

pub fn draw(
    repo: &Repository,
    config: &Config,
    sessions: &[Session],
    selected: usize,
    mode_label: &str,
) -> Result<(), String> {
    let (cols, rows) = terminal_size();
    print!(
        "{}",
        render_frame(repo, config, sessions, selected, mode_label, cols, rows)
    );
    io::stdout().flush().map_err(|error| error.to_string())
}

pub(crate) fn render_frame(
    repo: &Repository,
    config: &Config,
    sessions: &[Session],
    selected: usize,
    mode_label: &str,
    cols: u16,
    rows: u16,
) -> String {
    let left_width = cols.clamp(36, 52);
    let pr_width = if cols >= 110 { 34 } else { 0 };
    let center_width = cols
        .saturating_sub(left_width + pr_width + if pr_width > 0 { 2 } else { 1 })
        .max(24);
    let mut frame = String::from("\x1b[?25l\x1b[H");
    if pr_width > 0 {
        push_line(
            &mut frame,
            cols,
            format!(
                "{:<left_width$}| {:<center_width$}| {:<pr_width$}",
                "Sessions / Worktrees",
                "Agent Session",
                "PR / Review Context",
                left_width = left_width as usize,
                center_width = center_width.saturating_sub(2) as usize,
                pr_width = pr_width.saturating_sub(2) as usize
            ),
        );
        push_line(
            &mut frame,
            cols,
            format!(
                "{}+{}+{}",
                "-".repeat(left_width as usize),
                "-".repeat(center_width as usize),
                "-".repeat(pr_width as usize)
            ),
        );
    } else {
        push_line(
            &mut frame,
            cols,
            format!(
                "{:<left_width$}| {:<center_width$}",
                "Sessions / Worktrees",
                "Agent / PR",
                left_width = left_width as usize,
                center_width = center_width.saturating_sub(2) as usize
            ),
        );
        push_line(
            &mut frame,
            cols,
            format!(
                "{}+{}",
                "-".repeat(left_width as usize),
                "-".repeat(center_width as usize)
            ),
        );
    }

    let visible_rows = rows.saturating_sub(4) as usize;
    let start = if selected >= visible_rows {
        selected + 1 - visible_rows
    } else {
        0
    };
    let selected_session = sessions.get(selected);
    let agent_lines = format_agent_panel_lines(selected_session, mode_label);
    let pr_lines = format_pr_panel_lines(selected_session);

    for row in 0..visible_rows {
        let index = start + row;
        let left = if let Some(session) = sessions.get(index) {
            format_session_row(session, index == selected, left_width as usize)
        } else {
            " ".repeat(left_width as usize)
        };
        let center = if index == selected || row < agent_lines.len() {
            agent_lines.get(row).cloned().unwrap_or_default()
        } else if row == 0 {
            format!(
                "default agent: {}",
                truncate_line(&config.default_agent, center_width as usize - 2)
            )
        } else {
            String::new()
        };
        if pr_width > 0 {
            let pr = pr_lines.get(row).cloned().unwrap_or_default();
            push_line(
                &mut frame,
                cols,
                format!(
                    "{left}| {:<center_width$}| {:<pr_width$}",
                    truncate_line(&center, center_width.saturating_sub(2) as usize),
                    truncate_line(&pr, pr_width.saturating_sub(2) as usize),
                    center_width = center_width.saturating_sub(2) as usize,
                    pr_width = pr_width.saturating_sub(2) as usize
                ),
            );
        } else {
            let merged = if row < agent_lines.len() {
                center
            } else {
                pr_lines
                    .get(row - agent_lines.len())
                    .cloned()
                    .unwrap_or_default()
            };
            push_line(
                &mut frame,
                cols,
                format!(
                    "{left}| {:<center_width$}",
                    truncate_line(&merged, center_width.saturating_sub(2) as usize),
                    center_width = center_width.saturating_sub(2) as usize
                ),
            );
        }
    }

    let footer = format!(
        " {mode_label}  i/enter agent  c create  n plan  x run-plan  P PR  R packet  f fix  m commit  u push  a remove  D delete  q quit  repo {} ",
        repo.root.display()
    );
    if pr_width > 0 {
        push_line(
            &mut frame,
            cols,
            format!(
                "{}+{}+{}",
                "-".repeat(left_width as usize),
                "-".repeat(center_width as usize),
                "-".repeat(pr_width as usize)
            ),
        );
    } else {
        push_line(
            &mut frame,
            cols,
            format!(
                "{}+{}",
                "-".repeat(left_width as usize),
                "-".repeat(center_width as usize)
            ),
        );
    }
    frame.push_str(&fit_line(&footer, cols as usize));
    frame
}

fn push_line(frame: &mut String, cols: u16, line: String) {
    frame.push_str(&fit_line(&line, cols as usize));
    frame.push('\n');
}

fn fit_line(line: &str, cols: usize) -> String {
    let mut line = truncate_line(line, cols);
    let len = line.chars().count();
    if len < cols {
        line.push_str(&" ".repeat(cols - len));
    }
    line
}

fn format_session_row(session: &Session, selected: bool, width: usize) -> String {
    let marker = if selected { ">" } else { " " };
    let adopted = if session.adopted {
        "tracked"
    } else {
        "untracked"
    };
    let summary = if session.prompt_summary.is_empty() {
        "-"
    } else {
        &session.prompt_summary
    };
    let text = format!(
        "{marker} {:22} {:13} {:13} {:9} {}",
        truncate_line(&session.branch, 22),
        session.status_label,
        session.agent_state.label(),
        adopted,
        truncate_line(summary, 50)
    );
    format!("{:<width$}", truncate_line(&text, width), width = width)
}

fn format_agent_panel_lines(session: Option<&Session>, mode_label: &str) -> Vec<String> {
    let Some(session) = session else {
        return vec!["No worktrees discovered".to_string()];
    };
    let summary = if session.prompt_summary.is_empty() {
        "No stored prompt summary"
    } else {
        &session.prompt_summary
    };
    let output_tail = output_tail(&session.agent_output);
    let mut lines = vec![
        format!("branch: {}", session.branch),
        format!("mode: {mode_label}"),
        format!("agent: {}", session.agent_state.label()),
        format!("git: {}", session.status_label),
        format!("path: {}", session.path.display()),
        format!("prompt: {summary}"),
    ];
    if !output_tail.is_empty() {
        lines.push(format!("last output: {output_tail}"));
    }
    lines
}

fn format_pr_panel_lines(session: Option<&Session>) -> Vec<String> {
    let Some(session) = session else {
        return vec!["No selected worktree".to_string()];
    };
    if let Some(error) = &session.pr.error {
        return vec![
            "PR refresh error".to_string(),
            truncate_line(error, 120),
            "Press r to retry".to_string(),
        ];
    }
    let Some(summary) = &session.pr.summary else {
        let refreshed = session
            .pr
            .last_refreshed
            .as_deref()
            .unwrap_or("not refreshed");
        return vec![
            "No PR detected".to_string(),
            format!("branch: {}", session.branch),
            format!("last: {refreshed}"),
            "P creates one explicitly".to_string(),
        ];
    };
    let mut lines = vec![
        format!("PR #{} {}", summary.number, summary.state),
        truncate_line(&summary.title, 80),
        format!("{} -> {}", summary.head_ref, summary.base_ref),
        format!("review: {}", summary.review_decision),
        format!("checks: {}", summary.check_status),
    ];
    if let Some(details) = &session.pr.details {
        lines.push(format!(
            "comments: {}  reviews: {}",
            details.comments.len(),
            details.reviews.len()
        ));
        lines.push(format!(
            "inline comments: {}",
            details.review_comments.len()
        ));
        lines.push(format!("files: {}", details.files.len()));
    } else {
        lines.push("details: cached/pending".to_string());
    }
    if summary.draft {
        lines.push("draft".to_string());
    }
    if summary.merged {
        lines.push("merged".to_string());
    }
    if let Some(refreshed) = &session.pr.last_refreshed {
        lines.push(format!("last refreshed: {refreshed}"));
    }
    if let Some(details) = &session.pr.details {
        if !details.failing_checks.is_empty() {
            lines.push("failing checks:".to_string());
            for check in &details.failing_checks {
                lines.push(format!("  {check}"));
            }
        }
        if !details.files.is_empty() {
            lines.push("changed files:".to_string());
            for file in details.files.iter().take(4) {
                lines.push(format!("  {file}"));
            }
        }
        if !details.comments.is_empty() {
            lines.push(format!(
                "comment: {}",
                truncate_line(&details.comments[0].body, 80)
            ));
        }
        if !details.reviews.is_empty() {
            lines.push(format!("latest review: {}", details.reviews[0].state));
        }
        if !details.review_comments.is_empty() {
            lines.push(format!(
                "inline: {}",
                truncate_line(&details.review_comments[0].body, 80)
            ));
        }
    }
    if !summary.url.is_empty() {
        lines.push(summary.url.clone());
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::path::PathBuf;

    use crate::agent::AgentState;
    use crate::config::{Checks, Config, EscapeKey};
    use crate::github::PrCache;
    use crate::repo::Repository;
    use crate::session::Session;

    use super::render_frame;

    #[test]
    fn render_frame_does_not_clear_the_whole_screen() {
        let repo = Repository {
            root: PathBuf::from("/repo"),
        };
        let config = Config {
            default_agent: "codex".to_string(),
            default_base: None,
            plan_dir: "plans".to_string(),
            review_packet_dir: ".agent/review".to_string(),
            worktree_command: "wt".to_string(),
            escape_key: EscapeKey::EscEsc,
            checks: Checks::default(),
            tools: BTreeMap::new(),
            agent_commands: BTreeMap::new(),
            agent_prompt_modes: BTreeMap::new(),
            user_path: PathBuf::from("/tmp/user.toml"),
            repo_path: PathBuf::from("/repo/.prism.toml"),
        };
        let sessions = vec![Session {
            path: PathBuf::from("/repo"),
            path_display: "/repo".to_string(),
            branch: "main".to_string(),
            prompt_summary: "summary".to_string(),
            adopted: true,
            hidden: false,
            status_label: "clean".to_string(),
            agent: None,
            agent_output: VecDeque::new(),
            agent_state: AgentState::Idle,
            pr: PrCache::default(),
        }];

        let frame = render_frame(&repo, &config, &sessions, 0, "normal", 120, 20);

        assert!(frame.starts_with("\x1b[?25l\x1b[H"));
        assert!(!frame.contains("\x1b[2J"));
        assert!(!frame.contains("\x1b[2K"));
    }
}
