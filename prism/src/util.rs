use std::env;
use std::path::PathBuf;

pub fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "~".to_string();
    }
    let mut out = text.chars().take(max_chars - 1).collect::<String>();
    out.push('~');
    out
}

pub fn truncate_line(text: &str, max_chars: usize) -> String {
    truncate(&single_line(text), max_chars)
}

pub fn single_line(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_ascii_control() { ' ' } else { ch })
        .collect()
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub fn safe_branch_filename(branch: &str) -> String {
    branch
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_ascii_control() => '_',
            ch => ch,
        })
        .collect()
}

pub fn timestamp_label() -> String {
    match crate::process::run_capture(std::process::Command::new("date").arg("+%H:%M:%S")) {
        Ok(value) => value.trim().to_string(),
        Err(_) => "now".to_string(),
    }
}

pub fn timestamp_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

pub fn yes(value: &str) -> bool {
    matches!(value.trim(), "y" | "Y" | "yes" | "YES")
}

pub fn empty_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value.trim()
    }
}

pub fn indent_markdown_block(value: &str) -> String {
    value
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{single_line, truncate_line};

    #[test]
    fn single_line_replaces_control_characters() {
        assert_eq!(
            single_line("one\ntwo\r\tthree\x1b[31m"),
            "one two  three [31m"
        );
    }

    #[test]
    fn truncate_line_sanitizes_before_truncating() {
        assert_eq!(truncate_line("abc\ndef", 6), "abc d~");
    }
}
