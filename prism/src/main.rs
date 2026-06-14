mod actions;
mod agent;
mod args;
mod config;
mod git;
mod github;
mod input;
mod json;
mod plan;
mod process;
mod repo;
mod review;
mod session;
mod terminal;
mod tui;
mod util;
mod view;

use args::{Args, CommandKind};
use config::Config;
use repo::Repository;

fn main() {
    if let Err(error) = run() {
        eprintln!("prism: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(std::env::args_os().skip(1))?;
    let repo = Repository::discover(args.repo.as_deref())?;
    let mut config = Config::load(&repo);

    match args.command {
        CommandKind::Config => {
            config::print_config(&repo, &config);
            Ok(())
        }
        CommandKind::Doctor => config::doctor(&repo, &mut config),
        CommandKind::RunPlan(path) => plan::run_plan_cli(&repo, &config, &path),
        CommandKind::Tui => {
            config::ensure_default_agent(&mut config)?;
            let sessions = session::discover_sessions(&repo, &config)?;
            tui::Tui::new(repo, config, sessions, args.allow_dirty).run()
        }
    }
}
