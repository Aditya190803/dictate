//! Self update check: compare this binary against the latest GitHub Release.
//!
//! Design: never slow down recording. Only `dictate update` and
//! `dictate doctor` touch the network (short timeout, 24h cache). Every other
//! invocation does a cheap cache-file read and nudges on stderr if a newer
//! release is known — stdout stays clean for transcript output.
//! Opt out with `DICTATE_NO_UPDATE_CHECK=1`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const REPO: &str = "Aditya190803/dictate";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const CACHE_TTL_SECS: u64 = 24 * 3600;
const FETCH_TIMEOUT_MS: u64 = 5000;

#[derive(Serialize, Deserialize)]
struct UpdateCache {
    latest: String,
    checked_at: u64,
}

#[derive(Debug, PartialEq)]
enum CheckOutcome {
    Newer(String),
    Current,
    Unknown,
}

pub fn updates_disabled() -> bool {
    std::env::var("DICTATE_NO_UPDATE_CHECK")
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// `v1.2.3` → `1.2.3`.
fn normalize(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

fn split_pre(s: &str) -> (&str, Option<&str>) {
    match s.split_once('-') {
        Some((num, pre)) => (num, Some(pre)),
        None => (s, None),
    }
}

fn leading_num(part: &str) -> u64 {
    part.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// True when `latest` is a newer release than `current`.
/// A bare release beats the same-number prerelease (`1.1.0` > `1.1.0-dev`).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let latest = normalize(latest);
    let current = normalize(current);
    if latest.is_empty() || current.is_empty() {
        return false;
    }
    let (ln, lp) = split_pre(&latest);
    let (cn, cp) = split_pre(&current);
    let lparts: Vec<u64> = ln.split('.').map(leading_num).collect();
    let cparts: Vec<u64> = cn.split('.').map(leading_num).collect();
    for i in 0..lparts.len().max(cparts.len()) {
        let l = lparts.get(i).copied().unwrap_or(0);
        let c = cparts.get(i).copied().unwrap_or(0);
        if l != c {
            return l > c;
        }
    }
    matches!((lp, cp), (None, Some(_)))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn ttl_secs() -> u64 {
    std::env::var("DICTATE_UPDATE_CHECK_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(CACHE_TTL_SECS)
}

fn cache_path() -> PathBuf {
    if let Ok(p) = std::env::var("DICTATE_UPDATE_CACHE_PATH") {
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
        .join("dictate/update-check.json")
}

fn read_cache() -> Option<UpdateCache> {
    let path = cache_path();
    let raw = std::fs::read_to_string(path).ok()?;
    let cache: UpdateCache = serde_json::from_str(&raw).ok()?;
    if normalize(&cache.latest).is_empty() {
        return None;
    }
    Some(cache)
}

fn write_cache(latest: &str) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            crate::config_cli::restrict_perms(parent);
        }
    }
    let cache = UpdateCache {
        latest: latest.to_string(),
        checked_at: now_secs(),
    };
    if let Ok(raw) = serde_json::to_string(&cache) {
        if std::fs::write(&path, raw).is_ok() {
            crate::config_cli::restrict_perms(&path);
        }
    }
}

fn newer_than_current(latest: &str) -> Option<String> {
    if is_newer(latest, CURRENT_VERSION) {
        Some(normalize(latest))
    } else {
        None
    }
}

async fn fetch_latest_tag() -> Option<String> {
    let url = std::env::var("DICTATE_UPDATE_CHECK_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("https://api.github.com/repos/{REPO}/releases/latest"));
    let timeout_ms: u64 = std::env::var("DICTATE_UPDATE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(FETCH_TIMEOUT_MS);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .ok()?;
    let resp = client
        .get(&url)
        .header("User-Agent", format!("dictate/{CURRENT_VERSION}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    let tag = normalize(tag);
    if tag.is_empty() {
        None
    } else {
        Some(tag)
    }
}

async fn check(force: bool) -> CheckOutcome {
    if updates_disabled() {
        return CheckOutcome::Unknown;
    }
    if !force {
        if let Some(cached) = read_cache() {
            if now_secs().saturating_sub(cached.checked_at) < ttl_secs() {
                return match newer_than_current(&cached.latest) {
                    Some(v) => CheckOutcome::Newer(v),
                    None => CheckOutcome::Current,
                };
            }
        }
    }
    match fetch_latest_tag().await {
        Some(latest) => {
            write_cache(&latest);
            match newer_than_current(&latest) {
                Some(v) => CheckOutcome::Newer(v),
                None => CheckOutcome::Current,
            }
        }
        // Offline: a stale cache is still worth reporting.
        None => match read_cache().and_then(|c| newer_than_current(&c.latest)) {
            Some(v) => CheckOutcome::Newer(v),
            None => CheckOutcome::Unknown,
        },
    }
}

/// Cheap cached-only lookup (no network) for the startup nudge.
pub fn cached_update_available() -> Option<String> {
    if updates_disabled() {
        return None;
    }
    read_cache().and_then(|c| newer_than_current(&c.latest))
}

pub fn reinstall_command() -> &'static str {
    #[cfg(windows)]
    return "irm https://dictate.adityamer.dev/install.ps1 | iex";
    #[cfg(not(windows))]
    return "curl -fsSL https://dictate.adityamer.dev/install.sh | sh";
}

pub fn update_message(latest: &str) -> String {
    format!(
        "dictate: update available — v{} (you have v{}). Update with: {}",
        latest,
        CURRENT_VERSION,
        reinstall_command()
    )
}

/// Startup nudge: stderr only, so transcript stdout is never polluted.
pub fn print_cached_notice() {
    if let Some(latest) = cached_update_available() {
        eprintln!("{}", update_message(&latest));
    }
}

/// Fresh (network, cached 24h) status line for `dictate doctor`.
pub async fn print_fresh_status() {
    match check(false).await {
        CheckOutcome::Newer(latest) => eprintln!("{}", update_message(&latest)),
        CheckOutcome::Current => println!("✓ dictate v{CURRENT_VERSION} is up to date"),
        CheckOutcome::Unknown => {
            if updates_disabled() {
                return;
            }
            println!("  Update check: skipped (offline or no releases yet)");
        }
    }
}

/// `dictate update`: force a fresh check and tell the user what to do.
pub async fn run_update_command() -> Result<()> {
    match check(true).await {
        CheckOutcome::Newer(latest) => {
            println!("{}", update_message(&latest));
        }
        CheckOutcome::Current => {
            println!("dictate v{CURRENT_VERSION} is already the latest release.");
        }
        CheckOutcome::Unknown => {
            println!("Could not reach GitHub releases — check your connection and try again.");
            println!("Releases: https://github.com/{REPO}/releases");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn detects_newer_releases() {
        assert!(is_newer("1.2.4", "1.2.3"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.10.0", "1.9.0"));
        assert!(is_newer("v1.2.4", "1.2.3"));
        assert!(is_newer("1.2.4", "v1.2.3"));
        assert!(is_newer("1.2.1", "1.2"));
    }

    #[test]
    fn ignores_same_or_older() {
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.3", "1.2.4"));
        assert!(!is_newer("1.9.0", "1.10.0"));
        assert!(!is_newer("1.2", "1.2.0"));
        assert!(!is_newer("", "1.2.3"));
        assert!(!is_newer("1.2.3", ""));
    }

    #[test]
    fn release_beats_same_number_prerelease() {
        assert!(is_newer("1.1.0", "1.1.0-dev"));
        assert!(!is_newer("1.1.0-dev", "1.1.0"));
        assert!(is_newer("1.1.0-dev", "1.0.9"));
    }

    #[test]
    fn cache_roundtrip_reports_newer() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        std::env::set_var("DICTATE_UPDATE_CACHE_PATH", path.to_str().unwrap());
        std::env::remove_var("DICTATE_NO_UPDATE_CHECK");

        // Cache a version far in the future: always newer than the test build.
        write_cache("99.0.0");
        assert_eq!(cached_update_available(), Some("99.0.0".to_string()));

        // Cache current version: nothing to report.
        write_cache(CURRENT_VERSION);
        assert_eq!(cached_update_available(), None);

        std::env::remove_var("DICTATE_UPDATE_CACHE_PATH");
    }

    #[test]
    fn opt_out_disables_everything() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        std::env::set_var("DICTATE_UPDATE_CACHE_PATH", path.to_str().unwrap());
        std::env::set_var("DICTATE_NO_UPDATE_CHECK", "1");

        write_cache("99.0.0");
        assert_eq!(cached_update_available(), None);
        assert!(updates_disabled());

        std::env::remove_var("DICTATE_NO_UPDATE_CHECK");
        std::env::remove_var("DICTATE_UPDATE_CACHE_PATH");
    }

    #[tokio::test]
    async fn offline_check_fails_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("update-check.json");
        {
            let _g = env_lock();
            std::env::set_var("DICTATE_UPDATE_CACHE_PATH", path.to_str().unwrap());
            std::env::remove_var("DICTATE_NO_UPDATE_CHECK");
            // Discard port: connection refused instantly, no waiting on timeouts.
            std::env::set_var("DICTATE_UPDATE_CHECK_URL", "http://127.0.0.1:9/");
            std::env::set_var("DICTATE_UPDATE_TIMEOUT_MS", "2000");
        }

        // No cache + unreachable → Unknown, and nothing cached as fact.
        assert_eq!(check(true).await, CheckOutcome::Unknown);
        assert!(!path.exists());

        {
            let _g = env_lock();
            std::env::remove_var("DICTATE_UPDATE_CHECK_URL");
            std::env::remove_var("DICTATE_UPDATE_TIMEOUT_MS");
            std::env::remove_var("DICTATE_UPDATE_CACHE_PATH");
        }
    }
}
