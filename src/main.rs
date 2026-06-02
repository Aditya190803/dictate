#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::float_cmp)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::needless_continue)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::single_match_else)]
#![allow(clippy::match_bool)]

use anyhow::{anyhow, Result};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use std::{io::Write, path::PathBuf, process::Command as ProcessCommand};

use std::time::Instant;
use tokio::io::AsyncWriteExt;

use futures::stream::StreamExt;
#[cfg(not(test))]
use signal_hook::consts::{SIGTERM, SIGUSR1};
#[cfg(not(test))]
use signal_hook_tokio::Signals;

mod audio;
mod audio_processing;
mod beep;
mod command;
mod config;
mod streaming;
mod transcription;
mod wav;

#[cfg(test)]
mod test_utils;
#[cfg(not(test))]
use audio::AudioRecorder;
use audio_processing::AudioProcessor;
use beep::{BeepConfig, BeepPlayer, BeepType};
use config::Config;
use transcription::{TranscriptionError, TranscriptionFactory};
use wav::WavEncoder;

#[derive(Parser)]
#[command(name = "dictate")]
#[command(about = "Wayland Speech-to-Text Tool - Signal-driven transcription")]
#[command(version)]
struct Args {
    /// Path to environment file
    #[arg(long)]
    envfile: Option<PathBuf>,

    /// Pipe transcribed text to the specified command
    #[arg(long, short = 'p', num_args = 1.., value_name = "COMMAND", allow_hyphen_values = true, trailing_var_arg = true)]
    pipe_to: Option<Vec<String>>,

    /// Download the configured local model and exit
    #[arg(long)]
    download_model: bool,

    /// Stream mode: continuously transcribe speech with VAD (type as you speak)
    #[arg(long)]
    stream: bool,

    /// Daemon mode: keep running with model loaded in memory,
    /// handle multiple SIGUSR1 recordings without reloading
    #[arg(long)]
    daemon: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Read, write, or interactively create configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Print compositor shortcut snippets
    Shortcuts(ShortcutArgs),
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum ConfigCommand {
    /// Interactively create or update the config file. Pass flags to run non-interactively.
    Wizard(WizardOptions),
    /// Print the current config file, or one config key
    Get { key: Option<String> },
    /// Set one config key in the config file
    Set { key: String, value: String },
    /// Open the config file in $EDITOR
    Edit,
}

#[derive(ClapArgs, Default)]
struct WizardOptions {
    /// Provider: mistral, groq, or local
    #[arg(long)]
    provider: Option<String>,

    /// Mistral API key
    #[arg(long)]
    mistral_api_key: Option<String>,

    /// Groq API key
    #[arg(long)]
    groq_api_key: Option<String>,

    /// Mistral batch/offline model
    #[arg(long)]
    mistral_model: Option<String>,

    /// Mistral realtime WebSocket model
    #[arg(long)]
    mistral_realtime_model: Option<String>,

    /// Transcription mode: auto, realtime, or batch
    #[arg(long)]
    transcription_mode: Option<String>,

    /// Batch mode: true uses whole-clip transcription, false uses realtime where supported
    #[arg(long)]
    batch_mode: Option<String>,

    /// Groq model
    #[arg(long)]
    groq_model: Option<String>,

    /// Local Whisper model filename
    #[arg(long)]
    whisper_model: Option<String>,

    /// Language: auto or ISO code like en, es, fr
    #[arg(long)]
    language: Option<String>,

    /// Output mode: type, clipboard, or stdout
    #[arg(long)]
    output_mode: Option<String>,

    /// Desktop/compositor: hyprland, niri, gnome, kde, sway, or other
    #[arg(long)]
    desktop: Option<String>,

    /// Shortcut key label/config value, e.g. SUPER,R, Mod,R, or <Super>r
    #[arg(long)]
    shortcut_key: Option<String>,

    /// Enable audio feedback beeps: true or false
    #[arg(long)]
    audio_feedback: Option<String>,

    /// Beep volume from 0.0 to 1.0
    #[arg(long)]
    beep_volume: Option<String>,
}

#[derive(Parser)]
struct ShortcutArgs {
    /// Desktop/compositor to generate for
    #[arg(value_enum)]
    desktop: ShortcutDesktop,

    /// Output behavior for the shortcut
    #[arg(long, value_enum, default_value_t = ShortcutMode::Type)]
    mode: ShortcutMode,

