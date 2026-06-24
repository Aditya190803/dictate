//! Minimal interactive setup (`dictate setup`).

use crate::config_cli::{ensure_config_file, print_shortcut, set_config_value, ShortcutArgs};
use anyhow::Result;
use inquire::{Confirm, Text};
use std::path::Path;

pub fn run_setup(quick: bool, env_path: &Path) -> Result<()> {
    let path = env_path.to_path_buf();
    ensure_config_file(&path)?;

    println!("\n  dictate setup  (Enter = default)\n");

    let mistral = Confirm::new("Use Mistral for speech-to-text?")
        .with_default(true)
        .with_help_message("No → Groq. Polish needs a Mistral key.")
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

    set_config_value(&path, "profile", "segmented")?;
    set_config_value(&path, "batch-mode", "false")?;
    set_config_value(&path, "transcription-mode", "auto")?;
    set_config_value(&path, "language", "auto")?;
    set_config_value(&path, "shortcut-output", "type")?;
    set_config_value(&path, "shortcut-desktop", "hyprland")?;
    set_config_value(&path, "shortcut-key", "SUPER,R")?;

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
    set_config_value(&path, "enable-overlay", if pill { "true" } else { "false" })?;

    println!("\n✓ Saved {}\n", path.display());
    println!("One shortcut: speak in phrases; polished text inserts after each pause.\n");

    print_shortcut(&ShortcutArgs {
        desktop: crate::config_cli::ShortcutDesktop::Hyprland,
        profile: "segmented".to_string(),
        mode: crate::config_cli::ShortcutMode::Type,
        key: "SUPER,R".to_string(),
    });

    println!("\nRun: dictate doctor");
    if pill {
        println!(
            "Pill: `cargo build --release --features overlay` — daemon auto-starts dictate-overlay when ENABLE_OVERLAY=true."
        );
    }
    Ok(())
}
