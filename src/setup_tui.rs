//! Minimal interactive setup (`dictate setup`).

use crate::config_cli::{
    ensure_config_file, print_dual_shortcuts, set_config_value, ShortcutDesktop,
};
use anyhow::Result;
use inquire::{Confirm, Text};
use std::path::Path;

pub fn run_setup(quick: bool, env_path: &Path) -> Result<()> {
    let path = env_path.to_path_buf();
    ensure_config_file(&path)?;

    println!("\n  dictate setup  (Enter = default)\n");

    let mistral = Confirm::new("Use Mistral for speech-to-text?")
        .with_default(true)
        .with_help_message("No → Groq. Smart polish mode needs Mistral.")
        .prompt()?;

    let provider = if mistral { "mistral" } else { "groq" };
    set_config_value(&path, "provider", provider)?;

    if mistral {
        let key = Text::new("Mistral API key")
            .with_help_message("console.mistral.ai — skip if already in .env")
            .prompt_skippable()?;
        if let Some(k) = key.filter(|s| !s.trim().is_empty()) {
            set_config_value(&path, "mistral-api-key", k.trim())?;
        }
    } else if !quick {
        let key = Text::new("Groq API key").prompt_skippable()?;
        if let Some(k) = key.filter(|s| !s.trim().is_empty()) {
            set_config_value(&path, "groq-api-key", k.trim())?;
        }
    }

    set_config_value(&path, "profile", "live_typing")?;
    set_config_value(&path, "batch-mode", "false")?;
    set_config_value(&path, "transcription-mode", "auto")?;
    set_config_value(&path, "language", "auto")?;
    set_config_value(&path, "shortcut-output", "type")?;
    set_config_value(&path, "shortcut-desktop", "hyprland")?;
    set_config_value(&path, "shortcut-key-live", "SUPER,R")?;
    set_config_value(&path, "shortcut-key-smart", "SUPER,SHIFT,R")?;
    set_config_value(&path, "context-editing", "true")?;

    let beeps = Confirm::new("Audio feedback beeps?")
        .with_default(true)
        .prompt()?;
    set_config_value(
        &path,
        "audio-feedback",
        if beeps { "true" } else { "false" },
    )?;

    let pill = Confirm::new("Show recording pill while dictating? (experimental)")
        .with_default(false)
        .prompt()?;
    set_config_value(
        &path,
        "enable-overlay",
        if pill { "true" } else { "false" },
    )?;

    println!("\n✓ Saved {}\n", path.display());
    println!("Two shortcuts:");
    println!("  Live  — realtime typing, no context edits");
    println!("  Smart — speak, stop, polished paste (daemon)\n");

    print_dual_shortcuts(&ShortcutDesktop::Hyprland, "SUPER,R", "SUPER,SHIFT,R");

    println!("\nRun: dictate doctor");
    if pill {
        println!(
            "Pill: `cargo build --release --features overlay` — daemon auto-starts dictate-overlay when ENABLE_OVERLAY=true."
        );
    }
    Ok(())
}