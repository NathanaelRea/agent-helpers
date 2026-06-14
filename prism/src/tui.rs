use std::io::{self, ErrorKind, Read, Write};

use crate::agent::AgentState;
use crate::config::{Config, EscapeKey};
use crate::input::{Key, KeyInput};
use crate::repo::Repository;
use crate::session::{Session, append_runtime_log};
use crate::terminal::{RawTerminal, stdin_is_tty, terminal_size};
use crate::util::{truncate_line, yes};
use crate::view;

enum Mode {
    Normal,
    Agent { pending_escape: bool },
}

pub struct Tui {
    pub(crate) repo: Repository,
    pub(crate) config: Config,
    pub(crate) sessions: Vec<Session>,
    pub(crate) selected: usize,
    pub(crate) allow_dirty: bool,
    status_message: Option<String>,
    mode: Mode,
}

impl Tui {
    pub fn new(
        repo: Repository,
        config: Config,
        sessions: Vec<Session>,
        allow_dirty: bool,
    ) -> Self {
        Self {
            repo,
            config,
            sessions,
            selected: 0,
            allow_dirty,
            status_message: None,
            mode: Mode::Normal,
        }
    }

    pub fn run(&mut self) -> Result<(), String> {
        if !stdin_is_tty() {
            return Err("TUI requires an interactive terminal".to_string());
        }

        let mut raw = RawTerminal::enter()?;
        self.draw()?;
        let mut stdin = io::stdin();
        let mut buffer = [0_u8; 64];
        let mut key_input = KeyInput::default();
        let mut pending_g = false;
        let mut last_size = terminal_size();

        loop {
            let agents_changed = self.poll_agents();
            let prs_changed = self.poll_pull_requests(false);
            let current_size = terminal_size();
            let resized = current_size != last_size;
            if resized {
                last_size = current_size;
            }
            if agents_changed || prs_changed || resized {
                self.draw()?;
            }
            let count = match stdin.read(&mut buffer) {
                Ok(count) => count,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            };
            if count == 0 {
                continue;
            }

            if matches!(self.mode, Mode::Agent { .. }) {
                self.handle_agent_input(&buffer[..count])?;
                continue;
            }

            let mut should_quit = false;
            for key in key_input.feed(&buffer[..count]) {
                match key {
                    Key::Quit => {
                        pending_g = false;
                        should_quit = self.confirm_quit()?;
                    }
                    Key::Down => {
                        pending_g = false;
                        self.move_down();
                    }
                    Key::Up => {
                        pending_g = false;
                        self.move_up();
                    }
                    Key::Bottom => {
                        pending_g = false;
                        self.selected = self.sessions.len().saturating_sub(1);
                    }
                    Key::G => {
                        if pending_g {
                            self.selected = 0;
                            pending_g = false;
                        } else {
                            pending_g = true;
                        }
                    }
                    Key::AgentMode => {
                        pending_g = false;
                        self.enter_agent_mode()?;
                    }
                    Key::Refresh => {
                        pending_g = false;
                        self.refresh_sessions()?;
                        self.poll_pull_requests(true);
                    }
                    Key::PullRequest => {
                        pending_g = false;
                        if let Err(error) = self.create_or_update_pr() {
                            self.show_error("PR action failed", &error)?;
                        }
                    }
                    Key::ReviewPacket => {
                        pending_g = false;
                        if let Err(error) = self.refresh_review_packet() {
                            self.show_error("review packet failed", &error)?;
                        }
                    }
                    Key::ReviewFix => {
                        pending_g = false;
                        if let Err(error) = self.start_review_fix() {
                            self.show_error("review fix failed", &error)?;
                        }
                    }
                    Key::CommitReviewFix => {
                        pending_g = false;
                        if let Err(error) = self.commit_review_fix() {
                            self.show_error("commit failed", &error)?;
                        }
                    }
                    Key::Push => {
                        pending_g = false;
                        if let Err(error) = self.push_selected_branch() {
                            self.show_error("push failed", &error)?;
                        }
                    }
                    Key::CreatePlan => {
                        pending_g = false;
                        if let Err(error) = self.create_plan() {
                            self.show_error("plan creation failed", &error)?;
                        }
                    }
                    Key::RunPlan => {
                        pending_g = false;
                        raw.suspend()?;
                        let result = self.run_selected_plan();
                        print!("\nPress Enter to return to Prism...");
                        io::stdout().flush().map_err(|error| error.to_string())?;
                        let mut line = String::new();
                        let _ = io::stdin().read_line(&mut line);
                        raw.resume()?;
                        if let Err(error) = result {
                            self.show_error("plan run failed", &error)?;
                        }
                    }
                    Key::Create => {
                        pending_g = false;
                        if let Err(error) = self.create_session() {
                            self.show_error("create session failed", &error)?;
                        }
                    }
                    Key::Remove => {
                        pending_g = false;
                        if let Err(error) = self.remove_session_from_board() {
                            self.show_error("remove failed", &error)?;
                        }
                    }
                    Key::Delete => {
                        pending_g = false;
                        if let Err(error) = self.delete_session() {
                            self.show_error("delete failed", &error)?;
                        }
                    }
                    Key::Other => pending_g = false,
                }
                if should_quit {
                    break;
                }
            }
            if should_quit {
                break;
            }
            self.draw()?;
        }
        Ok(())
    }

