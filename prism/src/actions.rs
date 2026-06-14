use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use crate::agent::{AgentAdapter, AgentProcess, AgentState};
use crate::git::{has_upstream, selected_dirty, worktree_dirty};
use crate::github::{PR_POLL_INTERVAL, refresh_pr_cache, remove_pr_cache};
use crate::plan::{build_plan_prompt, default_plan_path, infer_total_phases, run_codex_plan};
use crate::process::{run_configured_commands, run_status};
use crate::review::write_review_packet;
use crate::session::{
    append_agent_log, clear_hidden, discover_sessions, mark_hidden, remove_logs,
    remove_process_state, remove_task_metadata, save_agent_state, write_task_metadata,
};
use crate::tui::Tui;
use crate::util::{truncate, yes};

impl Tui {
    pub(crate) fn refresh_sessions(&mut self) -> Result<(), String> {
        let old = std::mem::take(&mut self.sessions);
        let mut by_path = old
            .into_iter()
            .map(|session| (session.path.clone(), session))
            .collect::<BTreeMap<_, _>>();
        let mut fresh = discover_sessions(&self.repo, &self.config)?;
        for session in &mut fresh {
            if let Some(mut previous) = by_path.remove(&session.path) {
                session.agent = previous.agent.take();
                session.agent_output = previous.agent_output;
                session.agent_state = previous.agent_state;
                session.pr = previous.pr;
            }
        }
        self.sessions = fresh;
        if self.selected >= self.sessions.len() {
            self.selected = self.sessions.len().saturating_sub(1);
        }
        Ok(())
    }

    pub(crate) fn create_session(&mut self) -> Result<(), String> {
        if !self.allow_dirty && worktree_dirty(&self.repo, &self.config)? {
            self.show_message(
                "current worktree is dirty; restart Prism with --allow-dirty to create anyway",
            )?;
            return Ok(());
        }
        let branch = self.prompt_line("Branch name: ")?;
        if branch.trim().is_empty() {
            return Ok(());
        }
        let initial_prompt = self.prompt_line("Initial prompt (optional): ")?;
        self.show_message(&format!("creating worktree for {branch}"))?;
        run_status(
            Command::new(self.config.tool(&self.config.worktree_command))
                .current_dir(&self.repo.root)
                .args(["switch", "-c", branch.trim()]),
        )?;
        clear_hidden(&self.repo, branch.trim())?;
        self.refresh_sessions()?;
        let index = self
            .sessions
            .iter()
            .position(|session| session.branch == branch.trim())
            .ok_or_else(|| {
                format!(
                    "created branch '{}' was not found in git worktree list",
                    branch.trim()
                )
            })?;
        self.selected = index;
        if !initial_prompt.trim().is_empty() {
            write_task_metadata(&self.repo, &self.sessions[index], &initial_prompt)?;
            self.sessions[index].prompt_summary = truncate(&initial_prompt.replace('\n', " "), 50);
            self.sessions[index].adopted = true;
        }
        self.launch_agent(index, &initial_prompt)?;
        Ok(())
    }

    pub(crate) fn launch_agent(
        &mut self,
        index: usize,
        initial_prompt: &str,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(index)
            .ok_or_else(|| "no selected session".to_string())?;
        let adapter = AgentAdapter::from_config(&self.config, &self.config.default_agent);
        let prompt = initial_prompt.trim();
        let launch = adapter.prepare_launch(prompt)?;
        let argv = launch.argv;
        if argv.is_empty() {
            return Err(format!(
                "agent '{}' has an empty command",
                self.config.default_agent
            ));
        }
        let mut agent = AgentProcess::spawn(&argv, &session.path, launch.prompt_file)?;
        if let Some(stdin_prompt) = launch.stdin_prompt {
            agent.write_all(format!("{stdin_prompt}\n").as_bytes())?;
        }
        session.agent = Some(agent);
        session.agent_state = AgentState::Running;
        let _ = save_agent_state(&self.repo, &session.branch, session.agent_state);
        session.agent_output.clear();
        session.agent_output.push_back(format!(
            "started {} ({})",
            self.config.default_agent,
            adapter.prompt_mode.label()
        ));
        Ok(())
    }

    pub(crate) fn poll_agents(&mut self) {
        for session in &mut self.sessions {
            if let Some(agent) = &mut session.agent {
                for chunk in agent.drain_output() {
                    let _ = append_agent_log(&self.repo, &session.branch, &chunk);
                    session.agent_output.push_back(chunk);
                }
                while session.agent_output.len() > 200 {
                    session.agent_output.pop_front();
                }
                if session.agent_state == AgentState::Running {
                    if let Some(state) = agent.try_wait() {
                        session.agent_state = state;
                        let _ = save_agent_state(&self.repo, &session.branch, state);
                    }
                }
            }
        }
    }

    pub(crate) fn poll_pull_requests(&mut self, force: bool) {
        for session in &mut self.sessions {
            let due = session
                .pr
                .last_polled
                .map(|last| last.elapsed() >= PR_POLL_INTERVAL)
                .unwrap_or(true);
            if force || due {
                refresh_pr_cache(
                    &self.repo,
                    &session.branch,
                    &mut session.pr,
                    &session.path,
                    &self.config,
                    force,
                );
            }
        }
    }

