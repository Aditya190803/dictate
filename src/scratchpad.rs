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
        std::fs::create_dir_all(p)?;
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
    }
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("nano");
    let status = Command::new(program).args(parts).arg(&path).status();
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("editor exited with {s}"),
        Err(e) => anyhow::bail!("failed to run $EDITOR ({editor}): {e}"),
    }
}
