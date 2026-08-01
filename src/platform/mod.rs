//! Platform glue for the pieces of dictate that are not portable.
//!
//! On Wayland the output sinks are external helpers (`ydotool`, `wl-copy`), so
//! "type this text" is just a command with the text on stdin. Windows has no
//! such helpers, so the same sinks are implemented in-process (SendInput and
//! the Win32 clipboard) and addressed through a pseudo-command whose argv[0] is
//! [`INTERNAL_CMD`]. `command::execute_with_input` / `execute_capture` route
//! those to [`run_internal_sink`] / [`capture_internal`] instead of spawning a
//! process, so every caller keeps passing a plain `Vec<String>` either way.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// argv[0] marking a pipe target that dictate services itself.
pub const INTERNAL_CMD: &str = "@dictate";

/// True when `argv` addresses an in-process sink rather than an external binary.
pub fn is_internal(argv: &[String]) -> bool {
    argv.first().is_some_and(|a| a == INTERNAL_CMD)
}
