use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process::run_capture;

#[derive(Clone, Debug)]
pub struct Repository {
    pub root: PathBuf,
}

impl Repository {
    pub fn discover(repo_arg: Option<&Path>) -> Result<Self, String> {
        let start = match repo_arg {
            Some(path) => path.to_path_buf(),
            None => {
                std::env::current_dir().map_err(|error| format!("current directory: {error}"))?
            }
        };
        let output = run_capture(
            Command::new("git")
                .arg("-C")
                .arg(&start)
                .args(["rev-parse", "--show-toplevel"]),
        )?;
        let root = PathBuf::from(output.trim());
        if root.as_os_str().is_empty() {
            return Err("not inside a Git repository".to_string());
        }
        Ok(Self { root })
    }

    pub fn prism_dir(&self) -> PathBuf {
        self.root.join(".prism")
    }
}
