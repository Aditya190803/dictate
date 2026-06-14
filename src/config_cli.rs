//! CLI-driven configuration commands: wizard, get, set, edit, and shortcut printing.

use crate::config::Config;
use crate::profile::DictateProfile;
use anyhow::{anyhow, Result};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
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
    pub transcription_mode: Option<String>,
    #[arg(long)]
    pub batch_mode: Option<String>,
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
    #[arg(long, value_enum, default_value_t = ShortcutMode::Type)]
    pub mode: ShortcutMode,
    #[arg(long, default_value = "live_typing")]
    pub profile: String,
    #[arg(long, default_value = "SUPER,R")]
    pub key: String,
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
    Stdout,
    Clipboard,
    Type,
    Paste,
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

fn option_or_prompt(value: &Option<String>, message: &str, default: Option<&str>) -> Result<String> {
    match value {
        Some(v) => Ok(v.clone()),
        None => prompt(message, default),
    }
}

// ─── Config file operations ──────────────────────────────────────────────────

pub fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map_or_else(|_| PathBuf::from("."), PathBuf::from)
        })
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
            "TRANSCRIPTION_PROVIDER=mistral\nDICTATE_PROFILE=live_typing\n\
             MISTRAL_MODEL=voxtral-mini-latest\n\
             MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602\n\
             MISTRAL_REALTIME_DELAY_MS=480\n\
             GROQ_MODEL=whisper-large-v3-turbo\nTRANSCRIPTION_LANGUAGE=auto\n\
             TRANSCRIPTION_TIMEOUT_SECONDS=60\nTRANSCRIPTION_MAX_RETRIES=3\n\
             ENABLE_AUDIO_FEEDBACK=true\nBEEP_VOLUME=0.1\n\
             SHORTCUT_OUTPUT=type\n",
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
        "desktop" | "shortcut_desktop" => "SHORTCUT_DESKTOP",
        "mode" | "output_mode" => "SHORTCUT_OUTPUT",
        other => return other.to_uppercase(),
    }
    .to_string()
}

