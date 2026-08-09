use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use config::Config;
use futures::StreamExt;
use log::{error, info, warn};
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::Instant;
use tokio::io::AsyncWriteExt;

mod audio;
mod audio_processing;
mod beep;
#[cfg(not(test))]
mod clip_pipeline;
mod clipboard_intent;
mod command;
mod command_mode;
mod config;
mod config_cli;
mod context_session;
mod control;
mod developer_modes;
mod editing;
mod history;
mod intent;
mod llm_polish;
mod platform;
mod polish_styles;
mod profile;
mod scratchpad;
mod segment_output;
mod setup_tui;
mod streaming;
mod text_processing;
mod transcript;
mod transcription;
mod typing;
mod wav;

#[cfg(test)]
mod test_utils;

#[cfg(not(test))]
use audio::AudioRecorder;
#[cfg(not(test))]
use beep::{BeepConfig, BeepPlayer, BeepType};
#[cfg(not(test))]
use clip_pipeline::{
    process_audio_for_transcription, run_clip_transcription, ClipTranscriptionRequest,
};
use config_cli::{
    print_shortcut, run_autostart_command, run_config_command, run_toggle_daemon, AutostartCommand,
    ConfigCommand, ShortcutArgs, ToggleKind,
};
#[cfg(not(test))]
use control::{Control, ControlEvent};
#[cfg(not(test))]
use profile::DictateProfile;
#[cfg(not(test))]
use transcription::{SharedProvider, TranscriptionFactory};

// ─── CLI argument definitions ────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "dictate")]
#[command(about = "Speech-to-Text for Wayland and Windows - shortcut-driven transcription")]
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

    /// Stream mode: continuously transcribe speech with VAD
    #[arg(long)]
    stream: bool,

    /// Daemon mode: keep running with model loaded in memory
    #[arg(long)]
    daemon: bool,

    /// Start daemon warm but idle; a shortcut or `dictate toggle` begins recording
    #[arg(long)]
    idle_on_start: bool,

    /// Developer dictation mode
    #[arg(long, default_value = "plain")]
    dictation_mode: String,

    /// Shortcut mode: live (realtime) or smart (polish + context-friendly)
    #[arg(long, value_parser = ["live", "smart"])]
    mode: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Read, write, or interactively create configuration
    Config {
        #[command(subcommand)]
        command: Box<ConfigCommand>,
    },
    /// Print compositor shortcut snippets
    Shortcuts(ShortcutArgs),
    /// Start or toggle the live/smart daemon (what a shortcut runs)
    Toggle {
        #[arg(value_enum)]
        kind: ToggleKind,
    },
    /// Check config, API keys, and optional dependencies
    Doctor,
    /// Interactive setup (profiles, keys, shortcuts)
    Setup {
        /// Skip provider/beep questions
        #[arg(long)]
        quick: bool,
    },
    /// Local transcript history (no cloud)
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    /// Local markdown notes from dictation
    Scratchpad {
        #[command(subcommand)]
        command: ScratchpadCommand,
    },
    /// Open the preferred-words manager for names, tools, and common mishearings
    Words,
    /// Install/remove/status login startup for instant shortcuts
    Autostart {
        #[command(subcommand)]
        command: AutostartCommand,
    },
    /// Hold the global shortcuts and toggle daemons when they are pressed
    #[cfg(windows)]
    Hotkeys {
        /// Start idle daemons up front so the first press has no cold start
        #[arg(long)]
        warm: bool,
    },
}

#[derive(Subcommand)]
enum HistoryCommand {
    /// Show recent entries
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Delete history file
    Clear,
}

#[derive(Subcommand)]
enum ScratchpadCommand {
    /// Print scratchpad contents
    Show,
    /// Append text (stdin if omitted)
    Append { text: Option<String> },
    /// Clear scratchpad
    Clear,
    /// Open in $EDITOR
    Edit,
}

/// Runtime args plus resolved pipe target (CLI or SHORTCUT_OUTPUT from config).
#[cfg_attr(test, allow(dead_code))]
struct ArgsWithPipe<'a> {
    base: &'a Args,
    pipe_to: Option<&'a Vec<String>>,
}

fn load_config_for_doctor(envfile: &PathBuf) -> Result<Config> {
    let mut config = if envfile.exists() {
        Config::load_env_file(envfile).unwrap_or_else(|_| Config::from_env())
    } else {
        Config::from_env()
    };
    let text_path = Config::text_config_path_for_env_file(envfile);
    config.load_text_config_file(&text_path)?;
    Ok(config)
}

// ─── Model download ──────────────────────────────────────────────────────────