    pub(crate) fn create_or_update_pr(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        {
            let session = &mut self.sessions[self.selected];
            refresh_pr_cache(
                &self.repo,
                &session.branch,
                &mut session.pr,
                &session.path,
                &self.config,
                false,
            );
        }
        if let Some(summary) = &self.sessions[self.selected].pr.summary {
            let message = format!("PR #{} {}", summary.number, summary.url);
            self.show_message(&message)?;
            return Ok(());
        }
        let path = self.sessions[self.selected].path.clone();
        let branch = self.sessions[self.selected].branch.clone();

        if selected_dirty(&path, &self.config)? {
            self.show_message("working tree is dirty; commit or stash before creating a PR")?;
            return Ok(());
        }

        run_configured_commands(&self.config.checks.pre_pr, &path, "pre_pr")?;
        run_configured_commands(&self.config.checks.pre_push, &path, "pre_push")?;

        let push = self.prompt_line("No PR found. Push branch and create PR? [y/N] ")?;
        if !yes(&push) {
            return Ok(());
        }
        self.show_message("pushing branch")?;
        run_status(
            Command::new(self.config.tool("git"))
                .arg("-C")
                .arg(&path)
                .args(["push", "-u", "origin", &branch]),
        )?;
        self.show_message("creating pull request")?;
        run_status(
            Command::new(self.config.tool("gh"))
                .arg("pr")
                .arg("create")
                .arg("--fill")
                .current_dir(&path),
        )?;
        {
            let session = &mut self.sessions[self.selected];
            refresh_pr_cache(
                &self.repo,
                &session.branch,
                &mut session.pr,
                &session.path,
                &self.config,
                true,
            );
        }
        if let Some(summary) = &self.sessions[self.selected].pr.summary {
            let message = format!("created PR #{} {}", summary.number, summary.url);
            self.show_message(&message)?;
        }
        Ok(())
    }

    pub(crate) fn refresh_review_packet(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        {
            let session = &mut self.sessions[self.selected];
            refresh_pr_cache(
                &self.repo,
                &session.branch,
                &mut session.pr,
                &session.path,
                &self.config,
                true,
            );
        }
        let path = write_review_packet(&self.sessions[self.selected], &self.config)?;
        self.show_message(&format!("wrote {}", path.display()))?;
        Ok(())
    }