    fn confirm_quit(&self) -> Result<bool, String> {
        if !self
            .sessions
            .iter()
            .any(|session| session.agent_state == AgentState::Running)
        {
            return Ok(true);
        }
        let answer = self.prompt_line("Agents are running. Quit Prism? [y/N] ")?;
        Ok(yes(&answer))
    }

    fn enter_agent_mode(&mut self) -> Result<(), String> {
        let Some(session) = self.sessions.get(self.selected) else {
            return Ok(());
        };
        if session.agent.is_none() {
            self.show_message("no live agent PTY for selected session")?;
            return Ok(());
        }
        self.mode = Mode::Agent {
            pending_escape: false,
        };
        self.show_message(&format!(
            "agent mode; exit with {}",
            self.config.escape_key.label()
        ))
    }

    fn handle_agent_input(&mut self, bytes: &[u8]) -> Result<(), String> {
        let escape_key = self.config.escape_key;
        for byte in bytes {
            match (&mut self.mode, escape_key, *byte) {
                (Mode::Agent { .. }, EscapeKey::CtrlSpace, 0) => {
                    self.mode = Mode::Normal;
                    self.show_message("normal mode")?;
                    continue;
                }
                (Mode::Agent { pending_escape }, EscapeKey::EscEsc, b'\x1b')
                    if !*pending_escape =>
                {
                    *pending_escape = true;
                    continue;
                }
                (Mode::Agent { pending_escape }, EscapeKey::EscEsc, b'\x1b') if *pending_escape => {
                    *pending_escape = false;
                    self.mode = Mode::Normal;
                    self.show_message("normal mode")?;
                    continue;
                }
                (Mode::Agent { pending_escape }, EscapeKey::EscEsc, byte) if *pending_escape => {
                    *pending_escape = false;
                    self.write_to_selected_agent(&[b'\x1b', byte])?;
                }
                _ => self.write_to_selected_agent(&[*byte])?,
            }
        }
        Ok(())
    }

    fn write_to_selected_agent(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(session) = self.sessions.get_mut(self.selected) else {
            return Ok(());
        };
        let Some(agent) = session.agent.as_mut() else {
            self.mode = Mode::Normal;
            self.show_message("agent exited")?;
            return Ok(());
        };
        agent.write_all(bytes)
    }

    pub(crate) fn prompt_line(&self, prompt: &str) -> Result<String, String> {
        print!("\x1b[{};1H\x1b[2K\x1b[?25h{}", terminal_size().1, prompt);
        io::stdout().flush().map_err(|error| error.to_string())?;
        let mut input = String::new();
        let mut stdin = io::stdin();
        let mut byte = [0_u8; 1];
        loop {
            match stdin.read(&mut byte) {
                Ok(1) => {}
                Ok(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                    continue;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            }
            match byte[0] {
                b'\r' | b'\n' => {
                    print!("\r\n\x1b[?25l");
                    io::stdout().flush().map_err(|error| error.to_string())?;
                    return Ok(input);
                }
                3 | 27 => {
                    print!("\r\n\x1b[?25l");
                    io::stdout().flush().map_err(|error| error.to_string())?;
                    return Ok(String::new());
                }
                8 | 127 => {
                    if input.pop().is_some() {
                        print!("\x08 \x08");
                        io::stdout().flush().map_err(|error| error.to_string())?;
                    }
                }
                byte if !byte.is_ascii_control() => {
                    let ch = byte as char;
                    input.push(ch);
                    print!("{ch}");
                    io::stdout().flush().map_err(|error| error.to_string())?;
                }
                _ => {}
            }
        }
    }

    pub(crate) fn show_message(&mut self, message: &str) -> Result<(), String> {
        self.status_message = Some(message.to_string());
        print!(
            "\x1b[{};1H\x1b[2K{}",
            terminal_size().1,
            truncate_line(message, terminal_size().0 as usize)
        );
        io::stdout().flush().map_err(|error| error.to_string())
    }

    fn show_error(&mut self, context: &str, error: &str) -> Result<(), String> {
        let message = format!("{context}: {error}");
        let _ = append_runtime_log(&self.repo, &message);
        self.show_message(&message)
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.sessions.len() {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn draw(&self) -> Result<(), String> {
        let mode_label = match self.mode {
            Mode::Normal => "normal",
            Mode::Agent { .. } => "agent",
        };
        view::draw(
            &self.repo,
            &self.config,
            &self.sessions,
            self.selected,
            mode_label,
            self.status_message.as_deref(),
        )
    }
}