async fn download_model(model: &str) -> Result<PathBuf> {
    let base_url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
    let url = format!("{base_url}/{model}");
    let dir = Config::model_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(model);
    let temp_path = dir.join(format!("{model}.download"));

    let resp = reqwest::get(&url).await.map_err(|e| anyhow!("{e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("Failed to download model: {}", resp.status()));
    }

    let total_size = resp.content_length();
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    let start = Instant::now();

    print!("{model}... ");
    std::io::stdout().flush().ok();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Download error: {e}"))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(total) = total_size {
            let pct = (downloaded as f64 / total as f64) * 100.0;
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                downloaded as f64 / elapsed / 1024.0 / 1024.0
            } else {
                0.0
            };
            let eta = if speed > 0.0 {
                (total - downloaded) as f64 / (speed * 1024.0 * 1024.0)
            } else {
                0.0
            };
            print!("\r{model}... {pct:.1}% ({speed:.1} MB/s, ETA: {eta:.0}s)    ");
        } else {
            print!(
                "\r{model}... {:.1} MB downloaded    ",
                downloaded as f64 / 1024.0 / 1024.0
            );
        }
        std::io::stdout().flush().ok();
    }

    file.flush().await?;
    tokio::fs::rename(&temp_path, &path).await?;
    info!("Model downloaded to {}", path.display());
    Ok(path)
}

// ─── Clip mode (original behavior) ──────────────────────────────────────────

#[cfg(not(test))]
async fn run_clip_mode(config: &Config, args: &ArgsWithPipe<'_>) -> Result<()> {
    info!("dictate - Speech-to-Text for Wayland and Windows");
    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;
    let mut recorder = AudioRecorder::from_config(config)?;

    beep_player.play_async(BeepType::RecordingStart).await.ok();
    tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

    if let Err(e) = recorder.start_recording() {
        error!("Failed to start recording: {e}");
        eprintln!("This may be due to PipeWire not being available or insufficient permissions.");
        return Err(e);
    }

    info!("Recording started. {} to transcribe.", control::TOGGLE_HINT);
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut control =
        Control::start_best_effort(control::Slot::from_mode(args.base.mode.as_deref()));

    loop {
        let event =
            tokio::time::timeout(tokio::time::Duration::from_millis(50), control.recv()).await;

        match event {
            Ok(Some(ControlEvent::Toggle)) => {
                info!("Toggle received: transcribing");
                recorder.stop_recording().ok();
                record_result(
                    &recorder,
                    &beep_player,
                    config,
                    args.pipe_to,
                    &args.base.dictation_mode,
                )
                .await;
                break;
            }
            Ok(Some(ControlEvent::Shutdown)) | Ok(None) => {
                info!("Shutdown requested");
                recorder.stop_recording().ok();
                recorder.clear_buffer().ok();
                break;
            }
            Err(_) => {
                recorder.process_audio_events().ok();
            }
        }
    }

    Ok(())
}

