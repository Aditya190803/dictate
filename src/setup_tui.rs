//! Interactive setup flow (`dictate setup`) for non-technical users.

use crate::config_cli::{
    ensure_config_file, get_default_config_path, print_shortcut, set_config_value, ShortcutArgs,
    ShortcutDesktop, ShortcutMode,
};
use anyhow::Result;
use inquire::{Confirm, Select, Text};

pub fn run_setup(quick: bool) -> Result<()> {
    let path = get_default_config_path();
    ensure_config_file(&path)?;

    println!("\n  dictate setup\n");

    let profile = Select::new(
        "How should dictation work?",
        vec![
            "live_typing — type as you speak (realtime)",
            "smart_paste — speak, stop, polished paste",
            "batch_clip — transcribe once, local cleanup",
        ],
    )
    .prompt()?;

    let profile_key = profile.split(" — ").next().unwrap_or("live_typing").trim();

    let provider = if quick {
        "mistral".to_string()
    } else {
        Select::new("Transcription provider", vec!["mistral", "groq", "local"])
            .prompt()?
            .to_string()
    };

    set_config_value(&path, "provider", &provider)?;
    set_config_value(&path, "profile", profile_key)?;

    match profile_key {
        "smart_paste" | "batch_clip" => set_config_value(&path, "batch-mode", "true")?,
        _ => set_config_value(&path, "batch-mode", "false")?,
    }

    if provider == "mistral" {
        let key = Text::new("Mistral API key (paste, hidden in config file)")
            .with_help_message("Get one at console.mistral.ai")
            .prompt_skippable()?;
        if let Some(k) = key.filter(|s| !s.trim().is_empty()) {
            set_config_value(&path, "mistral-api-key", k.trim())?;
        }
    } else if provider == "groq" && !quick {
        let key = Text::new("Groq API key").prompt_skippable()?;
        if let Some(k) = key.filter(|s| !s.trim().is_empty()) {
            set_config_value(&path, "groq-api-key", k.trim())?;
        }
    }

    let default_out = if profile_key == "smart_paste" {
        "paste"
    } else {
        "type"
    };

    let output_options = vec![
        "type — ydotool types into focused window",
        "paste — clipboard then Ctrl+V (best for smart paste)",
        "clipboard — wl-copy only",
        "stdout — print to terminal",
    ];
    let start = output_options
        .iter()
        .position(|o| o.starts_with(default_out))
        .unwrap_or(0);
    let output = Select::new("Where should text go?", output_options)
        .with_starting_cursor(start)
        .prompt()?;

    let output_key = output.split(' ').next().unwrap_or(default_out);
    set_config_value(&path, "shortcut-output", output_key)?;

    let desktop = Select::new(
        "Desktop / compositor",
        vec!["hyprland", "niri", "gnome", "kde", "sway", "other"],
    )
    .prompt()?;

    let default_key = match desktop {
        "niri" => "Mod,R",
        "gnome" => "<Super>r",
        "kde" => "Meta+R",
        "sway" => "Mod4+R",
        _ => "SUPER,R",
    };

    let shortcut = Text::new("Shortcut key")
        .with_default(default_key)
        .prompt()?;

    set_config_value(&path, "shortcut-desktop", desktop)?;
    set_config_value(&path, "shortcut-key", &shortcut)?;

    if !quick {
        let feedback = Confirm::new("Audio feedback beeps?")
            .with_default(true)
            .prompt()?;
        set_config_value(
            &path,
            "audio-feedback",
            if feedback { "true" } else { "false" },
        )?;
    }

    set_config_value(&path, "language", "auto")?;

    println!("\n✓ Saved {}", path.display());

    let mode = shortcut_mode_from_str(output_key)?;
    let desktop_enum = shortcut_desktop_from_str(desktop)?;

    print_shortcut(&ShortcutArgs {
        desktop: desktop_enum,
        mode,
        profile: profile_key.to_string(),
        key: shortcut,
    });

    println!("\nNext: paste the bind line into your compositor config, then run:");
    println!("  dictate doctor");
    if profile_key == "live_typing" || profile_key == "smart_paste" {
        println!("  (Your shortcut starts `dictate --daemon` in the background.)");
    }

    Ok(())
}

fn shortcut_mode_from_str(s: &str) -> Result<ShortcutMode> {
    match s {
        "type" => Ok(ShortcutMode::Type),
        "clipboard" => Ok(ShortcutMode::Clipboard),
        "paste" => Ok(ShortcutMode::Paste),
        "stdout" => Ok(ShortcutMode::Stdout),
        other => anyhow::bail!("Unknown output mode: {other}"),
    }
}

fn shortcut_desktop_from_str(s: &str) -> Result<ShortcutDesktop> {
    Ok(match s.to_lowercase().as_str() {
        "hyprland" => ShortcutDesktop::Hyprland,
        "niri" => ShortcutDesktop::Niri,
        "gnome" => ShortcutDesktop::Gnome,
        "kde" => ShortcutDesktop::Kde,
        "sway" => ShortcutDesktop::Sway,
        _ => ShortcutDesktop::Other,
    })
}
