//! Minimal interactive setup (`dictate setup`).

use crate::config::Config;
use crate::config_cli::{
    default_shortcut_keys, ensure_config_file, install_gnome_shortcuts, print_shortcut,
    read_config_value, run_autostart_command, run_doctor, set_config_value, AutostartCommand,
    ShortcutArgs, ShortcutDesktop, ShortcutMode,
};
use anyhow::Result;
use inquire::{Confirm, Select, Text};
#[cfg(unix)]
use std::env;
use std::path::Path;

#[cfg(windows)]
fn detect_desktop() -> &'static str {
    "windows"
}

#[cfg(unix)]
fn detect_desktop() -> &'static str {
    let session = env::var("XDG_SESSION_TYPE").unwrap_or_default();
    if session != "wayland" && !session.is_empty() {
        return "other";
    }
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    if desktop.contains("gnome") || desktop.contains("unity") {
        "gnome"
    } else if desktop.contains("kde") || desktop.contains("plasma") {
        "kde"
    } else if desktop.contains("sway") {
        "sway"
    } else if desktop.contains("hyprland") {
        "hyprland"
    } else if desktop.contains("niri") {
        "niri"
    } else {
        "hyprland"
    }
}

fn parse_shortcut_desktop(s: &str) -> ShortcutDesktop {
    match s.trim().to_lowercase().as_str() {
        "gnome" => ShortcutDesktop::Gnome,
        "kde" => ShortcutDesktop::Kde,
        "sway" => ShortcutDesktop::Sway,
        "niri" => ShortcutDesktop::Niri,
        "windows" => ShortcutDesktop::Windows,
        "other" => ShortcutDesktop::Other,
        _ => ShortcutDesktop::Hyprland,
    }
}

/// Desktops offered by the setup wizard, most likely first.
fn desktop_choices() -> Vec<String> {
    #[cfg(windows)]
    return vec!["windows".into(), "other".into()];
    #[cfg(unix)]
    return vec![
        "hyprland".into(),
        "niri".into(),
        "gnome".into(),
        "kde".into(),
        "sway".into(),
        "other".into(),
    ];
}

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

    // First shortcut = live realtime typing, second = smart paste (INSTALL.md
    // default). The actual combos come from default_shortcut_keys() per platform.
    set_config_value(&path, "profile", "segmented")?;
    set_config_value(&path, "batch-mode", "false")?;
    set_config_value(&path, "transcription-mode", "auto")?;
    set_config_value(&path, "language", "auto")?;
    set_config_value(&path, "shortcut-output", "type")?;

    let (default_live, default_smart) = default_shortcut_keys();
    let live_key = if quick {
        default_live.to_string()
    } else {
        Text::new("Live typing shortcut (words as you speak)")
            .with_default(default_live)
            .with_help_message("Example: CTRL,ALT,R or SUPER,SHIFT,R")
            .prompt()?
    };
    let smart_key = if quick {
        default_smart.to_string()
    } else {
        Text::new("Smart paste shortcut (polish when you stop)")
            .with_default(default_smart)
            .prompt()?
    };
    set_config_value(&path, "shortcut-key-live", live_key.trim())?;
    set_config_value(&path, "shortcut-key-smart", smart_key.trim())?;

    let detected = detect_desktop();
    let desktop = if quick {
        detected.to_string()
    } else {
        let choices = desktop_choices();
        let start = choices.iter().position(|c| c == detected).unwrap_or(0);
        Select::new("Desktop (for shortcut instructions)", choices)
            .with_starting_cursor(start)
            .prompt()?
    };
    set_config_value(&path, "shortcut-desktop", &desktop)?;
    set_config_value(&path, "shortcut-key", live_key.trim())?;

    let beeps = Confirm::new("Audio feedback beeps?")
        .with_default(true)
        .prompt()?;
    set_config_value(
        &path,
        "audio-feedback",
        if beeps { "true" } else { "false" },
    )?;

    let desktop_enum = parse_shortcut_desktop(&desktop);
    let live_key_saved =
        read_config_value(&path, "SHORTCUT_KEY_LIVE")?.unwrap_or_else(|| default_live.to_string());
    let smart_key_saved = read_config_value(&path, "SHORTCUT_KEY_SMART")?
        .unwrap_or_else(|| default_smart.to_string());

    println!("\n✓ Saved {}\n", path.display());

    let mut shortcuts_done = false;
    if matches!(desktop_enum, ShortcutDesktop::Gnome) {
        let install = Confirm::new("Install GNOME keyboard shortcuts now?")
            .with_default(true)
            .with_help_message(
                "Same as `dictate shortcuts gnome --install` (dictate toggle live / smart)",
            )
            .prompt()?;
        if install {
            match install_gnome_shortcuts(env_path) {
                Ok(()) => shortcuts_done = true,
                Err(e) => eprintln!("⚠ Could not install GNOME shortcuts: {e}"),
            }
        }
    }

    if !shortcuts_done {
        println!("Add these shortcuts in your desktop (copy each block):\n");
        println!("── Live typing — {live_key_saved} ──\n");
        print_shortcut(
            &ShortcutArgs {
                desktop: desktop_enum,
                profile: "live_typing".to_string(),
                mode: ShortcutMode::Type,
                key: live_key_saved.clone(),
                install: false,
            },
            env_path,
        );
        println!("\n── Smart paste — {smart_key_saved} ──\n");
        print_shortcut(
            &ShortcutArgs {
                desktop: desktop_enum,
                profile: "smart_paste".to_string(),
                mode: ShortcutMode::Paste,
                key: smart_key_saved.clone(),
                install: false,
            },
            env_path,
        );
        if matches!(desktop_enum, ShortcutDesktop::Gnome) {
            println!("\nOr run: dictate shortcuts gnome --install");
        } else if matches!(desktop_enum, ShortcutDesktop::Kde | ShortcutDesktop::Other) {
            println!("\nCreate two custom shortcuts in Settings → Keyboard.");
        }
    }

    // On Windows this also registers the shortcuts themselves — there is no
    // compositor to hold them, so the agent has to be running.
    #[cfg(windows)]
    let (question, help) = (
        "Run the Dictate hotkey agent at login?",
        "Required for the shortcuts to work at all. Also keeps live + smart daemons warm.",
    );
    #[cfg(unix)]
    let (question, help) = (
        "Start warm Dictate daemons at login for instant shortcuts?",
        "Recommended: removes cold-start delay by keeping live + smart daemons idle until you press a shortcut.",
    );

    let warm = Confirm::new(question)
        .with_default(true)
        .with_help_message(help)
        .prompt()?;
    if warm {
        if let Err(e) = run_autostart_command(&AutostartCommand::Install) {
            eprintln!("⚠ Could not set up autostart: {e}");
            eprintln!("  You can retry later with: dictate autostart install");
        }
    } else {
        println!("Skipped. You can enable later with: dictate autostart install");
    }

    let run_doc = Confirm::new("Run dictate doctor now?")
        .with_default(true)
        .prompt()?;
    if run_doc {
        let mut config = Config::load_env_file(env_path).unwrap_or_else(|_| Config::from_env());
        let text_path = Config::text_config_path_for_env_file(env_path);
        config.load_text_config_file(&text_path).ok();
        println!();
        run_doctor(&config, env_path);
    } else {
        println!("\nRun: dictate doctor");
    }
    Ok(())
}