/// Shared helper to stop, get audio, process, and exit.
#[cfg(not(test))]
async fn record_result(
    recorder: &AudioRecorder,
    beep_player: &BeepPlayer,
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    dictation_mode: &str,
) {
    beep_player.play_async(BeepType::RecordingStop).await.ok();
    match recorder.get_audio_data() {
        Ok(audio_data) => {
            let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
            info!("Captured {} samples ({duration:.2}s)", audio_data.len());
            match process_audio_for_transcription(
                audio_data,
                config.audio_sample_rate,
                config,
                pipe_command,
                dictation_mode,
            )
            .await
            {
                Ok(code) => {
                    info!("Completed with exit code {code}");
                    recorder.clear_buffer().ok();
                    std::process::exit(code);
                }
                Err(e) => {
                    error!("Processing failed: {e}");
                    recorder.clear_buffer().ok();
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            error!("Failed to get audio data: {e}");
            std::process::exit(1);
        }
    }
}

// ─── Daemon clip mode ────────────────────────────────────────────────────────

#[cfg(not(test))]
async fn run_daemon_clip_mode(config: &Config, args: &ArgsWithPipe<'_>) -> Result<()> {
    info!("Daemon mode — model stays loaded for multiple recordings");

    let provider =
        TranscriptionFactory::create_provider(&config.transcription_provider, config).await?;
    let provider: SharedProvider = std::sync::Arc::new(tokio::sync::Mutex::new(provider));
    info!("Provider ready");

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;
    let mut recorder = AudioRecorder::from_config(config)?;
    let mut control = Control::start(control::Slot::from_mode(args.base.mode.as_deref()))?;
    let mut is_recording = false;

    if config.profile == DictateProfile::SmartPaste && !args.base.idle_on_start {
        info!("Smart paste recording started");
        beep_player.play_async(BeepType::RecordingStart).await.ok();
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        if recorder.start_recording().is_err() {
            error!("Failed to start recording");
        } else {
            is_recording = true;
        }
    }

    loop {
        let event =
            tokio::time::timeout(tokio::time::Duration::from_millis(50), control.recv()).await;

        match event {
            Ok(Some(ControlEvent::Toggle)) => {
                if !is_recording {
                    info!("Recording started");
                    beep_player.play_async(BeepType::RecordingStart).await.ok();
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    if recorder.start_recording().is_err() {
                        error!("Failed to start recording");
                    } else {
                        is_recording = true;
                    }
                } else {
                    info!("Recording stopped, transcribing...");
                    is_recording = false;
                    recorder.stop_recording().ok();
                    beep_player.play_async(BeepType::RecordingStop).await.ok();

                    match recorder.get_audio_data() {
                        Ok(audio_data) => {
                            let duration = recorder.get_recording_duration_seconds().unwrap_or(0.0);
                            info!("Captured {} samples ({duration:.2}s)", audio_data.len());

                            let result = run_clip_transcription(
                                audio_data,
                                config.audio_sample_rate,
                                ClipTranscriptionRequest {
                                    config,
                                    pipe_command: args.pipe_to,
                                    beep_player: &beep_player,
                                    dictation_mode: &args.base.dictation_mode,
                                    provider: Some(std::sync::Arc::clone(&provider)),
                                },
                            )
                            .await;

                            recorder.clear_buffer().ok();
                            match result {
                                Ok(code) => info!("Done (exit code: {code})"),
                                Err(e) => error!("Processing failed: {e}"),
                            }
                        }
                        Err(e) => error!("Failed to get audio data: {e}"),
                    }
                }
            }
            Ok(Some(ControlEvent::Shutdown)) | Ok(None) => {
                info!("Shutdown requested: stopping daemon");
                if is_recording {
                    recorder.stop_recording().ok();
                }
                recorder.clear_buffer().ok();
                break;
            }
            Err(_) => {
                if is_recording {
                    recorder.process_audio_events().ok();
                }
            }
        }
    }

    Ok(())
}

// ─── Entrypoint ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging (RUST_LOG env var, defaults to "info")
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    let args = Args::parse();
    let envfile = args
        .envfile
        .clone()
        .unwrap_or_else(config_cli::get_default_config_path);

    if let Some(command) = &args.command {
        match command {
            Commands::Config { command } => run_config_command(command, &envfile)?,
            Commands::Shortcuts(shortcut_args) => print_shortcut(shortcut_args, &envfile),
            Commands::Toggle { kind } => {
                run_toggle_daemon(*kind)?;
                return Ok(());
            }
            Commands::Doctor => {
                let config = load_config_for_doctor(&envfile)?;
                config_cli::run_doctor(&config, &envfile);
                return Ok(());
            }
            Commands::Setup { quick } => {
                setup_tui::run_setup(*quick, &envfile)?;
                return Ok(());
            }
            Commands::History { command } => {
                match command {
                    HistoryCommand::List { limit } => {
                        for entry in history::list_entries(*limit)? {
                            println!("{}\t[{}] {}", entry.ts, entry.profile, entry.text);
                        }
                    }
                    HistoryCommand::Clear => {
                        history::clear_history()?;
                        eprintln!("History cleared.");
                    }
                }
                return Ok(());
            }
            Commands::Scratchpad { command } => {
                match command {
                    ScratchpadCommand::Show => {
                        let s = scratchpad::read_all()?;
                        if s.is_empty() {
                            eprintln!("(scratchpad empty)");
                        } else {
                            print!("{s}");
                        }
                    }
                    ScratchpadCommand::Append { text } => {
                        let owned = match text {
                            Some(t) => t.clone(),
                            None => {
                                let mut s = String::new();
                                std::io::stdin().read_to_string(&mut s)?;
                                s
                            }
                        };
                        let path = scratchpad::append_text(&owned)?;
                        eprintln!("Appended to {}", path.display());
                    }
                    ScratchpadCommand::Clear => {
                        scratchpad::clear()?;
                        eprintln!("Scratchpad cleared.");
                    }
                    ScratchpadCommand::Edit => scratchpad::open_in_editor()?,
                }
                return Ok(());
            }
            Commands::Words => {
                let text_path = Config::text_config_path_for_env_file(&envfile);
                #[cfg(feature = "words-ui")]
                {
                    dictate::words_ui::run(&text_path)?;
                }
                #[cfg(not(feature = "words-ui"))]
                {
                    if let Some(parent) = text_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    if !text_path.exists() {
                        std::fs::write(&text_path, "preferred_words = []\n\n[dictionary]\n")?;
                    }
                    let editor = platform::default_editor();
                    let status = std::process::Command::new(&editor)
                        .arg(&text_path)
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("Editor exited with status {status}");
                    }
                    eprintln!(
                        "Tip: build with `cargo build --release --features words-ui` for the Dictionary GUI."
                    );
                }
                return Ok(());
            }
            Commands::Autostart { command } => {
                run_autostart_command(command)?;
                return Ok(());
            }
            #[cfg(windows)]
            Commands::Hotkeys { warm } => {
                config_cli::run_hotkey_agent(*warm, &envfile).await?;
                return Ok(());
            }
        }
        return Ok(());
    }

    // Load configuration
    let mut config = if envfile.exists() {
        info!("Loading environment from: {}", envfile.display());
        match Config::load_env_file(&envfile) {
            Ok(c) => c,
            Err(e) => {
                warn!(
                    "Failed to load {}: {e}, falling back to env vars",
                    envfile.display()
                );
                Config::from_env()
            }
        }
    } else {
        warn!("{} not found, using system environment", envfile.display());
        Config::from_env()
    };

    let text_config_path = Config::text_config_path_for_env_file(&envfile);
    config
        .load_text_config_file(&text_config_path)
        .unwrap_or_else(|e| {
            error!(
                "Failed to load text config {}: {e}",
                text_config_path.display()
            );
            std::process::exit(1);
        });

    if let Some(ref m) = args.mode {
        if let Some(mode) = profile::DictateMode::parse(m) {
            config.profile = mode.profile();
        } else {
            error!("Unknown --mode {m}; use live or smart");
            std::process::exit(1);
        }
    }

    // Download model and exit
    if args.download_model {
        match download_model(&config.whisper_model).await {
            Ok(path) => {
                eprintln!("Model downloaded to {}", path.display());
                return Ok(());
            }
            Err(e) => {
                error!("Failed to download model: {e}");
                std::process::exit(1);
            }
        }
    }

    // Validate config
    if let Err(e) = config.validate() {
        warn!("Configuration warning: {e}");
        if config.transcription_provider == "local" {
            std::process::exit(1);
        }
    }

    let pipe_to = config.resolve_pipe_to(args.pipe_to.as_ref());

    #[cfg(not(test))]
    let args_with_pipe = ArgsWithPipe {
        base: &args,
        pipe_to,
    };

    // Mode selection driven by DICTATE_PROFILE (see profile.rs)
    let use_realtime = config.use_mistral_realtime_stt();

    let daemon = args.daemon;

    #[cfg(not(test))]
    let slot = control::Slot::from_mode(args.mode.as_deref());

    if daemon && use_realtime {
        let (_control_tx, mut control_rx) = tokio::sync::mpsc::channel(8);
        // A daemon keeps running after a stop, so only toggles reach the loop.
        #[cfg(not(test))]
        control::spawn_forwarder(Control::start(slot)?, _control_tx, false);

        let active_on_start = !args.idle_on_start && config.realtime_daemon_active_on_start();
        streaming::run_mistral_realtime_daemon(
            &config,
            pipe_to,
            &mut control_rx,
            &args.dictation_mode,
            active_on_start,
        )
        .await?;
    } else if daemon {
        #[cfg(not(test))]
        run_daemon_clip_mode(&config, &args_with_pipe).await?;
        #[cfg(test)]
        {
            let _ = (&config, pipe_to);
            eprintln!("Daemon mode not available in tests");
        }
    } else if args.stream {
        let (_shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
        // One-shot stream: either kind of control message ends it.
        #[cfg(not(test))]
        control::spawn_forwarder(Control::start_best_effort(slot), _shutdown_tx, true);

        streaming::run_stream(&config, pipe_to, &mut shutdown_rx, &args.dictation_mode).await?;
    } else {
        #[cfg(not(test))]
        run_clip_mode(&config, &args_with_pipe).await?;
        #[cfg(test)]
        {
            let _ = (&config, pipe_to);
            eprintln!("Test mode: Signal handling disabled");
        }
    }

    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

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
        assert!(wav_result.is_ok(), "WAV encoding should succeed");

        let wav_data = wav_result.unwrap();
        assert!(wav_data.len() > 44, "WAV data should have header");

        // Verify header
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");
        assert_eq!(&wav_data[36..40], b"data");
    }
}
