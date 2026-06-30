//! CLI-driven configuration commands: wizard, get, set, edit, and shortcut printing.

use crate::config::{Config, YDOTOOL_PASTE_SHELL};
use crate::profile::DictateProfile;
use anyhow::{anyhow, Result};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

// ─── Data structures ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Interactively create or update the config file.
    Wizard(Box<WizardOptions>),
    /// Print the current config file, or one config key.
    Get { key: Option<String> },
    /// Set one config key in the config file.
    Set { key: String, value: String },
    /// Open the config file in $EDITOR.
    Edit,
}

#[derive(Subcommand)]
pub enum AutostartCommand {
    /// Install and start warm live + smart user services
    Install,
    /// Stop and remove warm user services
    Remove,
    /// Print warm service status
    Status,
}

#[derive(ClapArgs, Default)]
pub struct WizardOptions {
    #[arg(long)]
    pub provider: Option<String>,
    #[arg(long)]
    pub mistral_api_key: Option<String>,
    #[arg(long)]
    pub groq_api_key: Option<String>,
    #[arg(long)]
    pub mistral_model: Option<String>,
    #[arg(long)]
    pub mistral_realtime_model: Option<String>,
    #[arg(long)]
    pub profile: Option<String>,
    #[arg(long)]
    pub groq_model: Option<String>,
    #[arg(long)]
    pub whisper_model: Option<String>,
    #[arg(long)]
    pub language: Option<String>,
    #[arg(long)]
    pub output_mode: Option<String>,
    #[arg(long)]
    pub desktop: Option<String>,
    #[arg(long)]
    pub shortcut_key: Option<String>,
    #[arg(long)]
    pub audio_feedback: Option<String>,
    #[arg(long)]
    pub beep_volume: Option<String>,
}

#[derive(Parser)]
pub struct ShortcutArgs {
    #[arg(value_enum)]
    pub desktop: ShortcutDesktop,
    #[arg(long, value_enum, default_value_t = ShortcutMode::Auto)]
    pub mode: ShortcutMode,
    #[arg(long, default_value = "segmented")]
    pub profile: String,
    #[arg(long, default_value = "SUPER,R")]
    pub key: String,
    /// GNOME only: write live + smart keybindings from ~/.config/dictate/.env
    #[arg(long)]
    pub install: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShortcutDesktop {
    Hyprland,
    Niri,
    Gnome,
    Kde,
    Sway,
    Other,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum ShortcutMode {
    Auto,
    Stdout,
    Clipboard,
    Type,
    Paste,
}

/// Daemon slot toggled by desktop shortcuts (`dictate toggle live|smart`).
#[derive(Clone, Copy, PartialEq, ValueEnum)]
pub enum ToggleKind {
    Live,
    Smart,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(default) if !default.is_empty() => print!("{label} [{default}]: "),
        _ => print!("{label}: "),
    }
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if input.is_empty() {
        Ok(default.unwrap_or_default().to_string())
    } else {
        Ok(input.to_string())
    }
}

fn option_or_prompt(
    value: &Option<String>,
    message: &str,
    default: Option<&str>,
) -> Result<String> {
    match value {
        Some(v) => Ok(v.clone()),
        None => prompt(message, default),
    }
}

// ─── Config file operations ──────────────────────────────────────────────────

pub fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("dictate")
        .join(".env")
}

pub fn ensure_config_file(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::write(
            path,
            "TRANSCRIPTION_PROVIDER=mistral\nDICTATE_PROFILE=segmented\n\
             MISTRAL_MODEL=voxtral-mini-latest\n\
             MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602\n\
             MISTRAL_REALTIME_DELAY_MS=480\n\
             GROQ_MODEL=whisper-large-v3-turbo\nTRANSCRIPTION_LANGUAGE=auto\n\
             TRANSCRIPTION_TIMEOUT_SECONDS=60\nTRANSCRIPTION_MAX_RETRIES=3\n\
             ENABLE_AUDIO_FEEDBACK=true\nBEEP_VOLUME=0.1\n\
             SHORTCUT_OUTPUT=type\nSHORTCUT_KEY_LIVE=SUPER,R\nSHORTCUT_KEY_SMART=SUPER,SHIFT,R\n",
        )?;
    }
    Ok(())
}

