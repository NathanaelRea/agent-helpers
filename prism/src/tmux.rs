use std::path::Path;
use std::process::Command;

use crate::config::Config;
use crate::process::{run_status, split_command_words};
use crate::repo::Repository;
use crate::session::Session;
use crate::util::safe_branch_filename;

pub fn attach_or_create_agent(
    repo: &Repository,
    config: &Config,
    session: &Session,
) -> Result<(), String> {
    let name = agent_session_name(repo, &session.branch);
    if session_exists(config, &name) {
        return attach(config, &name);
    }

    let command = agent_shell_command(config)?;
    run_status(
        Command::new(config.tool("tmux"))
            .env_remove("TMUX")
            .args(["new-session", "-s"])
            .arg(&name)
            .arg("-c")
            .arg(&session.path)
            .arg(command),
    )
}

pub fn agent_session_exists(repo: &Repository, config: &Config, session: &Session) -> bool {
    session_exists(config, &agent_session_name(repo, &session.branch))
}

pub fn agent_session_name(repo: &Repository, branch: &str) -> String {
    let hash = stable_hash(repo.root.as_path());
    let branch = safe_tmux_name(&safe_branch_filename(branch));
    format!("prism-{hash:016x}-{branch}")
}

fn attach(config: &Config, name: &str) -> Result<(), String> {
    run_status(Command::new(config.tool("tmux")).env_remove("TMUX").args([
        "attach-session",
        "-t",
        name,
    ]))
}

fn session_exists(config: &Config, name: &str) -> bool {
    Command::new(config.tool("tmux"))
        .env_remove("TMUX")
        .args(["has-session", "-t", name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn agent_shell_command(config: &Config) -> Result<String, String> {
    let argv = split_command_words(&config.agent_command(&config.default_agent));
    if argv.is_empty() {
        return Err(format!(
            "agent '{}' has an empty command",
            config.default_agent
        ));
    }
    if argv.iter().any(|arg| arg.contains("{prompt")) {
        return Err(format!(
            "agent '{}' command contains a prompt placeholder; configure an interactive command for tmux attach",
            config.default_agent
        ));
    }
    Ok(argv
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" "))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn stable_hash(path: &Path) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in path.display().to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn safe_tmux_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::config::{Checks, Config, EscapeKey};
    use crate::repo::Repository;

    use super::{agent_session_name, shell_quote};

    #[test]
    fn tmux_session_names_are_stable_and_safe() {
        let repo = Repository {
            root: PathBuf::from("/repo/my project"),
        };

        let name = agent_session_name(&repo, "feature/foo:bar");

        assert!(name.starts_with("prism-"));
        assert!(name.ends_with("-feature_foo_bar"));
        assert!(!name.contains('/'));
        assert!(!name.contains(':'));
    }

    #[test]
    fn shell_quote_preserves_argument_boundaries() {
        assert_eq!(shell_quote("codex"), "codex");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("two words"), "'two words'");
        assert_eq!(shell_quote("that's"), "'that'\"'\"'s'");
    }

    #[test]
    fn rejects_prompt_placeholder_for_interactive_tmux_command() {
        let config = Config {
            default_agent: "custom".to_string(),
            default_base: None,
            plan_dir: "plans".to_string(),
            review_packet_dir: ".agent/review".to_string(),
            worktree_command: "wt".to_string(),
            escape_key: EscapeKey::EscEsc,
            checks: Checks::default(),
            tools: BTreeMap::new(),
            agent_commands: BTreeMap::from([(
                "custom".to_string(),
                "custom-agent --prompt {prompt}".to_string(),
            )]),
            agent_prompt_modes: BTreeMap::new(),
            user_path: PathBuf::from("/tmp/user.toml"),
            repo_path: PathBuf::from("/repo/.prism.toml"),
        };

        let error = super::agent_shell_command(&config).unwrap_err();

        assert!(error.contains("prompt placeholder"));
    }
}
