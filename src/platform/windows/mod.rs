//! Windows output sinks: typing, clipboard, and paste handled in-process.

pub mod autostart;
pub mod clipboard;
pub mod hotkeys;
pub mod input;

use super::INTERNAL_CMD;
use anyhow::{anyhow, Context, Result};
use std::time::Duration;

/// Give the focused window a moment to observe the new clipboard contents
/// before Ctrl+V arrives. Without it fast apps paste the previous value.
const PASTE_SETTLE: Duration = Duration::from_millis(40);

fn internal(sink: &str) -> Vec<String> {
    vec![INTERNAL_CMD.to_string(), sink.to_string()]
}

/// Argv for a `SHORTCUT_OUTPUT` / `--pipe-to` mode, or `None` for stdout.
pub fn pipe_to_for_mode(mode: &str) -> Option<Vec<String>> {
    match mode {
        "type" | "typing" => Some(internal("type")),
        "clipboard" | "copy" => Some(internal("clipboard")),
        "paste" | "clipboard_paste" => Some(internal("paste")),
        _ => None,
    }
}

/// Argv that yields the current clipboard.
pub fn clipboard_read_command() -> Vec<String> {
    internal("clipboard-read")
}

/// Hint shown when the clipboard cannot be read.
#[cfg_attr(test, allow(dead_code))]
pub const CLIPBOARD_HINT: &str =
    "Close any clipboard manager holding the clipboard, or set [command_mode].clipboard_command in text.toml.";

/// Editor used by `dictate config edit` / `dictate scratchpad edit`.
pub fn default_editor() -> String {
    std::env::var("EDITOR").unwrap_or_else(|_| "notepad".to_string())
}

/// How to erase `count` characters through `sink`.
///
/// The typing sinks send real Backspace keys; anything else falls back to
/// writing `\u{8}` into the sink's stdin (handled by the caller).
pub fn backspace_invocation(sink: &[String], count: usize) -> Option<(Vec<String>, String)> {
    if !super::is_internal(sink) {
        return None;
    }
    match sink.get(1).map(String::as_str) {
        Some("type") | Some("paste") => Some((internal("backspace"), count.to_string())),
        _ => None,
    }
}

/// Service an in-process sink. `args` is argv[1..] of an [`INTERNAL_CMD`] pipe.
pub async fn run_internal_sink(args: &[String], input: &str) -> Result<i32> {
    let sink = args
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("internal sink is missing a name"))?;
    let text = input.to_string();

    tokio::task::spawn_blocking(move || -> Result<i32> {
        match sink.as_str() {
            "type" => input::type_text(&text)?,
            "clipboard" => clipboard::set_text(&text)?,
            "paste" => {
                clipboard::set_text(&text)?;
                std::thread::sleep(PASTE_SETTLE);
                input::paste()?;
            }
            "backspace" => {
                let count: usize = text
                    .trim()
                    .parse()
                    .context("backspace sink expects a count on stdin")?;
                input::backspace(count)?;
            }
            other => return Err(anyhow!("unknown internal sink '{other}'")),
        }
        Ok(0)
    })
    .await?
}

/// Service an in-process source. `args` is argv[1..] of an [`INTERNAL_CMD`] pipe.
pub async fn capture_internal(args: &[String]) -> Result<String> {
    let source = args
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("internal source is missing a name"))?;

    tokio::task::spawn_blocking(move || match source.as_str() {
        "clipboard-read" => clipboard::get_text(),
        other => Err(anyhow!("unknown internal source '{other}'")),
    })
    .await?
}