fn normalize_config_key(key: &str) -> String {
    match key.trim().to_lowercase().replace('-', "_").as_str() {
        "provider" => "TRANSCRIPTION_PROVIDER",
        "language" | "transcription_language" => "TRANSCRIPTION_LANGUAGE",
        "timeout" | "transcription_timeout" | "transcription_timeout_seconds" => {
            "TRANSCRIPTION_TIMEOUT_SECONDS"
        }
        "retries" | "max_retries" | "transcription_max_retries" => "TRANSCRIPTION_MAX_RETRIES",
        "mistral_key" | "mistral_api_key" => "MISTRAL_API_KEY",
        "polish_provider" | "polish-provider" => "POLISH_PROVIDER",
        "ollama_base_url" | "ollama-base-url" => "OLLAMA_BASE_URL",
        "ollama_api_key" | "ollama-api-key" => "OLLAMA_API_KEY",
        "ollama_polish_model" | "ollama-polish-model" => "OLLAMA_POLISH_MODEL",
        "mistral_model" => "MISTRAL_MODEL",
        "mistral_realtime_model" | "realtime_model" => "MISTRAL_REALTIME_MODEL",
        "mistral_realtime_base_url" | "realtime_base_url" => "MISTRAL_REALTIME_BASE_URL",
        "mistral_realtime_delay" | "mistral_realtime_delay_ms" | "realtime_delay" => {
            "MISTRAL_REALTIME_DELAY_MS"
        }
        "profile" | "dictate_profile" => "DICTATE_PROFILE",
        "batch_mode" | "batch" => "BATCH_MODE",
        "transcription_mode" | "stt_mode" => "TRANSCRIPTION_MODE",
        "shortcut_output" | "shortcut_output_mode" => "SHORTCUT_OUTPUT",
        "mistral_base_url" => "MISTRAL_BASE_URL",
        "groq_key" | "groq_api_key" => "GROQ_API_KEY",
        "groq_model" => "GROQ_MODEL",
        "groq_base_url" => "GROQ_BASE_URL",
        "local_model" | "whisper_model" => "WHISPER_MODEL",
        "audio_feedback" | "enable_audio_feedback" => "ENABLE_AUDIO_FEEDBACK",
        "beep_volume" => "BEEP_VOLUME",
        "shortcut" | "shortcut_key" => "SHORTCUT_KEY",
        "shortcut_key_live" | "shortcut_live" | "shortcut-live" => "SHORTCUT_KEY_LIVE",
        "shortcut_key_smart" | "shortcut_smart" | "shortcut-smart" => "SHORTCUT_KEY_SMART",
        "desktop" | "shortcut_desktop" => "SHORTCUT_DESKTOP",
        "mode" | "output_mode" => "SHORTCUT_OUTPUT",
        other => return other.to_uppercase(),
    }
    .to_string()
}

pub fn read_config_value(path: &PathBuf, key: &str) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let key = normalize_config_key(key);
    let contents = std::fs::read_to_string(path)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((line_key, value)) = trimmed.split_once('=') {
            if line_key.trim() == key {
                return Ok(Some(value.trim().to_string()));
            }
        }
    }
    Ok(None)
}

