//! Append-only local transcript history (no cloud).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub ts: u64,
    pub text: String,
    #[serde(default)]
    pub profile: String,
}

fn history_path() -> PathBuf {
    if let Ok(p) = std::env::var("DICTATE_HISTORY_PATH") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local/share")
        })
        .join("dictate/history.jsonl")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Record finalized text if non-empty. Best-effort; errors are logged by caller.
pub fn append_transcript(text: &str, profile: &str, enabled: bool) -> Result<()> {
    let text = text.trim();
    if text.is_empty() || !enabled {
        return Ok(());
    }

    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let entry = HistoryEntry {
        ts: now_secs(),
        text: text.to_string(),
        profile: profile.to_string(),
    };
    let line = serde_json::to_string(&entry)?;
    {
        let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(f, "{line}")?;
    }
    trim_file(&path)?;
    Ok(())
}

fn trim_file(path: &Path) -> Result<()> {
    let f = File::open(path)?;
    let lines: Vec<String> = BufReader::new(f).lines().collect::<std::io::Result<_>>()?;
    if lines.len() <= MAX_ENTRIES {
        return Ok(());
    }
    let keep = &lines[lines.len() - MAX_ENTRIES..];
    let mut out = File::create(path)?;
    for line in keep {
        writeln!(out, "{line}")?;
    }
    Ok(())
}

pub fn list_entries(limit: usize) -> Result<Vec<HistoryEntry>> {
    let path = history_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let f = File::open(&path)?;
    let mut entries: Vec<HistoryEntry> = BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str(&l).ok())
        .collect();
    if limit > 0 && entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    Ok(entries)
}

pub fn clear_history() -> Result<()> {
    let path = history_path();
    if path.exists() {
        std::fs::remove_file(&path).context("remove history file")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn with_temp_history<F: FnOnce(&Path)>(f: F) {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _g = LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        std::env::set_var("DICTATE_HISTORY_PATH", path.to_str().unwrap());
        f(dir.path());
        std::env::remove_var("DICTATE_HISTORY_PATH");
    }

    #[test]
    fn append_and_list_roundtrip() {
        with_temp_history(|_| {
            append_transcript("hello world", "segmented", true).unwrap();
            let entries = list_entries(10).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].text, "hello world");
            assert_eq!(entries[0].profile, "segmented");
            clear_history().unwrap();
            assert!(list_entries(10).unwrap().is_empty());
        });
    }
}