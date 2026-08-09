//! Out-of-band control of a running daemon: start/stop recording, shut down.
//!
//! On Unix this is SIGUSR1/SIGTERM delivered by `pkill -f`, matching whatever a
//! compositor keybinding does. Windows has no user-defined signals, so each
//! daemon serves a named pipe (`\\.\pipe\dictate-<slot>`) and `dictate toggle`
//! connects to it. Both sides expose the same [`Control`] / [`send`] API so the
//! daemon loops and `dictate toggle` are written once.

use tokio::sync::mpsc;

/// How the user toggles a running daemon, phrased for this platform.
#[cfg(unix)]
pub const TOGGLE_HINT: &str = "Send SIGUSR1";
/// How the user toggles a running daemon, phrased for this platform.
#[cfg(windows)]
pub const TOGGLE_HINT: &str = "Press the shortcut or run `dictate toggle live|smart`";

/// What a control message asks the daemon to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlEvent {
    /// Start recording, or stop and transcribe if already recording.
    Toggle,
    /// Wind down and exit.
    Shutdown,
}

/// Which long-running dictate process a message is addressed to.
///
/// Slots keep the live and smart daemons independently addressable so a
/// shortcut only ever toggles its own.
// The daemon loops that construct most of these live behind `cfg(not(test))`.
#[cfg_attr(test, allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// `dictate --daemon` with no explicit mode.
    Main,
    /// `dictate --daemon --mode live`
    Live,
    /// `dictate --daemon --mode smart`
    Smart,
    /// The Windows hotkey agent (`dictate hotkeys`). Constructed only on Windows —
    /// Linux uses systemd user services instead of a resident agent.
    #[cfg_attr(not(windows), allow(dead_code))]
    Hotkeys,
}

impl Slot {
    /// Names the Windows control pipe. The unix backend matches on
    /// `process_pattern` instead, so this is Windows-only in practice.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Main => "main",
            Slot::Live => "live",
            Slot::Smart => "smart",
            Slot::Hotkeys => "hotkeys",
        }
    }

    /// Slot a daemon started with `--mode <mode>` listens on.
    #[cfg_attr(test, allow(dead_code))]
    pub fn from_mode(mode: Option<&str>) -> Slot {
        match mode {
            Some("live") => Slot::Live,
            Some("smart") => Slot::Smart,
            _ => Slot::Main,
        }
    }
}

/// Receives control events for this process.
pub struct Control {
    rx: mpsc::Receiver<ControlEvent>,
}

impl Control {
    /// Next control event, or `None` once the channel can no longer produce any.
    pub async fn recv(&mut self) -> Option<ControlEvent> {
        self.rx.recv().await
    }

    /// Like [`Control::start`], but for one-shot runs that should still work
    /// when a daemon already owns the slot.
    ///
    /// A daemon failing to claim its slot is a real conflict — two of them would
    /// fight over the microphone — but a one-shot recording losing only its
    /// remote-toggle ability is not worth refusing to run over.
    #[cfg_attr(test, allow(dead_code))]
    pub fn start_best_effort(slot: Slot) -> Self {
        match Self::start(slot) {
            Ok(control) => control,
            Err(e) => {
                log::warn!("Remote toggling unavailable ({e}); stop with Ctrl+C");
                Self::ctrl_c_only()
            }
        }
    }

    /// A control channel with no remote listener at all.
    ///
    /// The spawned task owns the sender, so `recv` parks until Ctrl+C rather
    /// than reporting the channel closed — which callers read as "shut down".
    #[cfg_attr(test, allow(dead_code))]
    fn ctrl_c_only() -> Self {
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = tx.send(ControlEvent::Shutdown).await;
            }
        });
        Self { rx }
    }
}

/// Forward control events into a plain `()` channel.
///
/// The streaming loops only understand "something happened". Set
/// `shutdown_ends_run` when that loop exits on any message, so a shutdown can
/// ride the same channel. When it is false the loop keeps running after a
/// message, so a shutdown has to end the process here instead — otherwise
/// intercepting SIGTERM would make the daemon unkillable, which is exactly what
/// `stop_other_daemons` relies on.
#[cfg_attr(test, allow(dead_code))]
pub fn spawn_forwarder(mut control: Control, tx: mpsc::Sender<()>, shutdown_ends_run: bool) {
    tokio::spawn(async move {
        while let Some(event) = control.recv().await {
            if event == ControlEvent::Shutdown && !shutdown_ends_run {
                log::info!("Shutdown requested: stopping daemon");
                std::process::exit(0);
            }
            if tx.send(()).await.is_err() {
                break;
            }
        }
    });
}

/// True when a process is listening on `slot`.
pub fn is_running(slot: Slot) -> bool {
    send(slot, "ping").unwrap_or(false)
}

// ─── Unix: signals ───────────────────────────────────────────────────────────

#[cfg(unix)]
mod imp {
    use super::{Control, ControlEvent, Slot};
    use anyhow::{bail, Result};
    use futures::StreamExt;
    use signal_hook::consts::{SIGTERM, SIGUSR1};
    use signal_hook_tokio::Signals;
    use std::process::{Command, Stdio};
    use tokio::sync::mpsc;