pub fn set_config_value(path: &PathBuf, key: &str, value: &str) -> Result<()> {
    ensure_config_file(path)?;
    let key = normalize_config_key(key);
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    let mut found = false;
    let mut lines: Vec<String> = contents
        .lines()
        .map(|line| {
            if !found && !line.trim_start().starts_with('#') {
                if let Some((lk, _)) = line.split_once('=') {
                    if lk.trim() == key {
                        found = true;
                        return format!("{key}={value}");
                    }
                }
            }
            line.to_string()
        })
        .collect();

    if !found {
        lines.push(format!("{key}={value}"));
    }

    std::fs::write(path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

// ─── Wizard ──────────────────────────────────────────────────────────────────

fn run_config_wizard(path: &PathBuf, options: &WizardOptions) -> Result<()> {
    ensure_config_file(path)?;
    println!("Configuring Dictate at {}", path.display());

    let provider = option_or_prompt(
        &options.provider,
        "Provider (mistral/groq/local)",
        Some("mistral"),
    )?
    .to_lowercase();
    set_config_value(path, "provider", &provider)?;

    let profile = option_or_prompt(
        &options.profile,
        "Dictation style (Enter = segmented default; legacy: live_typing | smart_paste | batch_clip)",
        Some("segmented"),
    )?
    .to_lowercase()
    .replace(' ', "_");
    set_config_value(path, "profile", &profile)?;

    match profile.as_str() {
        "smart_paste" | "batch_clip" => {
            set_config_value(path, "batch-mode", "true")?;
            set_config_value(path, "transcription-mode", "auto")?;
        }
        "live_typing" | "segmented" => {
            set_config_value(path, "batch-mode", "false")?;
            set_config_value(path, "transcription-mode", "auto")?;
        }
        _ => {
            set_config_value(path, "batch-mode", "false")?;
            set_config_value(path, "transcription-mode", "auto")?;
        }
    }

    match provider.as_str() {
        "mistral" => {
            let key = option_or_prompt(&options.mistral_api_key, "Mistral API key", None)?;
            if !key.is_empty() {
                set_config_value(path, "mistral-api-key", &key)?;
            }
            if options.mistral_model.is_some() {
                let model = option_or_prompt(
                    &options.mistral_model,
                    "Mistral batch model",
                    Some("voxtral-mini-latest"),
                )?;
                set_config_value(path, "mistral-model", &model)?;
            }
            if options.mistral_realtime_model.is_some() {
                let rt_model = option_or_prompt(
                    &options.mistral_realtime_model,
                    "Mistral realtime model",
                    Some("voxtral-mini-transcribe-realtime-2602"),
                )?;
                set_config_value(path, "mistral-realtime-model", &rt_model)?;
            }
        }
        "groq" => {
            let key = option_or_prompt(&options.groq_api_key, "Groq API key", None)?;
            if !key.is_empty() {
                set_config_value(path, "groq-api-key", &key)?;
            }
            let model = option_or_prompt(
                &options.groq_model,
                "Groq model",
                Some("whisper-large-v3-turbo"),
            )?;
            set_config_value(path, "groq-model", &model)?;
        }
        "local" => {
            let model = option_or_prompt(
                &options.whisper_model,
                "Local Whisper model",
                Some("ggml-base.en.bin"),
            )?;
            set_config_value(path, "whisper-model", &model)?;
        }
        _ => {
            anyhow::bail!("Unsupported provider '{provider}'. Use mistral, groq, or local.");
        }
    }

    let language = option_or_prompt(
        &options.language,
        "Language (auto or ISO code)",
        Some("auto"),
    )?;
    set_config_value(path, "language", &language)?;

    let default_output = if profile == "smart_paste" {
        "paste"
    } else {
        "type"
    };
    let output_mode = option_or_prompt(
        &options.output_mode,
        "Shortcut output: type | clipboard | paste | stdout",
        Some(default_output),
    )?;
    set_config_value(path, "shortcut-output", &output_mode)?;

    let desktop = option_or_prompt(
        &options.desktop,
        "Desktop (hyprland/niri/gnome/kde/sway/other)",
        Some("hyprland"),
    )?
    .to_lowercase();
    set_config_value(path, "shortcut-desktop", &desktop)?;

    let default_shortcut = match desktop.as_str() {
        "niri" => "Mod,R",
        "gnome" => "<Super>r",
        "kde" => "Meta+R",
        "sway" => "Mod4+R",
        _ => "SUPER,R",
    };
    let shortcut = option_or_prompt(
        &options.shortcut_key,
        "Shortcut key",
        Some(default_shortcut),
    )?;
    set_config_value(path, "shortcut-key", &shortcut)?;

    let af = option_or_prompt(
        &options.audio_feedback,
        "Audio feedback (true/false)",
        Some("true"),
    )?;
    set_config_value(path, "audio-feedback", &af)?;

    if af.trim().eq_ignore_ascii_case("true") || options.beep_volume.is_some() {
        let vol = option_or_prompt(
            &options.beep_volume,
            "Beep volume (0.0 to 1.0)",
            Some("0.1"),
        )?;
        set_config_value(path, "beep-volume", &vol)?;
    }

    println!("\nSaved config to {}", path.display());
    println!(
        "Run `dictate shortcuts {desktop} --profile {profile} --mode {output_mode} --key {shortcut}` \
         to print a shortcut snippet."
    );
    if profile == "smart_paste" || profile == "live_typing" {
        println!("Daemon profiles: run `dictate --daemon` in the background (or use the shortcut below).");
    }
    Ok(())
}

// ─── Config command dispatcher ───────────────────────────────────────────────

pub fn run_config_command(command: &ConfigCommand, path: &PathBuf) -> Result<()> {
    match command {
        ConfigCommand::Wizard(options) => run_config_wizard(path, options),
        ConfigCommand::Get { key } => {
            if let Some(key) = key {
                match read_config_value(path, key)? {
                    Some(value) => println!("{value}"),
                    None => anyhow::bail!("Config key '{key}' is not set"),
                }
            } else if path.exists() {
                print!("{}", std::fs::read_to_string(path)?);
            } else {
                anyhow::bail!("Config file does not exist: {}", path.display());
            }
            Ok(())
        }
        ConfigCommand::Set { key, value } => {
            set_config_value(path, key, value)?;
            println!("Set {} in {}", normalize_config_key(key), path.display());
            Ok(())
        }
        ConfigCommand::Edit => {
            ensure_config_file(path)?;
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
            let status = ProcessCommand::new(editor).arg(path).status()?;
            if status.success() {
                Ok(())
            } else {
                Err(anyhow!("Editor exited with status {status}"))
            }
        }
    }
}

// ─── Doctor ──────────────────────────────────────────────────────────────────

pub fn run_doctor(config: &Config, env_path: &Path) {
    println!("dictate doctor\n");

    if env_path.exists() {
        if Config::prune_retired_env_keys(env_path).unwrap_or(false) {
            println!("✓ Config file: {} (removed obsolete keys)", env_path.display());
        } else {
            println!("✓ Config file: {}", env_path.display());
        }
    } else {
        println!("✗ Config file missing: {}", env_path.display());
        println!("  Run: dictate config wizard");
    }

    println!(
        "  Profile: {} ({}) — {}",
        config.profile.label(),
        config.profile.as_str(),
        config.profile.description()
    );
    if config.profile.wants_daemon() {
        println!("  Run: dictate --daemon (shortcut toggles recording via SIGUSR1)");
    }
    if config.profile.uses_segment_polish() || config.profile.uses_llm_polish() {
        match config.resolve_polish_backend() {
            Some(crate::config::PolishBackend::Mistral) => {
                println!("✓ LLM polish: Mistral chat ([polish] in text.toml)");
            }
            Some(crate::config::PolishBackend::Ollama) => {
                let model = if config.text_processing.polish.model.trim().is_empty()
                    || config.text_processing.polish.model.contains("mistral")
                {
                    std::env::var("OLLAMA_POLISH_MODEL")
                        .unwrap_or_else(|_| "gemma-4".to_string())
                } else {
                    config.text_processing.polish.model.clone()
                };
                println!(
                    "✓ LLM polish: Ollama at {} (model: {model})",
                    config.ollama_base_url
                );
            }
            None => println!(
                "  LLM polish off — set MISTRAL_API_KEY or run Ollama (POLISH_PROVIDER=auto|ollama)"
            ),
        }
    }
    println!(
        "✓ Clipboard commands: auto when you copied text (≥12 chars) and speak an instruction (same shortcut)"
    );
    let live_key = config.shortcut_key_live.as_deref().unwrap_or("SUPER,R");
    let smart_key = config
        .shortcut_key_smart
        .as_deref()
        .unwrap_or("SUPER,SHIFT,R");
    println!("✓ Live shortcut: {live_key} → realtime typing (minimal local cleanup)");
    println!("✓ Smart shortcut: {smart_key} → record, polish, paste once");
    println!(
        "✓ Words UI: run `dictate words` to edit {}",
        Config::text_config_path_for_env_file(env_path).display()
    );
    let word_count = crate::text_processing::preferred_vocabulary(&config.text_processing).len();
    if word_count > 0 {
        println!("  Preferred vocabulary: {word_count} terms configured");
    }
    if let Some(key) = &config.shortcut_key {
        println!("  Legacy shortcut key (saved): {key}");
    }
    if let Some(desktop) = &config.shortcut_desktop {
        println!("  Shortcut desktop (saved): {desktop}");
    }
    if config.batch_mode || config.profile.implies_batch_stt() {
        println!("  Batch STT path (profile or legacy BATCH_MODE)");
    }

    match config.transcription_provider.to_lowercase().as_str() {
        "mistral" => {
            if config
                .mistral_api_key
                .as_ref()
                .is_some_and(|k| !k.is_empty())
            {
                println!("✓ MISTRAL_API_KEY is set");
            } else {
                println!("✗ MISTRAL_API_KEY missing (required for Mistral)");
            }
        }
        "groq" => {
            if config.groq_api_key.as_ref().is_some_and(|k| !k.is_empty()) {
                println!("✓ GROQ_API_KEY is set");
            } else {
                println!("✗ GROQ_API_KEY missing");
            }
        }
        "local" => {
            let path = Config::model_path(&config.whisper_model);
            if path.exists() {
                println!("✓ Local model: {}", path.display());
            } else {
                println!("✗ Local model missing — run: dictate --download-model");
            }
        }
        other => println!("✗ Unknown provider: {other}"),
    }

    if let Some(pipe) = &config.default_pipe_to {
        println!("✓ Default output pipe: {}", pipe.join(" "));
    } else {
        println!("  Default output: stdout (set SHORTCUT_OUTPUT in .env or use --pipe-to)");
    }

    print_autostart_status();
    println!(
        "  Stuck modifiers after paste? Run: ydotool key 29:0 42:0 56:0 125:0 (release Ctrl/Shift/Alt/Super)"
    );

    for (name, check) in [
        ("wl-copy", "command -v wl-copy"),
        ("ydotool", "command -v ydotool"),
        ("pw-cli", "pw-cli ls Node"),
    ] {
        let ok = ProcessCommand::new("sh")
            .arg("-c")
            .arg(check)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            println!("✓ {name} available");
        } else {
            println!("  Optional: {name} not detected ({check})");
        }
    }

    if let Err(e) = config.validate() {
        println!("\n⚠ Validation: {e}");
    } else {
        println!("\n✓ Configuration validates");
    }
}

// ─── Shortcut printing ───────────────────────────────────────────────────────

fn shortcut_command(mode: &ShortcutMode, profile: &str) -> String {
    let daemon = matches!(
        DictateProfile::parse(profile),
        Some(DictateProfile::Segmented)
            | Some(DictateProfile::LiveTyping)
            | Some(DictateProfile::SmartPaste)
            | Some(DictateProfile::Command)
    ) || profile == "segmented"
        || profile == "live_typing"
        || profile == "smart_paste"
        || profile == "command";
    let daemon_flag = if daemon { " --daemon" } else { "" };
    let mode_flag = if profile == "smart_paste" || profile == "smart" {
        " --mode smart"
    } else if profile == "live_typing" || profile == "live" {
        " --mode live"
    } else {
        ""
    };
    let base = format!("dictate{daemon_flag}{mode_flag}");
    match mode {
        ShortcutMode::Auto => base,
        ShortcutMode::Stdout => base,
        ShortcutMode::Clipboard => format!("{base} --pipe-to wl-copy"),
        ShortcutMode::Type => format!("{base} --pipe-to ydotool type --file -"),
        ShortcutMode::Paste => {
            format!("{base} --pipe-to sh -c '{YDOTOOL_PASTE_SHELL}'")
        }
    }
}

fn mode_name(mode: &ShortcutMode) -> &'static str {
    match mode {
        ShortcutMode::Auto => "auto output",
        ShortcutMode::Stdout => "stdout",
        ShortcutMode::Clipboard => "clipboard",
        ShortcutMode::Type => "direct typing",
        ShortcutMode::Paste => "clipboard + paste",
    }
}