    /// Shortcut key label to include in comments/config examples
    #[arg(long, default_value = "SUPER,R")]
    key: String,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShortcutDesktop {
    Hyprland,
    Niri,
    Gnome,
    Kde,
    Sway,
    Other,
}

#[derive(Clone, Copy, ValueEnum)]
enum ShortcutMode {
    Stdout,
    Clipboard,
    Type,
}

fn get_default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("dictate")
        .join(".env")
}

fn ensure_config_file(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !path.exists() {
        std::fs::write(
            path,
            "TRANSCRIPTION_PROVIDER=mistral\nBATCH_MODE=false\nTRANSCRIPTION_MODE=auto\nMISTRAL_MODEL=voxtral-mini-latest\nMISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602\nMISTRAL_REALTIME_DELAY_MS=480\nGROQ_MODEL=whisper-large-v3-turbo\nTRANSCRIPTION_LANGUAGE=auto\nTRANSCRIPTION_TIMEOUT_SECONDS=60\nTRANSCRIPTION_MAX_RETRIES=3\nENABLE_AUDIO_FEEDBACK=true\nBEEP_VOLUME=0.1\n",
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
        "batch_mode" | "batch" => "BATCH_MODE",
        "transcription_mode" | "stt_mode" => "TRANSCRIPTION_MODE",
        "mistral_base_url" => "MISTRAL_BASE_URL",
        "groq_key" | "groq_api_key" => "GROQ_API_KEY",
        "groq_model" => "GROQ_MODEL",
        "groq_base_url" => "GROQ_BASE_URL",
        "local_model" | "whisper_model" => "WHISPER_MODEL",
        "audio_feedback" | "enable_audio_feedback" => "ENABLE_AUDIO_FEEDBACK",
        "beep_volume" => "BEEP_VOLUME",
        "shortcut" | "shortcut_key" => "SHORTCUT_KEY",
        "desktop" | "shortcut_desktop" => "SHORTCUT_DESKTOP",
        "mode" | "output_mode" => "OUTPUT_MODE",
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
    let mut lines = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            if let Some((line_key, _)) = line.split_once('=') {
                if line_key.trim() == key {
                    lines.push(format!("{}={}", key, value));
                    found = true;
                    continue;
                }
            }
        }
        lines.push(line.to_string());
    }

    if !found {
        lines.push(format!("{}={}", key, value));
    }

    std::fs::write(path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

fn prompt(label: &str, default: Option<&str>) -> Result<String> {
    match default {
        Some(default) if !default.is_empty() => print!("{} [{}]: ", label, default),
        _ => print!("{}: ", label),
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
        Some(value) => Ok(value.clone()),
        None => prompt(message, default),
    }
}

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

    match provider.as_str() {
        "mistral" => {
            let key = option_or_prompt(&options.mistral_api_key, "Mistral API key", None)?;
            if !key.is_empty() {
                set_config_value(path, "mistral-api-key", &key)?;
            }
            let batch_mode = option_or_prompt(
                &options.batch_mode,
                "Batch mode: whole-clip transcription instead of realtime (true/false)",
                Some("false"),
            )?;
            set_config_value(path, "batch-mode", &batch_mode)?;
            let mode = option_or_prompt(
                &options.transcription_mode,
                "Legacy transcription mode (auto/realtime/batch)",
                Some("auto"),
            )?;
            set_config_value(path, "transcription-mode", &mode)?;
            let model = option_or_prompt(
                &options.mistral_model,
                "Mistral batch model",
                Some("voxtral-mini-latest"),
            )?;
            set_config_value(path, "mistral-model", &model)?;
            let realtime_model = option_or_prompt(
                &options.mistral_realtime_model,
                "Mistral realtime model",
                Some("voxtral-mini-transcribe-realtime-2602"),
            )?;
            set_config_value(path, "mistral-realtime-model", &realtime_model)?;
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
            return Err(anyhow!(
                "Unsupported provider '{}'. Use mistral, groq, or local.",
                provider
            ));
        }
    }

    let language = option_or_prompt(
        &options.language,
        "Language (auto or ISO code like en)",
        Some("auto"),
    )?;
    set_config_value(path, "language", &language)?;

    let output_mode = option_or_prompt(
        &options.output_mode,
        "Default shortcut output (type/clipboard/stdout)",
        Some("type"),
    )?;
    set_config_value(path, "output-mode", &output_mode)?;

    let desktop = option_or_prompt(
        &options.desktop,
        "Desktop environment (hyprland/niri/gnome/kde/sway/other)",
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

    let audio_feedback = option_or_prompt(
        &options.audio_feedback,
        "Audio feedback beeps (true/false)",
        Some("true"),
    )?;
    set_config_value(path, "audio-feedback", &audio_feedback)?;

    if audio_feedback.trim().eq_ignore_ascii_case("true") || options.beep_volume.is_some() {
        let beep_volume = option_or_prompt(
            &options.beep_volume,
            "Beep volume (0.0 to 1.0)",
            Some("0.1"),
        )?;
        set_config_value(path, "beep-volume", &beep_volume)?;
    }

    println!("\nSaved config to {}", path.display());
    println!(
        "Run `dictate shortcuts {} --mode {} --key {}` to print a shortcut snippet.",
        desktop, output_mode, shortcut
    );
    Ok(())
}

fn run_config_command(command: &ConfigCommand, path: &PathBuf) -> Result<()> {
    match command {
        ConfigCommand::Wizard(options) => run_config_wizard(path, options),
        ConfigCommand::Get { key } => {
            if let Some(key) = key {
                match read_config_value(path, key)? {
                    Some(value) => println!("{}", value),
                    None => return Err(anyhow!("Config key '{}' is not set", key)),
                }
            } else if path.exists() {
                print!("{}", std::fs::read_to_string(path)?);
            } else {
                return Err(anyhow!("Config file does not exist: {}", path.display()));
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
                Err(anyhow!("Editor exited with status {}", status))
            }
        }
    }
}

fn shortcut_command(mode: &ShortcutMode) -> &'static str {
    match mode {
        ShortcutMode::Stdout => "dictate",
        ShortcutMode::Clipboard => "dictate --pipe-to wl-copy",
        ShortcutMode::Type => "dictate --pipe-to ydotool type --file -",
    }
}

fn print_shortcut(args: &ShortcutArgs) {
    let command = shortcut_command(&args.mode);
    let shell = format!(
        "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || ({} &)",
        command
    );

    match args.desktop {
        ShortcutDesktop::Hyprland => {
            let key = args.key.replace(',', ", ");
            println!("# Dictate ({})", args.mode_name());
            println!("bind = {}, exec, {}", key, shell);
            println!("# Clipboard variant:");
            println!("# bind = {} SHIFT, {}, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)", 
                args.key.split(',').next().unwrap_or("SUPER"),
                args.key.split(',').nth(1).unwrap_or("R"));
        }
        ShortcutDesktop::Niri => {
            let key = args.key.replace(',', "+");
            println!("// Dictate ({})", args.mode_name());
            println!("{} {{ spawn \"sh\" \"-c\" \"{}\"; }}", key, shell);
            println!("// Clipboard variant:");
            let mod_key = args.key.split(',').next().unwrap_or("Mod");
            let char_key = args.key.split(',').nth(1).unwrap_or("R");
            println!("// Shift+{}+{} {{ spawn \"sh\" \"-c\" \"pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)\"; }}", mod_key, char_key);
        }
        ShortcutDesktop::Gnome => {
            println!("# GNOME Custom Shortcut");
            println!("# 1. Open Settings → Keyboard → Keyboard Shortcuts");
            println!("# 2. Scroll to bottom, click +");
            println!("# 3. Name: Dictate ({})", args.mode_name());
            println!("#    Command: sh -c '{}'", shell);
            println!("#    Shortcut: {}", args.key);
            println!("#");
            println!("# Run this to set it programmatically:");
            println!("# gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/dictate/ name 'Dictate ({})'", args.mode_name());
            println!("# gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/dictate/ binding '{}'", args.key);
            println!("# gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/dictate/ command 'sh -c \"{}\"'", shell);
        }
        ShortcutDesktop::Kde | ShortcutDesktop::Sway => {
            println!(
                "# {} Custom Shortcut",
                match args.desktop {
                    ShortcutDesktop::Kde => "KDE",
                    _ => "Sway",
                }
            );
            println!("# Add this command as a custom shortcut:");
            println!("{}", shell);
        }
        ShortcutDesktop::Other => {
            println!("# Generic Custom Shortcut");
            println!("# Add this command as a custom shortcut in your desktop settings:");
            println!("{}", shell);
        }
    }
}

impl ShortcutArgs {
    fn mode_name(&self) -> &'static str {
        match self.mode {
            ShortcutMode::Stdout => "stdout",
            ShortcutMode::Clipboard => "clipboard",
            ShortcutMode::Type => "direct typing",
        }
    }
}

async fn download_model(model: &str) -> Result<PathBuf> {
    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    let url = format!("{}/{}", base_url, model);
    let dir = Config::model_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(model);

    let resp = reqwest::get(&url).await.map_err(|e| anyhow!("{}", e))?;
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to download model: {}", resp.status()));
    }

