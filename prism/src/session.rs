use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::agent::{AgentProcess, AgentState};
use crate::config::Config;
use crate::git::git_status_label;
use crate::github::{PrCache, load_pr_cache};
use crate::json::{json_escape, json_string_field};
use crate::process::run_capture;
use crate::repo::Repository;
use crate::util::{safe_branch_filename, truncate};

#[derive(Debug)]
pub struct Session {
    pub path: PathBuf,
    pub path_display: String,
    pub branch: String,
    pub prompt_summary: String,
    pub adopted: bool,
    pub hidden: bool,
    pub status_label: String,
    pub agent: Option<AgentProcess>,
    pub agent_output: VecDeque<String>,
    pub agent_state: AgentState,
    pub pr: PrCache,
}

pub fn discover_sessions(repo: &Repository, config: &Config) -> Result<Vec<Session>, String> {
    let output = run_capture(
        Command::new(config.tool("git"))
            .arg("-C")
            .arg(&repo.root)
            .args(["worktree", "list", "--porcelain"]),
    )?;
    let mut sessions = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(path) = current_path.take() {
                let branch = current_branch
                    .take()
                    .unwrap_or_else(|| "(detached)".to_string());
                let session = build_session(repo, path, branch, config);
                if !session.hidden {
                    sessions.push(session);
                }
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(path));
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string(),
            );
        } else if line.starts_with("detached") {
            current_branch = Some("(detached)".to_string());
        }
    }

    sessions.sort_by(|a, b| a.branch.cmp(&b.branch).then_with(|| a.path.cmp(&b.path)));
    Ok(sessions)
}

fn build_session(repo: &Repository, path: PathBuf, branch: String, config: &Config) -> Session {
    let metadata_path = task_metadata_path(repo, &branch);
    let legacy_metadata_path = path
        .join(".agent/tasks")
        .join(format!("{}.json", safe_branch_filename(&branch)));
    let prompt_summary = read_prompt_summary(&metadata_path)
        .or_else(|| read_prompt_summary(&legacy_metadata_path))
        .unwrap_or_default();
    let adopted = metadata_path.exists() || legacy_metadata_path.exists();
    let hidden = hidden_path(repo, &branch).exists();
    let status_label = git_status_label(&path, config);
    let path_display = path.display().to_string();
    let agent_state = load_agent_state(repo, &branch).unwrap_or(AgentState::Idle);
    let pr = load_pr_cache(repo, &branch);
    Session {
        path,
        path_display,
        branch,
        prompt_summary,
        adopted,
        hidden,
        status_label,
        agent: None,
        agent_output: VecDeque::new(),
        agent_state,
        pr,
    }
}

pub fn write_task_metadata(
    repo: &Repository,
    session: &Session,
    initial_prompt: &str,
) -> Result<(), String> {
    let path = task_metadata_path(repo, &session.branch);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create task metadata dir: {error}"))?;
    }
    let summary = truncate(&initial_prompt.replace('\n', " "), 50);
    let text = format!(
        "{{\n  \"branch\": \"{}\",\n  \"prompt_summary\": \"{}\",\n  \"initial_prompt\": \"{}\",\n  \"worktree\": \"{}\"\n}}\n",
        json_escape(&session.branch),
        json_escape(&summary),
        json_escape(initial_prompt),
        json_escape(&session.path_display)
    );
    fs::write(path, text).map_err(|error| format!("write task metadata: {error}"))
}

pub fn remove_task_metadata(repo: &Repository, branch: &str) -> Result<(), String> {
    remove_if_exists(task_metadata_path(repo, branch), "task metadata")
}

pub fn mark_hidden(repo: &Repository, branch: &str) -> Result<(), String> {
    let path = hidden_path(repo, branch);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create hidden dir: {error}"))?;
    }
    fs::write(path, b"hidden\n").map_err(|error| format!("write hidden marker: {error}"))
}

pub fn clear_hidden(repo: &Repository, branch: &str) -> Result<(), String> {
    remove_if_exists(hidden_path(repo, branch), "hidden marker")
}

pub fn append_agent_log(repo: &Repository, branch: &str, chunk: &str) -> Result<(), String> {
    let path = log_path(repo, branch);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create log dir: {error}"))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open agent log: {error}"))?;
    file.write_all(chunk.as_bytes())
        .map_err(|error| format!("write agent log: {error}"))
}

pub fn append_runtime_log(repo: &Repository, message: &str) -> Result<(), String> {
    crate::observability::append_runtime_message(repo, message)
}

pub fn remove_logs(repo: &Repository, branch: &str) -> Result<(), String> {
    remove_if_exists(log_path(repo, branch), "agent log")
}

pub fn save_agent_state(repo: &Repository, branch: &str, state: AgentState) -> Result<(), String> {
    let path = process_state_path(repo, branch);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("create process dir: {error}"))?;
    }
    fs::write(
        path,
        format!("{{\n  \"state\": \"{}\"\n}}\n", json_escape(state.label())),
    )
    .map_err(|error| format!("write process state: {error}"))
}

pub fn remove_process_state(repo: &Repository, branch: &str) -> Result<(), String> {
    remove_if_exists(process_state_path(repo, branch), "process state")
}

fn load_agent_state(repo: &Repository, branch: &str) -> Option<AgentState> {
    let text = fs::read_to_string(process_state_path(repo, branch)).ok()?;
    let state = json_string_field(&text, "state")?;
    AgentState::parse(&state)
}

fn read_prompt_summary(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    for key in ["prompt_summary", "summary", "initial_prompt", "prompt"] {
        if let Some(value) = json_string_field(&text, key) {
            return Some(truncate(&value.replace('\n', " "), 50));
        }
    }
    None
}

fn task_metadata_path(repo: &Repository, branch: &str) -> PathBuf {
    repo.prism_dir()
        .join("tasks")
        .join(format!("{}.json", safe_branch_filename(branch)))
}

fn hidden_path(repo: &Repository, branch: &str) -> PathBuf {
    repo.prism_dir()
        .join("hidden")
        .join(format!("{}.hidden", safe_branch_filename(branch)))
}

fn log_path(repo: &Repository, branch: &str) -> PathBuf {
    repo.prism_dir()
        .join("logs")
        .join(format!("{}.log", safe_branch_filename(branch)))
}

fn process_state_path(repo: &Repository, branch: &str) -> PathBuf {
    repo.prism_dir()
        .join("process")
        .join(format!("{}.json", safe_branch_filename(branch)))
}

fn remove_if_exists(path: PathBuf, label: &str) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("remove {label}: {error}"))?;
    }
    Ok(())
}