fn toggle_shell(cmd: &str) -> String {
    // SIGUSR1 only to the matching long-running daemon (not one-shot foreground dictate).
    let pattern = cmd
        .replacen("dictate", "[d]ictate", 1)
        .replace('\'', "'\"'\"'");
    format!("pgrep -f '{pattern}' >/dev/null && pkill -f --signal SIGUSR1 '{pattern}' || ({cmd} &)")
}

fn toggle_daemon_argv(kind: ToggleKind) -> Vec<String> {
    let mut v = vec![
        "dictate".to_string(),
        "--daemon".to_string(),
        "--mode".to_string(),
    ];
    match kind {
        ToggleKind::Live => {
            v.push("live".to_string());
            v.extend([
                "--pipe-to".to_string(),
                "ydotool".to_string(),
                "type".to_string(),
                "--file".to_string(),
                "-".to_string(),
            ]);
        }
        ToggleKind::Smart => {
            v.push("smart".to_string());
            v.extend([
                "--pipe-to".to_string(),
                "sh".to_string(),
                "-c".to_string(),
                YDOTOOL_PASTE_SHELL.to_string(),
            ]);
        }
    }
    v
}

fn daemon_match_pattern(kind: ToggleKind) -> String {
    match kind {
        ToggleKind::Live => "[d]ictate --daemon --mode live".to_string(),
        ToggleKind::Smart => "[d]ictate --daemon --mode smart".to_string(),
    }
}