    let total_size = resp.content_length();
    let mut file = tokio::fs::File::create(&path).await?;
    let mut stream = resp.bytes_stream();

    let mut downloaded = 0u64;
    let start_time = Instant::now();

    print!("{}... ", model);
    std::io::stdout().flush().unwrap();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Download error: {}", e))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total_size {
            let percentage = (downloaded as f64 / total as f64) * 100.0;
            let elapsed = start_time.elapsed().as_secs_f64();

            if elapsed > 0.0 {
                let speed = downloaded as f64 / elapsed / 1024.0 / 1024.0;
                let eta = if speed > 0.0 {
                    (total - downloaded) as f64 / (speed * 1024.0 * 1024.0)
                } else {
                    0.0
                };

                print!(
                    "\r{}... {:.1}% ({:.1} MB/s, ETA: {:.0}s)    ",
                    model, percentage, speed, eta
                );
                std::io::stdout().flush().unwrap();
            }
        } else {
            print!(
                "\r{}... {:.1} MB downloaded    ",
                model,
                downloaded as f64 / 1024.0 / 1024.0
            );
            std::io::stdout().flush().unwrap();
        }
    }

    file.flush().await?;
    Ok(path)
}

async fn process_audio_for_transcription(
    audio_data: Vec<f32>,
    sample_rate: u32,
    config: &Config,
    pipe_command: Option<&Vec<String>>,
) -> Result<i32> {
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;
    eprintln!("Processing audio: {} samples", audio_data.len());

    let processor = AudioProcessor::new(sample_rate);

    match processor.process_for_speech_recognition(&audio_data) {
        Ok(processed_audio) => {
            let original_duration = processor.get_duration_seconds(&audio_data);
            let processed_duration = processor.get_duration_seconds(&processed_audio);

            eprintln!(
                "Audio processed successfully: {:.2}s -> {:.2}s ({} samples)",
                original_duration,
                processed_duration,
                processed_audio.len()
            );

            let encoder = WavEncoder::new(sample_rate, 1);
            match encoder.encode_to_wav(&processed_audio) {
                Ok(wav_data) => {
                    eprintln!(
                        "WAV encoded: {} bytes ready for transcription",
                        wav_data.len()
                    );

                    let provider =
                        TranscriptionFactory::create_provider(&config.transcription_provider)
                            .await?;

                    eprintln!(
                        "Sending audio to {} provider...",
                        config.transcription_provider
                    );
                    let language = if config.transcription_language == "auto" {
                        None
                    } else {
                        Some(config.transcription_language.clone())
                    };
                    match provider.transcribe_with_language(wav_data, language).await {
                        Ok(transcribed_text) => {
                            if transcribed_text.trim().is_empty() {
                                eprintln!("Warning: Received empty transcription from provider");
                                eprintln!("This might indicate silent audio or unclear speech");

                                let exit_code = if let Some(cmd) = pipe_command {
                                    match command::execute_with_input(cmd, "").await {
                                        Ok(exit_code) => exit_code,
                                        Err(e) => {
                                            eprintln!("Failed to execute pipe command: {}", e);
                                            if let Err(beep_err) =
                                                beep_player.play_async(BeepType::Error).await
                                            {
                                                eprintln!(
                                                    "Warning: Failed to play error beep: {}",
                                                    beep_err
                                                );
                                            }
                                            1
                                        }
                                    }
                                } else {
                                    println!("{}", transcribed_text);
                                    0
                                };

                                if let Err(e) = beep_player.play_async(BeepType::Success).await {
                                    eprintln!("Warning: Failed to play success beep: {}", e);
                                }

                                return Ok(exit_code);
                            }

                            eprintln!("Transcription successful: \"{}\"", transcribed_text);

                            let exit_code = if let Some(cmd) = pipe_command {
                                match command::execute_with_input(cmd, &transcribed_text).await {
                                    Ok(exit_code) => exit_code,
                                    Err(e) => {
                                        eprintln!("Failed to execute pipe command: {}", e);
                                        if let Err(beep_err) =
                                            beep_player.play_async(BeepType::Error).await
                                        {
                                            eprintln!(
                                                "Warning: Failed to play error beep: {}",
                                                beep_err
                                            );
                                        }
                                        return Ok(1);
                                    }
                                }
                            } else {
                                println!("{}", transcribed_text);
                                0
                            };

                            if let Err(e) = beep_player.play_async(BeepType::Success).await {
                                eprintln!("Warning: Failed to play success beep: {}", e);
                            }

                            Ok(exit_code)
                        }
                        Err(e) => {
                            eprintln!("❌ Transcription failed: {}", e);

                            if let Err(beep_err) = beep_player.play_async(BeepType::Error).await {
                                eprintln!("Warning: Failed to play error beep: {}", beep_err);
                            }

                            match &e {
                                TranscriptionError::AuthenticationFailed { provider, details } => {
                                    if let Some(details) = details {
                                        eprintln!("🔑 Authentication details: {}", details);
                                    }
                                    eprintln!("💡 Check your {} API key configuration", provider);
                                }
                                TranscriptionError::NetworkError(details) => {
                                    eprintln!(
                                        "🌐 Network details: {} - {}",
                                        details.error_type, details.error_message
                                    );
                                }
                                TranscriptionError::ApiError(details) => {
                                    if let Some(status) = details.status_code {
                                        eprintln!("📡 API Response: HTTP {}", status);
                                    }
                                    if let Some(code) = &details.error_code {
                                        eprintln!("🏷️  Error Code: {}", code);
                                    }
                                }
                                TranscriptionError::FileTooLarge(size) => {
                                    eprintln!("💡 Audio file too large: {} bytes (max 25MB)", size);
                                }
                                TranscriptionError::ConfigurationError(_) => {
                                    eprintln!("💡 Check your transcription provider configuration");
                                }
                                TranscriptionError::UnsupportedProvider(provider) => {
                                    eprintln!("💡 Unsupported provider: {}", provider);
                                }
                                _ => {}
                            }

                            Ok(1)
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to encode WAV: {}", e);
                    Ok(1)
                }
            }
        }
        Err(e) => {
            eprintln!("Audio processing failed: {}", e);

            if let Err(beep_err) = beep_player.play_async(BeepType::Error).await {
                eprintln!("Warning: Failed to play error beep: {}", beep_err);
            }

            if e.to_string().contains("too short") {
                eprintln!("Tip: Try speaking for at least 0.1 seconds before sending signal");
            } else if e.to_string().contains("only silence") {
                eprintln!("Tip: Make sure your microphone is working and you're speaking clearly");
            }

            Ok(1)
        }
    }
}

/// Clip mode: the original dictate behavior — record on start, transcribe on SIGUSR1, then exit
#[cfg(not(test))]
async fn run_clip_mode(config: &Config, args: &Args) -> Result<()> {
    eprintln!("dictate - Wayland Speech-to-Text Tool");
    eprintln!("Starting audio recording...");

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    let mut recorder = AudioRecorder::new()?;

    if let Err(e) = beep_player.play_async(BeepType::RecordingStart).await {
        eprintln!("Warning: Failed to play recording start beep: {}", e);
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

    if let Err(e) = recorder.start_recording() {
        eprintln!("Failed to start audio recording: {}", e);
        eprintln!("This may be due to PipeWire not being available or insufficient permissions.");
        return Err(e);
    }

    eprintln!("Audio recording started successfully!");
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    eprintln!("Ready. Send SIGUSR1 to transcribe and output to stdout.");

    let mut signals = Signals::new([SIGUSR1, SIGTERM])?;

    loop {
        match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await {
            Ok(Some(signal)) => match signal {
                SIGUSR1 => {
                    eprintln!("Received SIGUSR1: Stop recording, transcribe, and output");

                    if let Err(e) = recorder.stop_recording() {
                        eprintln!("Failed to stop recording: {}", e);
                    } else {
                        if let Err(e) = beep_player.play_async(BeepType::RecordingStop).await {
                            eprintln!("Warning: Failed to play recording stop beep: {}", e);
                        }
                    }

                    match recorder.get_audio_data() {
                        Ok(audio_data) => {
                            let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                            eprintln!(
                                "Captured {} audio samples ({:.2} seconds)",
                                audio_data.len(),
                                duration
                            );

                            match process_audio_for_transcription(
                                audio_data,
                                16000,
                                config,
                                args.pipe_to.as_ref(),
                            )
                            .await
                            {
                                Ok(exit_code) => {
                                    eprintln!(
                                        "Audio processing completed with exit code: {}",
                                        exit_code
                                    );

                                    if let Err(e) = recorder.clear_buffer() {
                                        eprintln!("Failed to clear audio buffer: {}", e);
                                    }

                                    std::process::exit(exit_code);
                                }
                                Err(e) => {
                                    eprintln!("Audio processing failed: {}", e);

                                    if let Err(e) = recorder.clear_buffer() {
                                        eprintln!("Failed to clear audio buffer: {}", e);
                                    }

                                    std::process::exit(1);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to get audio data: {}", e);
                        }
                    }

                    break;
                }
                SIGTERM => {
                    eprintln!("Received SIGTERM: Shutting down gracefully");
                    if let Err(e) = recorder.stop_recording() {
                        eprintln!("Failed to stop recording: {}", e);
                    }
                    if let Err(e) = recorder.clear_buffer() {
                        eprintln!("Failed to clear audio buffer during shutdown: {}", e);
                    }
                    break;
                }
                _ => {
                    eprintln!("Received unexpected signal: {}", signal);
                }
            },
            Ok(None) => {
                break;
            }
            Err(_) => {
                if let Err(e) = recorder.process_audio_events() {
                    eprintln!("Error processing audio events: {}", e);
                }
                continue;
            }
        }
    }

    eprintln!("Exiting dictate");
    Ok(())
}

/// Daemon clip mode: keep model loaded, loop recording/transcription on SIGUSR1
#[cfg(not(test))]
async fn run_daemon_clip_mode(config: &Config, args: &Args) -> Result<()> {
    eprintln!("🔄 dictate daemon mode — model stays loaded, ready for multiple recordings");
    eprintln!("   Send SIGUSR1 to start/stop recording, SIGTERM to quit");

    // Load provider once and keep it in memory
    eprintln!("📦 Loading transcription provider...");
    let provider = TranscriptionFactory::create_provider(&config.transcription_provider).await?;
    let provider = std::sync::Arc::new(tokio::sync::Mutex::new(provider));
    eprintln!("✅ Provider ready");

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    let mut recorder = AudioRecorder::new()?;
    let mut signals = Signals::new([SIGUSR1, SIGTERM])?;
    let mut is_recording = false;

    loop {
        match tokio::time::timeout(tokio::time::Duration::from_millis(50), signals.next()).await {
            Ok(Some(signal)) => {
                match signal {
                    SIGUSR1 => {
                        if !is_recording {
                            // Start recording
                            eprintln!("\n▶️  Recording started");
                            if let Err(e) = beep_player.play_async(BeepType::RecordingStart).await {
                                eprintln!("Warning: Failed to play start beep: {}", e);
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                            if let Err(e) = recorder.start_recording() {
                                eprintln!("❌ Failed to start recording: {}", e);
                            } else {
                                is_recording = true;
                                eprintln!("🎤 Speak now... (send SIGUSR1 to stop)");
                            }
                        } else {
                            // Stop recording and transcribe
                            eprintln!("⏹️  Stopping recording...");
                            is_recording = false;

                            if let Err(e) = recorder.stop_recording() {
                                eprintln!("❌ Failed to stop recording: {}", e);
                                continue;
                            }

                            if let Err(e) = beep_player.play_async(BeepType::RecordingStop).await {
                                eprintln!("Warning: Failed to play stop beep: {}", e);
                            }

                            match recorder.get_audio_data() {
                                Ok(audio_data) => {
                                    let duration =
                                        recorder.get_recording_duration_seconds().unwrap_or(0.0);
                                    eprintln!(
                                        "Captured {} samples ({:.2}s)",
                                        audio_data.len(),
                                        duration
                                    );

                                    // Process and transcribe using the pre-loaded provider
                                    let exit_code = match process_clip_with_provider(
                                        audio_data,
                                        16000,
                                        config,
                                        args.pipe_to.as_ref(),
                                        &beep_player,
                                        std::sync::Arc::clone(&provider),
                                    )
                                    .await
                                    {
                                        Ok(code) => code,
                                        Err(e) => {
                                            eprintln!("❌ Processing failed: {}", e);
                                            1
                                        }
                                    };

                                    if let Err(e) = recorder.clear_buffer() {
                                        eprintln!("Failed to clear buffer: {}", e);
                                    }

                                    eprintln!(
                                        "✅ Done (exit code: {}). Ready for next recording.",
                                        exit_code
                                    );
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to get audio data: {}", e);
                                }
                            }
                        }
                    }
                    SIGTERM => {
                        eprintln!("\n🛑 Received SIGTERM: Shutting down daemon");
                        if is_recording {
                            let _ = recorder.stop_recording();
                        }
                        let _ = recorder.clear_buffer();
                        break;
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(_) => {
                if is_recording {
                    if let Err(e) = recorder.process_audio_events() {
                        eprintln!("Error processing audio: {}", e);
                    }
                }
            }
        }
    }

    eprintln!("👋 Daemon exiting");
    Ok(())
}

/// Process a clip using a pre-loaded provider (for daemon mode)
#[cfg(not(test))]
async fn process_clip_with_provider(
    audio_data: Vec<f32>,
    sample_rate: u32,
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    beep_player: &BeepPlayer,
    provider: std::sync::Arc<tokio::sync::Mutex<Box<dyn transcription::TranscriptionProvider>>>,
) -> Result<i32> {
    let processor = AudioProcessor::new(sample_rate);

    let processed_audio = match processor.process_for_speech_recognition(&audio_data) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Audio processing failed: {}", e);
            beep_player.play_async(BeepType::Error).await.ok();
            return Ok(1);
        }
    };

    let encoder = WavEncoder::new(sample_rate, 1);
    let wav_data = match encoder.encode_to_wav(&processed_audio) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("WAV encoding failed: {}", e);
            return Ok(1);
        }
    };

    eprintln!(
        "WAV encoded: {} bytes — transcribing with loaded model...",
        wav_data.len()
    );

    let language = if config.transcription_language == "auto" {
        None
    } else {
        Some(config.transcription_language.clone())
    };

    let guard = provider.lock().await;
    let result = match guard.transcribe_with_language(wav_data, language).await {
        Ok(text) => {
            if text.trim().is_empty() {
                eprintln!("⚠️  Empty transcription");
                0
            } else {
                eprintln!("📝 {}", text.trim());

                if let Some(cmd) = pipe_command {
                    match command::execute_with_input(cmd, &text).await {
                        Ok(code) => code,
                        Err(e) => {
                            eprintln!("Pipe command failed: {}", e);
                            1
                        }
                    }
                } else {
                    println!("{}", text);
                    0
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Transcription failed: {}", e);
            1
        }
    };
    drop(guard);

    if result == 0 {
        beep_player.play_async(BeepType::Success).await.ok();
    } else {
        beep_player.play_async(BeepType::Error).await.ok();
    }

    Ok(result)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let envfile = args.envfile.clone().unwrap_or_else(get_default_config_path);

    if let Some(command) = &args.command {
        match command {
            Commands::Config { command } => run_config_command(command, &envfile)?,
            Commands::Shortcuts(shortcut_args) => print_shortcut(shortcut_args),
        }
        return Ok(());
    }

    let config = if envfile.exists() {
        eprintln!("Loading environment from: {}", envfile.display());
        match Config::load_env_file(&envfile) {
            Ok(config) => config,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to load environment file {}: {}",
                    envfile.display(),
                    e
                );
                Config::from_env()
            }
        }
    } else {
        eprintln!(
            "Environment file {} not found, using system environment",
            envfile.display()
        );
        Config::from_env()
    };

    if args.download_model {
        match download_model(&config.whisper_model).await {
            Ok(path) => {
                eprintln!("Model downloaded to {}", path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("Failed to download model: {}", e);
                std::process::exit(1);
            }
        }
    }

    if let Err(e) = config.validate() {
        eprintln!("Configuration warning: {}", e);
        if config.transcription_provider == "local" {
            std::process::exit(1);
        }
    }

    // Mode selection. BATCH_MODE=false is the default: Mistral uses realtime STT
    // from the normal keyboard shortcut. BATCH_MODE=true opts into whole-clip batch.
    let default_realtime = config
        .transcription_provider
        .eq_ignore_ascii_case("mistral")
        && !config.batch_mode
        && !config.transcription_mode.eq_ignore_ascii_case("batch");

    if args.daemon && default_realtime {
        let (_control_tx, mut control_rx) = tokio::sync::mpsc::channel(8);

        #[cfg(not(test))]
        {
            let control_tx = _control_tx;
            tokio::spawn(async move {
                let mut signals = Signals::new([SIGUSR1]).unwrap();
                while signals.next().await.is_some() {
                    let _ = control_tx.send(()).await;
                }
            });
        }

        streaming::run_mistral_realtime_daemon(&config, args.pipe_to.as_ref(), &mut control_rx)
            .await?;
    } else if (args.stream && !config.batch_mode) || default_realtime {
        let (_shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);

        // Set up signal handler for graceful shutdown. SIGUSR1 keeps the existing
        // keyboard shortcut toggle working; SIGTERM remains available for process managers.
        #[cfg(not(test))]
        {
            let shutdown_tx = _shutdown_tx;
            tokio::spawn(async move {
                let mut signals = Signals::new([SIGUSR1, SIGTERM]).unwrap();
                if signals.next().await.is_some() {
                    let _ = shutdown_tx.send(()).await;
                }
            });
        }

        streaming::run_stream(&config, args.pipe_to.as_ref(), &mut shutdown_rx).await?;
    } else if args.daemon {
        // Daemon clip mode: keep model loaded, handle multiple recordings
        #[cfg(not(test))]
        {
            run_daemon_clip_mode(&config, &args).await?;
        }
        #[cfg(test)]
        {
            eprintln!("Daemon mode not available in tests");
        }
    } else {
        // Default clip mode: original behavior
        #[cfg(not(test))]
        {
            run_clip_mode(&config, &args).await?;
        }
        #[cfg(test)]
        {
            eprintln!("Test mode: Signal handling disabled");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_processing::AudioProcessor;
    use wav::WavEncoder;

    #[tokio::test]
    async fn test_audio_processing_pipeline_integration() {
        let sample_rate = 16000u32;
        let window_size = (sample_rate as f32 * 0.01) as usize;

        let mut test_audio = vec![0.0; window_size];
        test_audio.extend(vec![0.2; window_size * 20]);
        test_audio.extend(vec![0.0; window_size]);

        let processor = AudioProcessor::new(sample_rate);
        let processed = processor.process_for_speech_recognition(&test_audio);
        assert!(processed.is_ok(), "Audio processing should succeed");

        let encoder = WavEncoder::new(sample_rate, 1);
        let wav_result = encoder.encode_to_wav(&processed.unwrap());
        assert!(
            wav_result.is_ok(),
            "WAV encoding should succeed with valid audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_empty_audio() {
        let test_config = Config::default();
        let result = process_audio_for_transcription(vec![], 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with empty audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_too_short() {
        let short_audio = vec![0.5; 160];

        let test_config = Config::default();
        let result = process_audio_for_transcription(short_audio, 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with too short audio"
        );
    }

    #[tokio::test]
    async fn test_audio_processing_pipeline_only_silence() {
        let silent_audio = vec![0.0; 1600];

        let test_config = Config::default();
        let result = process_audio_for_transcription(silent_audio, 16000, &test_config, None).await;

        assert!(
            result.is_ok() && result.unwrap() == 1,
            "Audio processing should return exit code 1 with only silence"
        );
    }

    #[test]
    fn test_wav_encoder_whisper_compatibility() {
        let encoder = WavEncoder::default();
        let test_samples = vec![0.1, 0.2, -0.1, -0.2];

        let wav_data = encoder.encode_to_wav(&test_samples).unwrap();

        assert!(wav_data.len() > 44, "WAV should have header + data");
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");

        let sample_rate =
            u32::from_le_bytes([wav_data[24], wav_data[25], wav_data[26], wav_data[27]]);
        assert_eq!(sample_rate, 16000);

        let channels = u16::from_le_bytes([wav_data[22], wav_data[23]]);
        assert_eq!(channels, 1);

        let bits_per_sample = u16::from_le_bytes([wav_data[34], wav_data[35]]);
        assert_eq!(bits_per_sample, 16);
    }

    #[test]
    fn test_end_to_end_audio_pipeline() {
        let sample_rate = 16000u32;
        let processor = AudioProcessor::new(sample_rate);
        let encoder = WavEncoder::new(sample_rate, 1);

        let window_size = (sample_rate as f32 * 0.01) as usize;
        let mut audio = vec![0.005; window_size * 2];
        audio.extend(vec![0.3; window_size * 50]);
        audio.extend(vec![0.005; window_size * 2]);

        let processed = processor.process_for_speech_recognition(&audio).unwrap();

        assert!(processed.len() < audio.len(), "Audio should be trimmed");
        assert!(
            processed.len() >= window_size * 45,
            "Should contain most of the speech"
        );

        let wav_data = encoder.encode_to_wav(&processed).unwrap();

        assert!(wav_data.len() > 44, "Should have WAV header + data");
        assert_eq!(
            wav_data.len(),
            44 + processed.len() * 2,
            "Correct WAV file size"
        );

        assert!(
            wav_data.len() < 25 * 1024 * 1024,
            "Should be well under Whisper 25MB limit"
        );
    }

    #[tokio::test]
    async fn test_process_audio_for_transcription_error_handling() {
        let config = Config::default();

        let test_cases = vec![
            (vec![], "empty audio"),
            (vec![0.1; 100], "too short audio"),
            (vec![0.0; 1600], "silent audio"),
        ];

        for (audio_data, description) in test_cases {
            let result = process_audio_for_transcription(audio_data, 16000, &config, None).await;

            assert!(
                result.is_ok() && result.unwrap() == 1,
                "Should return exit code 1 for {}",
                description
            );
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["cat".to_string()];

        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_failing_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["false".to_string()];

        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_pipe_to_functionality_with_nonexistent_command() {
        use crate::test_utils::ENV_MUTEX;

        let _lock = ENV_MUTEX.lock().await;

        let config = Config::default();
        let pipe_command = vec!["nonexistent_command_12345".to_string()];

        let result =
            process_audio_for_transcription(vec![], 16000, &config, Some(&pipe_command)).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);
    }

    #[test]
    fn test_config_validation_comprehensive() {
        let mut config = Config {
            mistral_api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        config.audio_sample_rate = 0;
        assert!(config.validate().is_err());

        config.audio_sample_rate = 16000;
        config.audio_channels = 0;
        assert!(config.validate().is_err());

        config.audio_channels = 1;
        config.audio_buffer_duration_seconds = 0;
        assert!(config.validate().is_err());
    }
}
