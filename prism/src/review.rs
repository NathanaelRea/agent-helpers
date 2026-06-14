use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::session::Session;
use crate::util::{empty_dash, indent_markdown_block};

pub fn write_review_packet(session: &Session, config: &Config) -> Result<PathBuf, String> {
    let summary = session
        .pr
        .summary
        .as_ref()
        .ok_or_else(|| "no pull request found for selected branch".to_string())?;
    let details = session.pr.details.clone().unwrap_or_default();
    let dir = session.path.join(&config.review_packet_dir);
    fs::create_dir_all(&dir).map_err(|error| format!("create review packet dir: {error}"))?;
    let path = dir.join(format!("{}.md", summary.number));

    let mut comments = details.comments;
    comments.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.author.cmp(&b.author))
            .then_with(|| a.body.cmp(&b.body))
    });
    let mut reviews = details.reviews;
    reviews.sort_by(|a, b| {
        a.submitted_at
            .cmp(&b.submitted_at)
            .then_with(|| a.author.cmp(&b.author))
            .then_with(|| a.state.cmp(&b.state))
            .then_with(|| a.body.cmp(&b.body))
    });
    let mut review_comments = details.review_comments;
    review_comments.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.author.cmp(&b.author))
            .then_with(|| a.body.cmp(&b.body))
    });
    let mut files = details.files;
    files.sort();
    files.dedup();
    let mut failing_checks = details.failing_checks;
    failing_checks.sort();
    failing_checks.dedup();

    let mut text = String::new();
    text.push_str(&format!("# PR #{} Review Packet\n\n", summary.number));
    text.push_str("## Pull Request\n\n");
    text.push_str(&format!("- Title: {}\n", summary.title));
    text.push_str(&format!("- URL: {}\n", summary.url));
    text.push_str(&format!("- Branch: {}\n", summary.head_ref));
    text.push_str(&format!("- Base: {}\n", summary.base_ref));
    text.push_str(&format!("- State: {}\n", summary.state));
    text.push_str(&format!("- Review decision: {}\n", summary.review_decision));
    text.push_str(&format!("- Checks: {}\n", summary.check_status));
    text.push_str(&format!("- Head SHA: {}\n\n", summary.head_sha));

    text.push_str("## Failing Checks\n\n");
    if failing_checks.is_empty() {
        text.push_str("None detected.\n\n");
    } else {
        for check in &failing_checks {
            text.push_str(&format!("- {check}\n"));
        }
        text.push('\n');
    }

    text.push_str("## Changed Files\n\n");
    if files.is_empty() {
        text.push_str("None detected.\n\n");
    } else {
        for file in &files {
            text.push_str(&format!("- {file}\n"));
        }
        text.push('\n');
    }

    text.push_str("## Conversation Comments\n\n");
    if comments.is_empty() {
        text.push_str("None detected.\n\n");
    } else {
        for comment in &comments {
            text.push_str(&format!(
                "### {} {}\n\n{}\n\n",
                empty_dash(&comment.author),
                empty_dash(&comment.created_at),
                comment.body.trim()
            ));
        }
    }

    text.push_str("## Reviews\n\n");
    if reviews.is_empty() {
        text.push_str("None detected.\n\n");
    } else {
        for review in &reviews {
            text.push_str(&format!(
                "### {} {} {}\n\n{}\n\n",
                empty_dash(&review.state),
                empty_dash(&review.author),
                empty_dash(&review.submitted_at),
                review.body.trim()
            ));
        }
    }

    text.push_str("## Inline Review Comments\n\n");
    if review_comments.is_empty() {
        text.push_str("None detected.\n\n");
    } else {
        let mut current_file = String::new();
        for comment in &review_comments {
            if comment.path != current_file {
                current_file = comment.path.clone();
                text.push_str(&format!("### {}\n\n", empty_dash(&current_file)));
            }
            let line = if comment.line.is_empty() {
                "-".to_string()
            } else {
                format!("line {}", comment.line)
            };
            text.push_str(&format!(
                "- {} {} {}:\n\n{}\n\n",
                line,
                empty_dash(&comment.author),
                empty_dash(&comment.created_at),
                indent_markdown_block(comment.body.trim())
            ));
        }
    }

    text.push_str("## Requested Changes\n\n");
    if summary.review_decision == "CHANGES_REQUESTED"
        || reviews
            .iter()
            .any(|review| review.state == "CHANGES_REQUESTED")
    {
        text.push_str("Changes have been requested on this pull request.\n\n");
    } else {
        text.push_str("No explicit changes-requested review state detected.\n\n");
    }

    text.push_str("## Suggested Agent Prompt\n\n");
    text.push_str(&format!(
        "Here are some comments on PR {}. If they are applicable, fix them. Otherwise, say why not.\n\n",
        summary.number
    ));
    text.push_str("Make the requested changes, run relevant checks, and summarize what changed.\n");

    fs::write(&path, text).map_err(|error| format!("write review packet: {error}"))?;
    Ok(path)
}
