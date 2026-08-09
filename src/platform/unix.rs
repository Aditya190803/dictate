//! Wayland/Linux output sinks: everything is an external helper binary.

use crate::config::YDOTOOL_PASTE_SHELL;
use anyhow::{anyhow, Result};

/// Argv for a `SHORTCUT_OUTPUT` / `--pipe-to` mode, or `None` for stdout.
pub fn pipe_to_for_mode(mode: &str) -> Option<Vec<String>> {
    match mode {
        "type" | "typing" => Some(vec![
            "ydotool".to_string(),
            "type".to_string(),
            "--file".to_string(),
            "-".to_string(),
        ]),
        "clipboard" | "copy" => Some(vec!["wl-copy".to_string()]),
        "paste" | "clipboard_paste" => Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            YDOTOOL_PASTE_SHELL.to_string(),
        ]),
        _ => None,
    }
}

/// Argv that prints the current clipboard on stdout.
pub fn clipboard_read_command() -> Vec<String> {
    vec!["wl-paste".to_string(), "--no-newline".to_string()]
}

/// Hint shown when the clipboard cannot be read.
#[cfg_attr(test, allow(dead_code))]
pub const CLIPBOARD_HINT: &str =
    "Install wl-clipboard or configure [command_mode].clipboard_command in text.toml.";

/// Editor used by `dictate config edit` / `dictate scratchpad edit`.
pub fn default_editor() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string())
}

/// How to erase `count` characters through `sink`, when the sink has a
/// dedicated key-press path. `None` falls back to writing `\u{8}` to stdin.
pub fn backspace_invocation(sink: &[String], count: usize) -> Option<(Vec<String>, String)> {
    if !sink.first().is_some_and(|name| name.ends_with("ydotool")) {
        return None;
    }
    let mut argv = vec!["ydotool".to_string(), "key".to_string()];
    for _ in 0..count {
        argv.push("14:1".to_string());
        argv.push("14:0".to_string());
    }
    Some((argv, String::new()))
}

/// No in-process sinks on Unix — `pipe_to_for_mode` always yields a real binary.
pub async fn run_internal_sink(args: &[String], _input: &str) -> Result<i32> {
    Err(anyhow!(
        "internal sink '{}' is Windows-only",
        args.join(" ")
    ))
}

/// No in-process sources on Unix.
pub async fn capture_internal(args: &[String]) -> Result<String> {
    Err(anyhow!(
        "internal source '{}' is Windows-only",
        args.join(" ")
    ))
}