/// Stop the other dictate daemon so only one holds the mic / ydotool session.
fn stop_other_daemons(keep: ToggleKind) {
    for kind in [ToggleKind::Live, ToggleKind::Smart] {
        if kind == keep {
            continue;
        }
        let pattern = daemon_match_pattern(kind);
        let _ = ProcessCommand::new("pkill")
            .args(["-f", "--signal", "SIGTERM", &pattern])
            .status();
    }
}

/// Start or SIGUSR1-toggle the long-running daemon for `live` or `smart`.
pub fn run_toggle_daemon(kind: ToggleKind) -> Result<()> {
    let argv = toggle_daemon_argv(kind);
    let pattern = daemon_match_pattern(kind);
    let running = ProcessCommand::new("pgrep")
        .args(["-f", &pattern])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if running {
        stop_other_daemons(kind);
        std::thread::sleep(std::time::Duration::from_millis(200));
        let status = ProcessCommand::new("pkill")
            .args(["-f", "--signal", "SIGUSR1", &pattern])
            .status()?;
        if !status.success() {
            anyhow::bail!("pkill failed (is dictate running?)");
        }
        return Ok(());
    }
    stop_other_daemons(kind);
    std::thread::sleep(std::time::Duration::from_millis(200));
    ProcessCommand::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

const LIVE_SERVICE: &str = "dictate-live.service";
const SMART_SERVICE: &str = "dictate-smart.service";

fn systemd_user_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("systemd/user")
}

