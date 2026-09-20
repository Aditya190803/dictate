//! Shared pure modules, compiled into both the `dictate` library and the
//! binary (the binary re-uses them via `use dictate::...` so there is a single
//! implementation, not a copy).
//!
//! Only dependency-free modules live here: `transcript`, `intent`,
//! `developer_modes`, and `polish_styles` use nothing but `std`/`regex`.
//!
//! NOTE: `text_processing` and `editing` stay binary-only (`mod` in
//! `src/main.rs`). `text_processing` needs `crate::platform` (OS-specific
//! clipboard sinks) and `editing` needs `crate::typing` (platform backspace
//! invocations + `command` subprocesses). Moving either would drag the whole
//! platform/command/config graph into the library and turn it into a second
//! binary. If they ever become dependency-free, add them here and switch
//! `main.rs` to `use dictate::...`.

pub mod developer_modes;
pub mod intent;
pub mod polish_styles;
pub mod transcript;
pub mod word_store;

#[cfg(feature = "words-ui")]
pub mod words_ui;