fn read_config_value(path: &PathBuf, key: &str) -> Result<Option<String>> {
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

fn set_config_value(path: &PathBuf, key: &str, value: &str) -> Result<()> {
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
        "Dictation style: live_typing | smart_paste | batch_clip",
        Some("live_typing"),
    )?
    .to_lowercase()
    .replace(' ', "_");
    set_config_value(path, "profile", &profile)?;

    match profile.as_str() {
        "smart_paste" => {
            set_config_value(path, "batch-mode", "true")?;
        }
        "batch_clip" => {
            set_config_value(path, "batch-mode", "true")?;
        }
        _ => {
            set_config_value(path, "batch-mode", "false")?;
        }
    }

    match provider.as_str() {
        "mistral" => {
            let key = option_or_prompt(&options.mistral_api_key, "Mistral API key", None)?;
            if !key.is_empty() {
                set_config_value(path, "mistral-api-key", &key)?;
            }
            if options.transcription_mode.is_some() || options.batch_mode.is_some() {
                let batch = option_or_prompt(
                    &options.batch_mode,
                    "Batch mode (true/false) [advanced]",
                    Some("false"),
                )?;
                set_config_value(path, "batch-mode", &batch)?;
                let mode = option_or_prompt(
                    &options.transcription_mode,
                    "Transcription mode (auto/realtime/batch) [advanced]",
                    Some("auto"),
                )?;
                set_config_value(path, "transcription-mode", &mode)?;
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

    let language =
        option_or_prompt(&options.language, "Language (auto or ISO code)", Some("auto"))?;
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
    let shortcut = option_or_prompt(&options.shortcut_key, "Shortcut key", Some(default_shortcut))?;
    set_config_value(path, "shortcut-key", &shortcut)?;

    let af = option_or_prompt(&options.audio_feedback, "Audio feedback (true/false)", Some("true"))?;
    set_config_value(path, "audio-feedback", &af)?;

    if af.trim().eq_ignore_ascii_case("true") || options.beep_volume.is_some() {
        let vol = option_or_prompt(&options.beep_volume, "Beep volume (0.0 to 1.0)", Some("0.1"))?;
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

pub fn run_doctor(config: &Config, env_path: &PathBuf) {
    println!("dictate doctor\n");

    if env_path.exists() {
        println!("✓ Config file: {}", env_path.display());
    } else {
        println!("✗ Config file missing: {}", env_path.display());
        println!("  Run: dictate config wizard");
    }

    println!("  Profile: {} — {}", config.profile.as_str(), config.profile.description());

    match config.transcription_provider.to_lowercase().as_str() {
        "mistral" => {
            if config.mistral_api_key.as_ref().is_some_and(|k| !k.is_empty()) {
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

    for (name, check) in [
        ("wl-copy", "wl-copy --version"),
        ("ydotool", "ydotool --version"),
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
        Some(DictateProfile::LiveTyping) | Some(DictateProfile::SmartPaste)
    ) || profile == "live_typing"
        || profile == "smart_paste";
    let daemon_flag = if daemon { " --daemon" } else { "" };
    match mode {
        ShortcutMode::Stdout => format!("dictate{daemon_flag}"),
        ShortcutMode::Clipboard => format!("dictate{daemon_flag} --pipe-to wl-copy"),
        ShortcutMode::Type => format!("dictate{daemon_flag} --pipe-to ydotool type --file -"),
        ShortcutMode::Paste => {
            format!("dictate{daemon_flag} --pipe-to sh -c 'wl-copy && ydotool key 29:125'")
        }
    }
}

fn mode_name(mode: &ShortcutMode) -> &'static str {
    match mode {
        ShortcutMode::Stdout => "stdout",
        ShortcutMode::Clipboard => "clipboard",
        ShortcutMode::Type => "direct typing",
        ShortcutMode::Paste => "clipboard + paste",
    }
}

pub fn print_shortcut(args: &ShortcutArgs) {
    let cmd = shortcut_command(&args.mode, &args.profile);
    let shell = format!(
        "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || ({cmd} &)"
    );

    match args.desktop {
        ShortcutDesktop::Hyprland => {
            let key = args.key.replace(',', ", ");
            println!("# Dictate — profile {} ({})", args.profile, mode_name(&args.mode));
            println!("bind = {key}, exec, {shell}");
            if let Some(mod_part) = args.key.split(',').next() {
                let char_part = args.key.split(',').nth(1).unwrap_or("R");
                println!("# Clipboard variant:");
                println!(
                    "# bind = {mod_part} SHIFT, {char_part}, exec, \
                     pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate \
                     || (dictate --pipe-to wl-copy &)"
                );
            }
        }
        ShortcutDesktop::Niri => {
            let key = args.key.replace(',', "+");
            println!("// Dictate — profile {} ({})", args.profile, mode_name(&args.mode));
            println!("{key} {{ spawn \"sh\" \"-c\" \"{shell}\"; }}");
            if let Some(mod_part) = args.key.split(',').next() {
                let char_part = args.key.split(',').nth(1).unwrap_or("R");
                println!("// Clipboard variant:");
                println!(
                    "// Shift+{mod_part}+{char_part} {{ spawn \"sh\" \"-c\" \
                     \"pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate \
                     || (dictate --pipe-to wl-copy &)\"; }}"
                );
            }
        }
        ShortcutDesktop::Gnome => {
            println!("# GNOME Custom Shortcut");
            println!("# 1. Settings → Keyboard → Keyboard Shortcuts");
            println!("# 2. Scroll to bottom, click +");
            println!("# 3. Name: Dictate ({})", mode_name(&args.mode));
            println!("#    Command: sh -c '{shell}'");
            println!("#    Shortcut: {}", args.key);
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
