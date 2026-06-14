use std::env;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn run_capture(command: &mut Command) -> Result<String, String> {
    let output = command
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("{command:?}: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("exited with {}", output.status)
        } else {
            stderr
        };
        return Err(format!("{command:?}: {message}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn run_status(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?}: exited with {status}"))
    }
}

pub fn command_exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).is_file();
    }
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|dir| dir.join(command).is_file())
}

pub fn command_version(command: &str) -> Option<String> {
    let argv = split_command_words(command);
    let program = argv.first()?;
    if !command_exists(program) {
        return None;
    }
    let output = Command::new(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
}

pub fn split_command_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

pub fn run_configured_commands(commands: &[String], cwd: &Path, label: &str) -> Result<(), String> {
    for command in commands {
        let argv = split_command_words(command);
        let Some(program) = argv.first() else {
            continue;
        };
        run_status(Command::new(program).args(&argv[1..]).current_dir(cwd))
            .map_err(|error| format!("{label} check `{command}` failed: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_command_words_handles_quotes() {
        let words = split_command_words(r#"my-agent --mode "two words" 'three words'"#);
        assert_eq!(
            words,
            vec!["my-agent", "--mode", "two words", "three words"]
        );
    }
}