    impl Slot {
        /// `pgrep -f` pattern matching this slot's daemon.
        ///
        /// The bracket keeps pgrep from matching its own command line.
        fn process_pattern(self) -> String {
            match self {
                Slot::Main => "[d]ictate --daemon".to_string(),
                Slot::Live => "[d]ictate --daemon --mode live".to_string(),
                Slot::Smart => "[d]ictate --daemon --mode smart".to_string(),
                Slot::Hotkeys => "[d]ictate hotkeys".to_string(),
            }
        }
    }

    impl Control {
        pub fn start(_slot: Slot) -> Result<Self> {
            let (tx, rx) = mpsc::channel(8);
            let mut signals = Signals::new([SIGUSR1, SIGTERM])?;

            tokio::spawn(async move {
                while let Some(signal) = signals.next().await {
                    let event = match signal {
                        SIGUSR1 => ControlEvent::Toggle,
                        SIGTERM => ControlEvent::Shutdown,
                        _ => continue,
                    };
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            });

            Ok(Self { rx })
        }
    }

    fn quiet(command: &mut Command) -> bool {
        command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Deliver `message` to the daemon on `slot`; `Ok(false)` when none is running.
    pub fn send(slot: Slot, message: &str) -> Result<bool> {
        let pattern = slot.process_pattern();
        let signal = match message {
            "ping" => return Ok(quiet(Command::new("pgrep").args(["-f", &pattern]))),
            "toggle" => "SIGUSR1",
            "shutdown" => "SIGTERM",
            other => bail!("unknown control message '{other}'"),
        };
        Ok(quiet(
            Command::new("pkill").args(["-f", "--signal", signal, &pattern]),
        ))
    }
}

// ─── Windows: named pipes ────────────────────────────────────────────────────

#[cfg(windows)]
mod imp {
    use super::{Control, ControlEvent, Slot};
    use anyhow::{bail, Context, Result};
    use log::{debug, error};
    use std::io::Write;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::windows::named_pipe::ServerOptions;
    use tokio::sync::mpsc;

    fn pipe_name(slot: Slot) -> String {
        format!(r"\\.\pipe\dictate-{}", slot.as_str())
    }

    impl Control {
        pub fn start(slot: Slot) -> Result<Self> {
            let (tx, rx) = mpsc::channel(8);
            serve(slot, tx.clone())?;

            // Ctrl+C is the console equivalent of SIGTERM.
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    let _ = tx.send(ControlEvent::Shutdown).await;
                }
            });

            Ok(Self { rx })
        }
    }

    /// Listen on this slot's pipe and translate messages into events.
    ///
    /// `first_pipe_instance` doubles as the single-instance lock: a second
    /// daemon for the same slot fails here instead of fighting for the mic.
    fn serve(slot: Slot, tx: mpsc::Sender<ControlEvent>) -> Result<()> {
        let name = pipe_name(slot);
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&name)
            .with_context(|| {
                format!(
                    "another dictate '{}' process is already running",
                    slot.as_str()
                )
            })?;

        tokio::spawn(async move {
            loop {
                if let Err(e) = server.connect().await {
                    error!("control pipe {name} failed to accept: {e}");
                    return;
                }
                let mut client = server;

                // A pipe instance serves one client, so the next one has to
                // exist before this client is handled or toggles get dropped.
                server = match ServerOptions::new().create(&name) {
                    Ok(next) => next,
                    Err(e) => {
                        error!("control pipe {name} could not be reopened: {e}");
                        return;
                    }
                };

                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 64];
                    let read = client.read(&mut buf).await.unwrap_or(0);
                    let message = String::from_utf8_lossy(&buf[..read])
                        .trim()
                        .to_ascii_lowercase();
                    let _ = client.write_all(b"ok\n").await;

                    debug!("control message: {message}");
                    let event = match message.as_str() {
                        "toggle" => ControlEvent::Toggle,
                        "shutdown" => ControlEvent::Shutdown,
                        _ => return,
                    };
                    let _ = tx.send(event).await;
                });
            }
        });

        Ok(())
    }

    /// Deliver `message` to the process on `slot`; `Ok(false)` when none is listening.
    pub fn send(slot: Slot, message: &str) -> Result<bool> {
        if !matches!(message, "ping" | "toggle" | "shutdown") {
            bail!("unknown control message '{message}'");
        }
        let name = pipe_name(slot);

        for attempt in 0..5u64 {
            match std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&name)
            {
                Ok(mut pipe) => {
                    pipe.write_all(format!("{message}\n").as_bytes())?;
                    pipe.flush()?;
                    return Ok(true);
                }
                // No server has the pipe open — nothing is running.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                // All instances busy; the daemon is opening the next one.
                Err(_) => std::thread::sleep(Duration::from_millis(40 * (attempt + 1))),
            }
        }
        Ok(false)
    }
}

pub use imp::send;
