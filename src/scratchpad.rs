//! Local markdown scratchpad (no GUI).

use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn scratchpad_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        })
        .join("dictate/scratchpad.md")
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(p) = path.parent() {
        crate::config_cli::ensure_private_dir(p)?;
    }
    Ok(())
}

pub fn append_text(chunk: &str) -> Result<PathBuf> {
    let path = scratchpad_path();
    ensure_parent(&path)?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    if !chunk.is_empty() {
        if !chunk.ends_with('\n') {
            writeln!(f, "{chunk}")?;
        } else {
            write!(f, "{chunk}")?;
        }
    }
    crate::config_cli::restrict_perms(&path);
    Ok(path)
}

pub fn read_all() -> Result<String> {
    let path = scratchpad_path();
    if !path.exists() {
        return Ok(String::new());
    }
    let mut s = String::new();
    File::open(&path)?.read_to_string(&mut s)?;
    Ok(s)
}

pub fn clear() -> Result<()> {
    let path = scratchpad_path();
    if path.exists() {
        std::fs::remove_file(&path).context("remove scratchpad")?;
    }
    Ok(())
}

pub fn open_in_editor() -> Result<()> {
    let path = scratchpad_path();
    ensure_parent(&path)?;
    if !path.exists() {
        File::create(&path)?;
        crate::config_cli::restrict_perms(&path);
    }
    let editor = crate::platform::default_editor();
    #[cfg(windows)]
    let (program, mut args) = crate::config_cli::editor_argv(&editor, "notepad");
    #[cfg(not(windows))]
    let (program, mut args) = crate::config_cli::editor_argv(&editor, "vi");
    args.push(path.to_string_lossy().into_owned());
    let status = Command::new(&program).args(&args).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("editor exited with {s}"),
        Err(e) => anyhow::bail!("failed to run $EDITOR ({editor}): {e}"),
    }
}