fn service_path(name: &str) -> PathBuf {
    systemd_user_dir().join(name)
}

fn current_exe_for_service() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| anyhow!("Could not resolve current executable: {e}"))
}

fn warm_daemon_command(kind: ToggleKind) -> Result<String> {
    let exe = current_exe_for_service()?;
    let exe = exe.display();
    Ok(match kind {
        ToggleKind::Live => {
            format!("{exe} --daemon --mode live --idle-on-start --pipe-to ydotool type --file -")
        }
        ToggleKind::Smart => format!(
            "{exe} --daemon --mode smart --idle-on-start --pipe-to sh -c '{YDOTOOL_PASTE_SHELL}'"
        ),
    })
}

fn service_contents(kind: ToggleKind) -> Result<String> {
    let description = match kind {
        ToggleKind::Live => "Dictate warm live typing daemon",
        ToggleKind::Smart => "Dictate warm smart paste daemon",
    };
    let command = warm_daemon_command(kind)?;
    Ok(format!(
        "[Unit]\nDescription={description}\nAfter=graphical-session.target pipewire.service\n\n[Service]\nType=simple\nExecStart={command}\nRestart=on-failure\nRestartSec=2\n\n[Install]\nWantedBy=default.target\n",
    ))
}

fn run_systemctl(args: &[&str]) -> Result<bool> {
    let status = ProcessCommand::new("systemctl")
        .arg("--user")
        .args(args)
        .status()?;
    Ok(status.success())
}

