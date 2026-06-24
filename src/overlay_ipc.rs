//! Unix datagram IPC for the optional recording pill (`dictate-overlay`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::net::UnixDatagram;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayState {
    Idle,
    Listening,
    Processing,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OverlayMessage {
    State { s: OverlayState },
    Level { v: f32 },
    /// In-progress STT caption (not inserted into the focused app).
    Preview { t: String },
}

/// Default socket under XDG runtime (overlay binds here; dictate sends).
pub fn default_socket_path() -> PathBuf {
    let base = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("UID")
                .ok()
                .map(|uid| PathBuf::from(format!("/run/user/{uid}")))
        })
        .unwrap_or_else(std::env::temp_dir);
    base.join("dictate-overlay.sock")
}

#[derive(Clone)]
pub struct OverlayPublisher {
    #[cfg(unix)]
    socket: Option<std::sync::Arc<UnixDatagram>>,
    #[cfg(not(unix))]
    socket: Option<()>,
    path: PathBuf,
}

impl OverlayPublisher {
    pub fn from_env_enabled(enabled: bool) -> Self {
        if !enabled {
            return Self {
                socket: None,
                path: default_socket_path(),
            };
        }
        let path = std::env::var("DICTATE_OVERLAY_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_socket_path());
        #[cfg(unix)]
        let socket = UnixDatagram::unbound().ok().map(std::sync::Arc::new);
        #[cfg(not(unix))]
        let socket = None;
        Self { socket, path }
    }

    pub fn set_state(&self, state: OverlayState) {
        self.send(&OverlayMessage::State { s: state });
    }

    pub fn set_level(&self, level: f32) {
        let v = level.clamp(0.0, 1.0);
        self.send(&OverlayMessage::Level { v });
    }

    pub fn set_preview(&self, text: &str) {
        let t = text.chars().take(120).collect::<String>();
        self.send(&OverlayMessage::Preview { t });
    }

    pub fn clear_preview(&self) {
        self.send(&OverlayMessage::Preview { t: String::new() });
    }

    fn send(&self, msg: &OverlayMessage) {
        #[cfg(not(unix))]
        {
            let _ = msg;
            return;
        }
        #[cfg(unix)]
        let Some(sock) = &self.socket
        else {
            return;
        };
        let Ok(line) = serde_json::to_string(msg) else {
            return;
        };
        let _ = sock.send_to(line.as_bytes(), &self.path);
    }
}

/// Start `dictate-overlay` if built (`cargo build --features overlay`).
pub fn try_spawn_overlay_process() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let overlay = dir.join("dictate-overlay");
    if !overlay.is_file() {
        log::warn!(
            "ENABLE_OVERLAY=true but {} missing (build with --features overlay)",
            overlay.display()
        );
        return;
    }
    let _ = std::process::Command::new(&overlay)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// RMS → 0..1 for waveform bars (tuned for speech).
pub fn level_from_samples(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum / samples.len() as f32).sqrt();
    ((rms - 0.005) / 0.12).clamp(0.0, 1.0)
}
