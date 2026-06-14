use std::process::{Command, Stdio};

use crate::config::Config;
use crate::process::run_capture;
use crate::repo::Repository;

pub fn git_status_label(path: &std::path::Path, config: &Config) -> String {
    match run_capture(
        Command::new(config.tool("git"))
            .arg("-C")
            .arg(path)
            .args(["status", "--short", "--branch"]),
    ) {
        Ok(output) => parse_git_status_label(&output),
        Err(_) => "status error".to_string(),
    }
}

pub fn parse_git_status_label(output: &str) -> String {
    let mut branch = "";
    let mut dirty = false;
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            branch = rest;
        } else if !line.trim().is_empty() {
            dirty = true;
        }
    }
    let relation = if branch.contains("ahead ") && branch.contains("behind ") {
        "diverged"
    } else if branch.contains("ahead ") {
        "ahead"
    } else if branch.contains("behind ") {
        "behind"
    } else {
        "clean"
    };
    if dirty {
        if relation == "clean" {
            "dirty".to_string()
        } else {
            format!("dirty {relation}")
        }
    } else {
        relation.to_string()
    }
}

pub fn worktree_dirty(repo: &Repository, config: &Config) -> Result<bool, String> {
    let status = run_capture(
        Command::new(config.tool("git"))
            .arg("-C")
            .arg(&repo.root)
            .args(["status", "--short"]),
    )?;
    Ok(!status.trim().is_empty())
}

pub fn selected_dirty(path: &std::path::Path, config: &Config) -> Result<bool, String> {
    let status = run_capture(
        Command::new(config.tool("git"))
            .arg("-C")
            .arg(path)
            .args(["status", "--short"]),
    )?;
    Ok(!status.trim().is_empty())
}

pub fn has_upstream(path: &std::path::Path, config: &Config) -> Result<bool, String> {
    let upstream = Command::new(config.tool("git"))
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("git upstream check: {error}"))?;
    Ok(upstream.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_label_reports_clean_ahead_and_dirty() {
        assert_eq!(parse_git_status_label("## main...origin/main\n"), "clean");
        assert_eq!(
            parse_git_status_label("## main...origin/main [ahead 1]\n"),
            "ahead"
        );
        assert_eq!(
            parse_git_status_label("## main...origin/main [behind 1]\n M src/main.rs\n"),
            "dirty behind"
        );
        assert_eq!(
            parse_git_status_label("## main...origin/main [ahead 1, behind 1]\n"),
            "diverged"
        );
    }
}