fn service_active(name: &str) -> bool {
    ProcessCommand::new("systemctl")
        .args(["--user", "is-active", "--quiet", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn install_autostart_services() -> Result<()> {
    let dir = systemd_user_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        service_path(LIVE_SERVICE),
        service_contents(ToggleKind::Live)?,
    )?;
    std::fs::write(
        service_path(SMART_SERVICE),
        service_contents(ToggleKind::Smart)?,
    )?;

    run_systemctl(&["daemon-reload"])?;
    let enabled = run_systemctl(&["enable", LIVE_SERVICE, SMART_SERVICE])?;
    if !enabled {
        anyhow::bail!("systemctl --user enable failed");
    }
    let restarted = run_systemctl(&["restart", LIVE_SERVICE, SMART_SERVICE])?;
    if !restarted {
        anyhow::bail!("systemctl --user restart failed");
    }

    println!("Warm Dictate daemons installed and started:");
    println!(
        "  {LIVE_SERVICE}:  {}",
        service_path(LIVE_SERVICE).display()
    );
    println!(
        "  {SMART_SERVICE}: {}",
        service_path(SMART_SERVICE).display()
    );
    println!("Shortcuts now signal warm daemons instead of cold-starting them.");
    Ok(())
}

fn remove_autostart_services() -> Result<()> {
    let disable_args = ["disable", "--now", LIVE_SERVICE, SMART_SERVICE];
    let _ = run_systemctl(&disable_args[..]);
    for name in [LIVE_SERVICE, SMART_SERVICE] {
        let path = service_path(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    let reload_args = ["daemon-reload"];
    let _ = run_systemctl(&reload_args[..]);
    println!("Warm Dictate daemons removed.");
    Ok(())
}

pub fn print_autostart_status() {
    println!("Warm Dictate daemons:");
    for (name, label) in [(LIVE_SERVICE, "Live"), (SMART_SERVICE, "Smart")] {
        let installed = service_path(name).exists();
        let active = service_active(name);
        let marker = if active {
            "✓"
        } else if installed {
            "!"
        } else {
            "✗"
        };
        let state = if active {
            "active"
        } else if installed {
            "installed but not active"
        } else {
            "not installed"
        };
        println!("{marker} {label}: {state} ({name})");
    }
    if !service_active(LIVE_SERVICE) || !service_active(SMART_SERVICE) {
        println!("  Run: dictate autostart install");
    }
}

pub fn run_autostart_command(command: &AutostartCommand) -> Result<()> {
    match command {
        AutostartCommand::Install => install_autostart_services(),
        AutostartCommand::Remove => remove_autostart_services(),
        AutostartCommand::Status => {
            print_autostart_status();
            Ok(())
        }
    }
}

fn gnome_binding(key: &str) -> String {
    let mut mods = Vec::new();
    let mut keycap = String::new();
    for part in key.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match p.to_uppercase().as_str() {
            "SUPER" | "META" | "WIN" => mods.push("<Super>"),
            "SHIFT" => mods.push("<Shift>"),
            "CTRL" | "CONTROL" => mods.push("<Control>"),
            "ALT" => mods.push("<Alt>"),
            other => keycap = other.to_lowercase(),
        }
    }
    format!("{}{}", mods.join(""), keycap)
}

fn read_config_keys(env_path: &Path) -> Result<(String, String)> {
    let path = env_path.to_path_buf();
    let live =
        read_config_value(&path, "SHORTCUT_KEY_LIVE")?.unwrap_or_else(|| "SUPER,R".to_string());
    let smart = read_config_value(&path, "SHORTCUT_KEY_SMART")?
        .unwrap_or_else(|| "SUPER,SHIFT,R".to_string());
    Ok((live, smart))
}

/// Write GNOME custom-keybindings for live + smart (`dictate toggle …`).
pub fn install_gnome_shortcuts(env_path: &Path) -> Result<()> {
    let schema = "org.gnome.settings-daemon.plugins.media-keys";
    let base = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";
    let live_path = format!("{base}/dictate-live/");
    let smart_path = format!("{base}/dictate-smart/");
    let (live_key, smart_key) = read_config_keys(env_path)?;

    let list = ProcessCommand::new("gsettings")
        .args(["get", schema, "custom-keybindings"])
        .output()?;
    let mut paths: Vec<String> = if list.status.success() {
        let raw = String::from_utf8_lossy(&list.stdout);
        let trimmed = raw.trim().trim_start_matches('[').trim_end_matches(']');
        trimmed
            .split(',')
            .map(|s| s.trim().trim_matches('"').trim_matches('\''))
            .filter(|s| !s.is_empty())
            .filter(|s| {
                !s.contains("dictate-type")
                    && !s.contains("dictate-clip")
                    && !s.contains("dictate-stream")
            })
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };
    for p in [&live_path, &smart_path] {
        if !paths.iter().any(|x| x == p) {
            paths.push(p.clone());
        }
    }
    let list_val = format!(
        "[{}]",
        paths
            .iter()
            .map(|p| format!("'{p}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let status = ProcessCommand::new("gsettings")
        .args(["set", schema, "custom-keybindings", &list_val])
        .status()?;
    if !status.success() {
        anyhow::bail!("gsettings failed to update custom-keybindings");
    }

    let bind_schema = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:";
    for (path, name, binding, toggle) in [
        (
            live_path.as_str(),
            "Dictate live typing",
            gnome_binding(&live_key),
            "live",
        ),
        (
            smart_path.as_str(),
            "Dictate smart paste",
            gnome_binding(&smart_key),
            "smart",
        ),
    ] {
        let full = format!("{bind_schema}{path}");
        let status = ProcessCommand::new("gsettings")
            .args(["set", &full, "name", name])
            .status()?;
        if !status.success() {
            anyhow::bail!("gsettings failed to set shortcut name for {path}");
        }
        let status = ProcessCommand::new("gsettings")
            .args(["set", &full, "command", &format!("dictate toggle {toggle}")])
            .status()?;
        if !status.success() {
            anyhow::bail!("gsettings failed to set shortcut command for {path}");
        }
        let status = ProcessCommand::new("gsettings")
            .args(["set", &full, "binding", &binding])
            .status()?;
        if !status.success() {
            anyhow::bail!("gsettings failed to set shortcut binding for {path}");
        }
    }
    println!("GNOME shortcuts installed (dictate toggle live / smart).");
    println!("  Live:  {live_key} → dictate toggle live");
    println!("  Smart: {smart_key} → dictate toggle smart");
    Ok(())
}

pub fn print_shortcut(args: &ShortcutArgs, env_path: &Path) {
    if args.install {
        if !matches!(args.desktop, ShortcutDesktop::Gnome) {
            eprintln!("--install is only supported for gnome");
            return;
        }
        if let Err(e) = install_gnome_shortcuts(env_path) {
            eprintln!("Failed to install GNOME shortcuts: {e}");
        }
        return;
    }
    let cmd = shortcut_command(&args.mode, &args.profile);
    let shell = toggle_shell(&cmd);

    match args.desktop {
        ShortcutDesktop::Hyprland => {
            let key = args.key.replace(',', ", ");
            println!(
                "# Dictate — profile {} ({})",
                args.profile,
                mode_name(&args.mode)
            );
            println!("bind = {key}, exec, {shell}");
            if let Some(mod_part) = args.key.split(',').next() {
                let char_part = args.key.split(',').nth(1).unwrap_or("R");
                println!("# Clipboard variant:");
                println!(
                    "# bind = {mod_part} SHIFT, {char_part}, exec, \
                     pgrep -f '[d]ictate --daemon --pipe-to wl-copy' >/dev/null && pkill -f --signal SIGUSR1 '[d]ictate --daemon --pipe-to wl-copy' \
                     || (dictate --daemon --pipe-to wl-copy &)"
                );
            }
        }
        ShortcutDesktop::Niri => {
            let key = args.key.replace(',', "+");
            println!(
                "// Dictate — profile {} ({})",
                args.profile,
                mode_name(&args.mode)
            );
            println!("{key} {{ spawn \"sh\" \"-c\" \"{shell}\"; }}");
            if let Some(mod_part) = args.key.split(',').next() {
                let char_part = args.key.split(',').nth(1).unwrap_or("R");
                println!("// Clipboard variant:");
                println!(
                    "// Shift+{mod_part}+{char_part} {{ spawn \"sh\" \"-c\" \
                     \"pgrep -f '[d]ictate --daemon --pipe-to wl-copy' >/dev/null && pkill -f --signal SIGUSR1 '[d]ictate --daemon --pipe-to wl-copy' \
                     || (dictate --daemon --pipe-to wl-copy &)\"; }}"
                );
            }
        }
        ShortcutDesktop::Gnome => {
            let toggle = if args.profile == "smart_paste" || args.profile == "smart" {
                "smart"
            } else {
                "live"
            };
            println!("# GNOME Custom Shortcut");
            println!("# 1. Settings → Keyboard → Keyboard Shortcuts");
            println!("# 2. Scroll to bottom, click +");
            println!("# 3. Name: Dictate ({})", mode_name(&args.mode));
            println!("#    Command: dictate toggle {toggle}");
            println!("#    Shortcut: {}", args.key);
            println!("# Or: dictate shortcuts gnome --install");
        }
        ShortcutDesktop::Kde | ShortcutDesktop::Sway => {
            let name = match args.desktop {
                ShortcutDesktop::Kde => "KDE",
                _ => "Sway",
            };
            println!("# {name} Custom Shortcut");
            println!("# Add this command as a custom shortcut:");
            println!("{shell}");
        }
        ShortcutDesktop::Other => {
            println!("# Generic Custom Shortcut");
            println!("# Add this command in your desktop settings:");
            println!("{shell}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_shortcut_uses_daemon_and_configured_output() {
        let cmd = shortcut_command(&ShortcutMode::Auto, "segmented");
        assert_eq!(cmd, "dictate --daemon");
    }

    #[test]
    fn explicit_type_shortcut_still_available() {
        let cmd = shortcut_command(&ShortcutMode::Type, "segmented");
        assert_eq!(cmd, "dictate --daemon --pipe-to ydotool type --file -");
    }

    #[test]
    fn live_shortcut_targets_live_daemon_only() {
        let cmd = shortcut_command(&ShortcutMode::Type, "live_typing");
        assert_eq!(
            toggle_shell(&cmd),
            "pgrep -f '[d]ictate --daemon --mode live --pipe-to ydotool type --file -' >/dev/null && pkill -f --signal SIGUSR1 '[d]ictate --daemon --mode live --pipe-to ydotool type --file -' || (dictate --daemon --mode live --pipe-to ydotool type --file - &)"
        );
    }
}
