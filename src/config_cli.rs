//! CLI-driven configuration commands: wizard, get, set, edit, and shortcut printing.

use crate::config::Config;
use crate::control::{self, Slot};
use crate::platform;
use anyhow::{anyhow, Result};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

// ─── Data structures ─────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print the current config file, or one config key.
    Get { key: Option<String> },
    /// Set one config key in the config file.
    Set { key: String, value: String },
    /// Open the config file in $EDITOR.
    Edit,
    /// Interactively create or update the config file.
    #[command(hide = true)]
    Wizard(Box<WizardOptions>),
}

/// Login integration: warm systemd user services on Linux, the global-shortcut
/// agent on Windows.
#[derive(Subcommand)]
pub enum AutostartCommand {
    /// Install and start the login integration
    Install,
    /// Stop and remove the login integration
    Remove,
    /// Print login integration status
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
    /// GNOME/Windows: register the dictation shortcut from the config file
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
    Windows,
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

/// Legacy live/smart toggle aliases (`dictate toggle live|smart`).
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

/// Shortcut defaults the platform will actually accept.
///
/// Windows reserves Win+R for the Run dialog, so `RegisterHotKey` refuses the
/// Wayland default and the shortcut would silently never fire.
pub fn default_shortcut_key() -> &'static str {
    #[cfg(windows)]
    return "CTRL,ALT,R";
    #[cfg(unix)]
    return "SUPER,R";
}

/// Create a directory (and parents) for a private config/text file.
pub fn ensure_private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    restrict_perms(path);
    Ok(())
}

/// Write a private file (API keys / vocabulary), tightening permissions.
pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    std::fs::write(path, contents)?;
    restrict_perms(path);
    Ok(())
}

