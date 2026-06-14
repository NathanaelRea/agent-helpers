use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub repo: Option<PathBuf>,
    pub allow_dirty: bool,
    pub command: CommandKind,
}

#[derive(Debug)]
pub enum CommandKind {
    Tui,
    Doctor,
    Config,
    RunPlan(PathBuf),
}

impl Args {
    pub fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut repo = None;
        let mut allow_dirty = false;
        let mut command = CommandKind::Tui;
        let mut iter = args.into_iter();

        while let Some(arg) = iter.next() {
            let text = arg.to_string_lossy();
            match text.as_ref() {
                "--repo" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--repo requires a path".to_string())?;
                    repo = Some(PathBuf::from(value));
                }
                "--allow-dirty" => allow_dirty = true,
                "doctor" => command = CommandKind::Doctor,
                "config" => command = CommandKind::Config,
                "run-plan" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "run-plan requires a plan file".to_string())?;
                    command = CommandKind::RunPlan(PathBuf::from(value));
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(Self {
            repo,
            allow_dirty,
            command,
        })
    }
}

fn print_help() {
    println!(
        "Usage:\n  prism [--repo <path>] [--allow-dirty]\n  prism [--repo <path>] doctor\n  prism [--repo <path>] config\n  prism [--repo <path>] run-plan <plan.md>"
    );
}