    pub(crate) fn start_review_fix(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        if self.sessions[self.selected].agent_state == AgentState::Running {
            self.show_message("agent is already running; wait or select another session")?;
            return Ok(());
        }
        {
            let session = &mut self.sessions[self.selected];
            refresh_pr_cache(
                &self.repo,
                &session.branch,
                &mut session.pr,
                &session.path,
                &self.config,
                true,
            );
        }
        let path = self.sessions[self.selected].path.clone();
        run_configured_commands(&self.config.checks.review_fix, &path, "review_fix")?;
        let packet_path = write_review_packet(&self.sessions[self.selected], &self.config)?;
        let packet = fs::read_to_string(&packet_path)
            .map_err(|error| format!("read review packet: {error}"))?;
        let pr_number = self.sessions[self.selected]
            .pr
            .summary
            .as_ref()
            .map(|summary| summary.number.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let prompt = format!(
            "Here are some comments on PR {pr_number}. If they are applicable, fix them. Otherwise, say why not.\n\n{packet}\n\nMake the requested changes, run relevant checks, and summarize what changed."
        );
        self.launch_agent(self.selected, &prompt)?;
        self.show_message("started review-fix agent session")?;
        Ok(())
    }

    pub(crate) fn commit_review_fix(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        let path = self.sessions[self.selected].path.clone();
        if !selected_dirty(&path, &self.config)? {
            self.show_message("nothing to commit")?;
            return Ok(());
        }
        let answer = self.prompt_line("Commit all changes as 'fix: code review'? [y/N] ")?;
        if !yes(&answer) {
            return Ok(());
        }
        run_status(
            Command::new(self.config.tool("git"))
                .arg("-C")
                .arg(&path)
                .args(["add", "-A"]),
        )?;
        run_status(
            Command::new(self.config.tool("git"))
                .arg("-C")
                .arg(&path)
                .args(["commit", "-m", "fix: code review"]),
        )?;
        self.refresh_sessions()?;
        self.show_message("created commit: fix: code review")?;
        Ok(())
    }

    pub(crate) fn push_selected_branch(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        let path = self.sessions[self.selected].path.clone();
        let branch = self.sessions[self.selected].branch.clone();
        if branch == "(detached)" {
            self.show_message("cannot push a detached worktree")?;
            return Ok(());
        }
        run_configured_commands(&self.config.checks.pre_push, &path, "pre_push")?;
        let args = if has_upstream(&path, &self.config)? {
            vec!["push".to_string()]
        } else {
            let answer =
                self.prompt_line(&format!("No upstream. Push -u origin {branch}? [y/N] "))?;
            if !yes(&answer) {
                return Ok(());
            }
            vec![
                "push".to_string(),
                "-u".to_string(),
                "origin".to_string(),
                branch,
            ]
        };
        self.show_message("pushing branch")?;
        run_status(
            Command::new(self.config.tool("git"))
                .arg("-C")
                .arg(&path)
                .args(args),
        )?;
        {
            let session = &mut self.sessions[self.selected];
            refresh_pr_cache(
                &self.repo,
                &session.branch,
                &mut session.pr,
                &session.path,
                &self.config,
                true,
            );
        }
        self.show_message("push complete")?;
        Ok(())
    }

    pub(crate) fn create_plan(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        if self.sessions[self.selected].agent_state == AgentState::Running {
            self.show_message("agent is already running; wait or select another session")?;
            return Ok(());
        }
        let path = default_plan_path(&self.sessions[self.selected], &self.config);
        let request = self.prompt_line("Plan request (optional): ")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("create plan dir: {error}"))?;
        }
        let prompt = build_plan_prompt(&self.sessions[self.selected], &path, &request);
        self.launch_agent(self.selected, &prompt)?;
        self.show_message(&format!("started planning agent for {}", path.display()))?;
        Ok(())
    }

    pub(crate) fn run_selected_plan(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        let session = &self.sessions[self.selected];
        let plan_path = default_plan_path(session, &self.config);
        if !plan_path.is_file() {
            return Err(format!("plan file not found: {}", plan_path.display()));
        }
        let inferred_total = infer_total_phases(&plan_path)?;
        let total = if inferred_total > 0 {
            inferred_total
        } else {
            let input = self.prompt_line("Total phases: ")?;
            input
                .trim()
                .parse::<usize>()
                .map_err(|_| "total phases must be a positive integer".to_string())?
        };
        if total == 0 {
            return Err("total phases must be positive".to_string());
        }
        let start_input = self.prompt_line("Start phase [1]: ")?;
        let start = if start_input.trim().is_empty() {
            1
        } else {
            start_input
                .trim()
                .parse::<usize>()
                .map_err(|_| "start phase must be a positive integer".to_string())?
        };
        if start == 0 || start > total {
            return Err("start phase must be between 1 and total phases".to_string());
        }
        let parallel_input = self.prompt_line("Run phases in parallel? [y/N] ")?;
        let parallel = matches!(
            parallel_input.trim(),
            "y" | "Y" | "yes" | "YES" | "true" | "TRUE"
        );
        let answer = self.prompt_line(&format!(
            "Run {} phases from {} starting at {}? [y/N] ",
            total,
            plan_path.display(),
            start
        ))?;
        if !yes(&answer) {
            return Ok(());
        }
        run_codex_plan(session, &self.config, &plan_path, total, start, parallel)
    }

    pub(crate) fn remove_session_from_board(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        let branch = self.sessions[self.selected].branch.clone();
        let answer = self.prompt_line(&format!("Remove {branch} from Prism board only? [y/N] "))?;
        if !yes(&answer) {
            return Ok(());
        }
        mark_hidden(&self.repo, &branch)?;
        self.refresh_sessions()?;
        self.show_message("session removed from board")?;
        Ok(())
    }

    pub(crate) fn delete_session(&mut self) -> Result<(), String> {
        if self.selected >= self.sessions.len() {
            return Ok(());
        }
        let branch = self.sessions[self.selected].branch.clone();
        let path = self.sessions[self.selected].path.clone();
        let adopted = self.sessions[self.selected].adopted;
        let warning = if adopted {
            "Delete local Prism data, worktree, and local branch? [y/N] "
        } else {
            "Untracked worktree. Type Y to delete worktree and local branch; y hides only: "
        };
        let answer = self.prompt_line(warning)?;
        if answer.trim() == "Y" {
            self.delete_local_data(&branch)?;
            run_status(
                Command::new(self.config.tool("git"))
                    .arg("-C")
                    .arg(&self.repo.root)
                    .args(["worktree", "remove", "--force"])
                    .arg(&path),
            )?;
            if branch != "(detached)" {
                run_status(
                    Command::new(self.config.tool("git"))
                        .arg("-C")
                        .arg(&self.repo.root)
                        .args(["branch", "-D", &branch]),
                )?;
            }
            self.refresh_sessions()?;
            self.show_message("deleted local session data, worktree, and branch")?;
        } else if !adopted && yes(&answer) {
            mark_hidden(&self.repo, &branch)?;
            self.refresh_sessions()?;
            self.show_message("untracked session hidden")?;
        }
        Ok(())
    }

    fn delete_local_data(&self, branch: &str) -> Result<(), String> {
        remove_task_metadata(&self.repo, branch)?;
        remove_pr_cache(&self.repo, branch)?;
        remove_logs(&self.repo, branch)?;
        remove_process_state(&self.repo, branch)?;
        clear_hidden(&self.repo, branch)?;
        Ok(())
    }
}