/// Best-effort permissions on Unix so keys and transcripts are not
/// world-readable: `0700` for directories (owner-traversal only), `0600` for
/// files. No-op elsewhere (Windows uses ACLs).
pub fn restrict_perms(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        if let Ok(md) = std::fs::metadata(path) {
            let mut perms = md.permissions();
            if perms.mode() & 0o777 != mode {
                perms.set_mode(mode);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Split `$EDITOR` (or a fallback) into program + args for `Command`.
pub fn editor_argv(editor: &str, fallback: &str) -> (String, Vec<String>) {
    let raw = if editor.trim().is_empty() {
        fallback.to_string()
    } else {
        editor.to_string()
    };
    let mut parts = raw.split_whitespace();
    let program = parts.next().unwrap_or(fallback).to_string();
    let args: Vec<String> = parts.map(str::to_string).collect();
    (program, args)
}

pub fn ensure_config_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    if !path.exists() {
        let key = default_shortcut_key();
        write_private_file(
            path,
            format!(
                "TRANSCRIPTION_PROVIDER=mistral\nDICTATE_PROFILE=segmented\n\
                 MISTRAL_MODEL=voxtral-mini-latest\n\
                 MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602\n\
                 MISTRAL_REALTIME_DELAY_MS=480\n\
                 GROQ_MODEL=whisper-large-v3-turbo\nTRANSCRIPTION_LANGUAGE=auto\n\
                 TRANSCRIPTION_TIMEOUT_SECONDS=60\nTRANSCRIPTION_MAX_RETRIES=3\n\
                 ENABLE_AUDIO_FEEDBACK=true\nBEEP_VOLUME=0.1\n\
                 SHORTCUT_OUTPUT=type\nSHORTCUT_KEY={key}\nSHORTCUT_KEY_LIVE={key}\n"
            )
            .as_bytes(),
        )?;
    } else {
        restrict_perms(path);
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
        "opencode_key" | "opencode_api_key" => "OPENCODE_API_KEY",
        "opencode_base_url" | "opencode-base-url" => "OPENCODE_BASE_URL",
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
        "deepgram_key" | "deepgram_api_key" => "DEEPGRAM_API_KEY",
        "deepgram_model" => "DEEPGRAM_MODEL",
        "deepgram_base_url" => "DEEPGRAM_BASE_URL",
        "local_model" | "whisper_model" => "WHISPER_MODEL",
        "audio_feedback" | "enable_audio_feedback" => "ENABLE_AUDIO_FEEDBACK",
        "beep_volume" => "BEEP_VOLUME",
        "shortcut" | "shortcut_key" => "SHORTCUT_KEY",
        "shortcut_key_live" | "shortcut_live" | "shortcut-live" => "SHORTCUT_KEY_LIVE",
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

    write_private_file(path, format!("{}\n", lines.join("\n")).as_bytes())?;
    Ok(())
}

// ─── Wizard ──────────────────────────────────────────────────────────────────

fn run_config_wizard(path: &PathBuf, options: &WizardOptions) -> Result<()> {
    ensure_config_file(path)?;
    println!("Configuring Dictate at {}", path.display());

    let provider = option_or_prompt(
        &options.provider,
        "Provider (mistral/groq/deepgram/local)",
        Some("mistral"),
    )?
    .to_lowercase();
    set_config_value(path, "provider", &provider)?;

    if let Some(profile) = &options.profile {
        set_config_value(path, "profile", profile)?;
    } else {
        set_config_value(path, "profile", "segmented")?;
    }
    set_config_value(path, "batch-mode", "false")?;
    set_config_value(path, "transcription-mode", "auto")?;

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

    let output_mode = option_or_prompt(
        &options.output_mode,
        "Shortcut output: type | clipboard | paste | stdout",
        Some("type"),
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
    println!("Start dictation with `dictate` (or bind that command to your shortcut).");
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
            let editor = platform::default_editor();
            #[cfg(windows)]
            let (program, args) = editor_argv(&editor, "notepad");
            #[cfg(not(windows))]
            let (program, args) = editor_argv(&editor, "vi");
            let status = ProcessCommand::new(&program)
                .args(&args)
                .arg(path)
                .status()
                .map_err(|e| anyhow!("failed to run $EDITOR ({editor}): {e}"))?;
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
            println!(
                "✓ Config file: {} (removed obsolete keys)",
                env_path.display()
            );
        } else {
            println!("✓ Config file: {}", env_path.display());
        }
    } else {
        println!("✗ Config file missing: {}", env_path.display());
        println!("  Run: dictate setup");
    }

    println!(
        "  Profile: {} ({}) — {}",
        config.profile.label(),
        config.profile.as_str(),
        config.profile.description()
    );
    if config.profile.wants_daemon() {
        println!("  Run: dictate to start or stop");
    }
    if config.profile.uses_segment_polish() || config.profile.uses_llm_polish() {
        match config.resolve_polish_backend() {
            Some(crate::config::PolishBackend::OpenCode) => {
                let model = if config.text_processing.polish.model.trim().is_empty()
                    || config.text_processing.polish.model.contains("mistral")
                {
                    "big-pickle".to_string()
                } else {
                    config.text_processing.polish.model.clone()
                };
                println!("✓ LLM polish: OpenCode Zen (model: {model}, [polish] in text.toml)");
            }
            Some(crate::config::PolishBackend::Mistral) => {
                println!("✓ LLM polish: Mistral chat ([polish] in text.toml)");
            }
            Some(crate::config::PolishBackend::Ollama) => {
                // Mirrors the Ollama arm of `resolve_polish_model` in llm_polish.rs:
                // `big-pickle` and `mistral-*` are hosted-API ids that Ollama cannot
                // serve, so they count as unset here too. Without the `big-pickle`
                // case, doctor reports a model the runtime would never actually use.
                let configured = config.text_processing.polish.model.trim();
                let model = if configured.is_empty()
                    || configured.contains("mistral")
                    || configured.eq_ignore_ascii_case("big-pickle")
                {
                    std::env::var("OLLAMA_POLISH_MODEL").unwrap_or_else(|_| "gemma-4".to_string())
                } else {
                    configured.to_string()
                };
                println!(
                    "✓ LLM polish: Ollama at {} (model: {model})",
                    config.ollama_base_url
                );
            }
            None => println!(
                "  LLM polish off — set OPENCODE_API_KEY or run Ollama (POLISH_PROVIDER=auto|opencode|ollama)"
            ),
        }
    }
    println!(
        "✓ Clipboard commands: auto when you copied text (≥12 chars) and speak an instruction (same shortcut)"
    );
    let key = config
        .shortcut_key_live
        .as_deref()
        .or(config.shortcut_key.as_deref())
        .unwrap_or(default_shortcut_key());
    println!("✓ Shortcut: {key} → dictate");
    println!(
        "✓ Dictionary: `dictate words` lists terms; `dictate words Hyprland` adds one ({})",
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
    if config.use_realtime_stt() {
        println!(
            "✓ Realtime WebSocket STT ({})",
            config.transcription_provider
        );
    }
    // Shortcut-spawned daemons run windowless, so this is the only record of why
    // one failed.
    println!(
        "  Daemon logs: {}",
        daemon_log_path()
            .parent()
            .map(|p| p.join("dictate-daemon.log").display().to_string())
            .unwrap_or_default()
    );

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
        "deepgram" => {
            if config
                .deepgram_api_key
                .as_ref()
                .is_some_and(|k| !k.is_empty())
            {
                println!(
                    "✓ DEEPGRAM_API_KEY is set (model: {})",
                    config.deepgram_model
                );
            } else {
                println!("✗ DEEPGRAM_API_KEY missing (required for Deepgram)");
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
    check_platform_dependencies(config);

    if let Err(e) = config.validate() {
        println!("\n⚠ Validation: {e}");
    } else {
        println!("\n✓ Configuration validates");
    }
}

/// Report on the helpers and permissions dictate needs from the desktop.
#[cfg(unix)]
fn check_platform_dependencies(_config: &Config) {
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
}

/// Windows needs no helper binaries — typing, clipboard, and shortcuts are all
/// in-process — so this checks that those in-process paths actually work.
#[cfg(windows)]
fn check_platform_dependencies(config: &Config) {
    println!("✓ Typing and clipboard are built in (no ydotool or wl-clipboard needed)");

    if let Some(key) = config
        .shortcut_key_live
        .as_deref()
        .or(config.shortcut_key.as_deref())
    {
        match platform::hotkeys::parse(key) {
            Ok(_) => println!("✓ SHORTCUT_KEY '{key}' is a usable Windows shortcut"),
            Err(e) => println!("✗ SHORTCUT_KEY '{key}': {e}"),
        }
    }

    match crate::audio::default_input_device_name() {
        Some(name) => println!("✓ Microphone: {name}"),
        None => println!("✗ No input device — check Settings → Privacy → Microphone"),
    }

    match platform::clipboard::get_text() {
        Ok(_) => println!("✓ Clipboard readable (command mode available)"),
        Err(e) => println!("✗ Clipboard unavailable: {e}"),
    }

    for slot in [Slot::Main, Slot::Live, Slot::Smart] {
        if control::is_running(slot) {
            println!("✓ {} daemon running", slot.as_str());
        }
    }

    println!("  Elevated windows ignore synthetic input — run dictate as admin to type into them");
}

// ─── Shortcut printing ───────────────────────────────────────────────────────

/// argv[0] for a spawned daemon.
///
/// Unix keeps the bare name so the `pgrep -f` patterns in [`crate::control`]
/// still match; Windows has no PATH guarantee and matches on a pipe instead.
fn daemon_program() -> String {
    #[cfg(windows)]
    if let Ok(exe) = std::env::current_exe() {
        return exe.display().to_string();
    }
    "dictate".to_string()
}

fn toggle_daemon_argv() -> Vec<String> {
    let mut argv = vec![daemon_program(), "--daemon".to_string()];
    if let Some(pipe) = platform::pipe_to_for_mode("type") {
        argv.push("--pipe-to".to_string());
        argv.extend(pipe);
    }
    argv
}

/// Where a shortcut-spawned daemon writes its output.
///
/// These are started windowless, so anything they print is otherwise lost — a
/// daemon that fails to reach its provider, or cannot register audio, would
/// simply appear to do nothing. Keeping the last run on disk makes that
/// diagnosable: `dictate doctor` prints the path.
pub fn daemon_log_path() -> PathBuf {
    get_default_config_path()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dictate-daemon.log")
}

fn daemon_log_target() -> Option<std::fs::File> {
    let path = daemon_log_path();
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            restrict_perms(parent);
        }
    }
    // Truncate per launch: the interesting failure is always the latest one.
    std::fs::File::create(path).ok()
}

fn spawn_daemon(argv: &[String]) -> Result<()> {
    let mut command = ProcessCommand::new(&argv[0]);
    command.args(&argv[1..]).stdin(std::process::Stdio::null());

    // One handle, cloned — opening the path twice would truncate the first.
    match daemon_log_target().and_then(|f| f.try_clone().ok().map(|c| (f, c))) {
        Some((out, err)) => {
            command.stdout(out).stderr(err);
        }
        None => {
            command
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// Keep a console window from flashing when a shortcut starts a daemon.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.spawn()?;
    Ok(())
}

fn dictation_slots() -> [Slot; 3] {
    [Slot::Live, Slot::Smart, Slot::Main]
}

/// Start the dictation daemon, or toggle recording if it already runs.
pub fn run_toggle_daemon(_kind: Option<ToggleKind>) -> Result<()> {
    for slot in dictation_slots() {
        if control::is_running(slot) {
            if !control::send(slot, "toggle")? {
                anyhow::bail!("the daemon stopped before it could be toggled");
            }
            return Ok(());
        }
    }

    let _ = control::send(Slot::Live, "shutdown");
    let _ = control::send(Slot::Smart, "shutdown");
    std::thread::sleep(std::time::Duration::from_millis(200));
    spawn_daemon(&toggle_daemon_argv())
}

// ─── Autostart: systemd user services (Linux) ────────────────────────────────

#[cfg(unix)]
const DAEMON_SERVICE: &str = "dictate.service";
#[cfg(unix)]
const LIVE_SERVICE: &str = "dictate-live.service";
#[cfg(unix)]
const SMART_SERVICE: &str = "dictate-smart.service";

#[cfg(unix)]
fn systemd_user_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("systemd/user")
}

#[cfg(unix)]
fn service_path(name: &str) -> PathBuf {
    systemd_user_dir().join(name)
}

#[cfg(unix)]
fn current_exe_for_service() -> Result<PathBuf> {
    std::env::current_exe().map_err(|e| anyhow!("Could not resolve current executable: {e}"))
}

#[cfg(unix)]
fn warm_daemon_command() -> Result<String> {
    let exe = current_exe_for_service()?;
    Ok(format!(
        "{} --daemon --idle-on-start --pipe-to ydotool type --file -",
        exe.display()
    ))
}

#[cfg(unix)]
fn service_contents() -> Result<String> {
    let command = warm_daemon_command()?;
    Ok(format!(
        "[Unit]\nDescription=Dictate warm dictation daemon\nAfter=graphical-session.target pipewire.service\n\n[Service]\nType=simple\nExecStart={command}\nRestart=on-failure\nRestartSec=2\n\n[Install]\nWantedBy=default.target\n",
    ))
}

#[cfg(unix)]
fn run_systemctl(args: &[&str]) -> Result<bool> {
    let status = ProcessCommand::new("systemctl")
        .arg("--user")
        .args(args)
        .status()?;
    Ok(status.success())
}

#[cfg(unix)]
fn service_active(name: &str) -> bool {
    ProcessCommand::new("systemctl")
        .args(["--user", "is-active", "--quiet", name])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn remove_legacy_mode_services() {
    let _ = run_systemctl(&["disable", "--now", LIVE_SERVICE, SMART_SERVICE]);
    for name in [LIVE_SERVICE, SMART_SERVICE] {
        let path = service_path(name);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(unix)]
fn install_autostart_services() -> Result<()> {
    let dir = systemd_user_dir();
    std::fs::create_dir_all(&dir)?;
    remove_legacy_mode_services();
    std::fs::write(service_path(DAEMON_SERVICE), service_contents()?)?;

    run_systemctl(&["daemon-reload"])?;
    let enabled = run_systemctl(&["enable", DAEMON_SERVICE])?;
    if !enabled {
        anyhow::bail!("systemctl --user enable failed");
    }
    let restarted = run_systemctl(&["restart", DAEMON_SERVICE])?;
    if !restarted {
        anyhow::bail!("systemctl --user restart failed");
    }

    println!("Warm Dictate daemon installed and started:");
    println!(
        "  {DAEMON_SERVICE}: {}",
        service_path(DAEMON_SERVICE).display()
    );
    println!("Shortcuts now signal a warm daemon instead of cold-starting it.");
    Ok(())
}

#[cfg(unix)]
fn remove_autostart_services() -> Result<()> {
    let _ = run_systemctl(&["disable", "--now", DAEMON_SERVICE]);
    remove_legacy_mode_services();
    let path = service_path(DAEMON_SERVICE);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    let _ = run_systemctl(&["daemon-reload"]);
    println!("Warm Dictate daemon removed.");
    Ok(())
}

/// Report whether dictate starts itself at login.
///
/// Linux keeps a warm dictation daemon as a systemd user service; Windows keeps a
/// single hotkey agent that starts the daemon on demand.
pub fn print_autostart_status() {
    #[cfg(windows)]
    platform::autostart::print_status();
    #[cfg(unix)]
    print_service_status();
}

pub fn run_autostart_command(command: &AutostartCommand) -> Result<()> {
    match command {
        AutostartCommand::Install => {
            #[cfg(windows)]
            return platform::autostart::install();
            #[cfg(unix)]
            install_autostart_services()
        }
        AutostartCommand::Remove => {
            #[cfg(windows)]
            return platform::autostart::remove();
            #[cfg(unix)]
            remove_autostart_services()
        }
        AutostartCommand::Status => {
            print_autostart_status();
            Ok(())
        }
    }
}

/// Hold the configured global shortcuts and toggle daemons on press.
///
/// `RegisterHotKey` delivers to the thread that registered, so the message pump
/// owns a dedicated thread while this task keeps serving the control pipe that
/// `dictate autostart remove` uses to stop the agent.
#[cfg(windows)]
pub async fn run_hotkey_agent(warm: bool, env_path: &Path) -> Result<()> {
    let mut control = crate::control::Control::start(Slot::Hotkeys)?;
    let key = shortcut_key_to_bind(env_path)?;
    let spec = platform::hotkeys::parse(&key).map_err(|e| anyhow!("shortcut '{key}': {e}"))?;
    let specs = vec![(0, spec, key.clone())];

    if warm {
        let already = dictation_slots()
            .iter()
            .any(|slot| control::is_running(*slot));
        if !already {
            let mut argv = toggle_daemon_argv();
            argv.insert(1, "--idle-on-start".to_string());
            if let Err(e) = spawn_daemon(&argv) {
                eprintln!("⚠ Could not pre-start the daemon: {e}");
            }
        }
    }

    println!("Dictate hotkey agent listening:");
    println!("  {key} → dictate");

    std::thread::spawn(move || {
        let result = platform::hotkeys::run_loop(&specs, |_id| {
            if let Err(e) = run_toggle_daemon(None) {
                eprintln!("⚠ toggle failed: {e}");
            }
        });
        if let Err(e) = result {
            eprintln!("⚠ Hotkey registration failed: {e}");
            eprintln!("  Windows reserves some combinations (Win+R opens Run).");
            eprintln!("  Pick another: dictate config set SHORTCUT_KEY_LIVE 'CTRL,ALT,R'");
            std::process::exit(1);
        }
    });

    // Exits when `dictate autostart remove` or Ctrl+C asks it to.
    while let Some(event) = control.recv().await {
        if event == crate::control::ControlEvent::Shutdown {
            break;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn print_service_status() {
    println!("Warm Dictate daemon:");
    let installed = service_path(DAEMON_SERVICE).exists();
    let active = service_active(DAEMON_SERVICE);
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
    println!("{marker} Dictation: {state} ({DAEMON_SERVICE})");
    if !service_active(DAEMON_SERVICE) {
        println!("  Run: dictate setup");
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

fn shortcut_key_to_bind(env_path: &Path) -> Result<String> {
    let path = env_path.to_path_buf();
    Ok(read_config_value(&path, "SHORTCUT_KEY_LIVE")?
        .or(read_config_value(&path, "SHORTCUT_KEY")?)
        .unwrap_or_else(|| default_shortcut_key().to_string()))
}

/// Write a GNOME custom-keybinding for `dictate`.
pub fn install_gnome_shortcuts(env_path: &Path) -> Result<()> {
    let schema = "org.gnome.settings-daemon.plugins.media-keys";
    let base = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";
    let dictate_path = format!("{base}/dictate/");
    let live_path = format!("{base}/dictate-live/");
    let smart_path = format!("{base}/dictate-smart/");
    let primary = shortcut_key_to_bind(env_path)?;

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
    if !paths.iter().any(|x| x == &dictate_path || x == &live_path) {
        paths.push(dictate_path.clone());
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
    let target = if paths.iter().any(|x| x == &live_path) {
        live_path.clone()
    } else {
        dictate_path.clone()
    };
    let full = format!("{bind_schema}{target}");
    for (key, value) in [
        ("name", "Dictate"),
        ("command", "dictate"),
        ("binding", &gnome_binding(&primary)),
    ] {
        let status = ProcessCommand::new("gsettings")
            .args(["set", &full, key, value])
            .status()?;
        if !status.success() {
            anyhow::bail!("gsettings failed to set shortcut {key}");
        }
    }
    if paths.iter().any(|x| x == &smart_path) {
        let smart_full = format!("{bind_schema}{smart_path}");
        let _ = ProcessCommand::new("gsettings")
            .args(["set", &smart_full, "command", "dictate"])
            .status();
    }
    println!("GNOME shortcut installed: {primary} → dictate");
    Ok(())
}

pub fn print_shortcut(args: &ShortcutArgs, env_path: &Path) {
    if args.install {
        match args.desktop {
            ShortcutDesktop::Gnome => {
                if let Err(e) = install_gnome_shortcuts(env_path) {
                    eprintln!("Failed to install GNOME shortcuts: {e}");
                }
            }
            // Windows shortcuts are held by dictate's own agent, so installing
            // them is the same thing as installing autostart.
            ShortcutDesktop::Windows => {
                if let Err(e) = run_autostart_command(&AutostartCommand::Install) {
                    eprintln!("Failed to install the hotkey agent: {e}");
                }
            }
            _ => eprintln!("--install is only supported for gnome and windows"),
        }
        return;
    }
    println!("Bind this command to your shortcut:");
    println!("dictate");
    match args.desktop {
        ShortcutDesktop::Gnome => {
            println!("# Or: dictate setup (installs the GNOME binding)");
        }
        ShortcutDesktop::Windows => {
            println!("# Windows: dictate setup  (installs the hotkey agent at login)");
            println!("# Reserves Win+R — prefer CTRL,ALT,R");
        }
        ShortcutDesktop::Hyprland => {
            let key = args.key.replace(',', ", ");
            println!("# Hyprland: bind = {key}, exec, dictate");
        }
        ShortcutDesktop::Niri => {
            let key = args.key.replace(',', "+");
            println!("# Niri: {key} {{ spawn \"dictate\"; }}");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_argv_carries_the_platform_output_sink() {
        let argv = toggle_daemon_argv();
        assert!(argv[1..].starts_with(&["--daemon".to_string(), "--pipe-to".to_string()]));
        assert_eq!(
            argv[3..].to_vec(),
            platform::pipe_to_for_mode("type").unwrap()
        );
    }

    #[test]
    fn editor_argv_splits_program_and_args() {
        assert_eq!(
            editor_argv("code --wait", "vi"),
            ("code".to_string(), vec!["--wait".to_string()])
        );
        assert_eq!(
            editor_argv("  ", "vi"),
            ("vi".to_string(), Vec::<String>::new())
        );
        assert_eq!(
            editor_argv("", "notepad"),
            ("notepad".to_string(), Vec::<String>::new())
        );
    }
}
