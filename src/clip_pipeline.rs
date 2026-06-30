//! One-shot clip transcription: process audio → STT → text post-process → stdout/pipe.

use crate::audio_processing::{AudioProcessingError, AudioProcessor};
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::command;
use crate::command_mode;
use crate::config::Config;
use crate::developer_modes::apply_developer_mode;
use crate::llm_polish::{handle_polish_failure, polish_transcript};
use crate::profile::DictateProfile;
use crate::text_processing::process_text;
use crate::transcription::{SharedProvider, TranscriptionError, TranscriptionFactory};
use crate::wav::WavEncoder;
use anyhow::Result;
use log::{debug, error, info, warn};

/// Bundles runtime targets for a single clip transcription.
pub struct ClipTranscriptionRequest<'a> {
    pub config: &'a Config,
    pub pipe_command: Option<&'a Vec<String>>,
    pub beep_player: &'a BeepPlayer,

    pub dictation_mode: &'a str,
    /// When set (daemon), reuse the loaded provider instead of creating one per clip.
    pub provider: Option<SharedProvider>,
}

async fn maybe_run_clipboard_command(text: &str, config: &Config) -> Option<Result<String>> {
    let cm = &config.text_processing.command_mode;
    let clip = command_mode::peek_clipboard_text(cm)
        .await
        .unwrap_or_default();
    let force = config.profile.is_command_mode();
    if force
        || (crate::clipboard_intent::should_apply_clipboard_command(text, &clip)
            && clip.len() >= 12)
    {
        eprintln!("📋 Clipboard command");
        let clip_ref = if force { None } else { Some(clip.as_str()) };
        return Some(command_mode::run_command_mode(text, clip_ref, cm, Some(config)).await);
    }
    None
}

pub async fn finalize_transcribed_text(
    text: &str,
    config: &Config,
    dictation_mode: &str,
) -> Result<String> {
    if let Some(r) = maybe_run_clipboard_command(text, config).await {
        return r;
    }

    let local = process_text(text, &config.text_processing);
    let local = apply_developer_mode(&local, dictation_mode);

    if config.profile != DictateProfile::SmartPaste {
        return Ok(local);
    }

    let polish = &config.text_processing.polish;
    if !polish.effective_enabled(true) {
        return Ok(local);
    }

    eprintln!("✨ Polishing…");
    match polish_transcript(&local, config, polish, dictation_mode).await {
        Ok(p) => Ok(p),
        Err(e) => Ok(handle_polish_failure(polish, &local, &e)),
    }
}

fn transcription_language(config: &Config) -> Option<String> {
    if config.transcription_language == "auto" {
        None
    } else {
        Some(config.transcription_language.clone())
    }
}

async fn pipe_or_print(pipe_command: Option<&Vec<String>>, text: &str) -> i32 {
    if let Some(cmd) = pipe_command {
        match command::execute_with_input(cmd, text).await {
            Ok(code) => code,
            Err(e) => {
                error!("Pipe command failed: {e}");
                1
            }
        }
    } else {
        println!("{text}");
        0
    }
}

pub fn print_transcription_error_hint(e: &TranscriptionError) {
    match e {
        TranscriptionError::AuthenticationFailed { provider, details } => {
            if let Some(d) = details {
                eprintln!("  🔑 Authentication details: {d}");
            }
            eprintln!("  💡 Check your {provider} API key");
        }
        TranscriptionError::NetworkError(d) => {
            eprintln!(
                "  🌐 {}: {} - {}",
                d.provider, d.error_type, d.error_message
            );
        }
        TranscriptionError::ApiError(d) => {
            if let Some(s) = d.status_code {
                eprintln!("  📡 HTTP {s}");
            }
            if let Some(c) = &d.error_code {
                eprintln!("  🏷️  Error code: {c}");
            }
        }
        TranscriptionError::FileTooLarge(size) => {
            eprintln!("  💡 Audio too large: {size} bytes (max 25MB)");
        }
        _ => {}
    }
}

async fn handle_audio_process_error(beep: &BeepPlayer, err: AudioProcessingError) -> i32 {
    error!("Audio processing failed: {err}");
    if let Some(tip) = err.user_tip() {
        eprintln!("{tip}");
    }
    beep.play_async(BeepType::Error).await.ok();
    1
}

async fn transcribe_wav(
    wav_data: Vec<u8>,
    language: Option<String>,
    config: &Config,
    provider: Option<SharedProvider>,
) -> std::result::Result<String, TranscriptionError> {
    if let Some(shared) = provider {
        let guard = shared.lock().await;
        crate::transcription::TranscriptionProvider::transcribe_with_language(
            guard.as_ref(),
            wav_data,
            language,
        )
        .await
    } else {
        let provider =
            TranscriptionFactory::create_provider(&config.transcription_provider, config)
                .await
                .map_err(|e| TranscriptionError::ConfigurationError(format!("{e:#}")))?;
        crate::transcription::TranscriptionProvider::transcribe_with_language(
            provider.as_ref(),
            wav_data,
            language,
        )
        .await
    }
}

/// Process captured PCM, transcribe, post-process, and emit text. Returns process exit code.
pub async fn run_clip_transcription(
    audio_data: Vec<f32>,
    sample_rate: u32,
    req: ClipTranscriptionRequest<'_>,
) -> Result<i32> {
    let processor = AudioProcessor::new(sample_rate);

    let processed_audio = match processor.process_for_speech_recognition(&audio_data) {
        Ok(p) => p,
        Err(e) => return Ok(handle_audio_process_error(req.beep_player, e).await),
    };

    if req.provider.is_none() {
        let original_duration = processor.get_duration_seconds(&audio_data);
        let processed_duration = processor.get_duration_seconds(&processed_audio);
        debug!(
            "Audio: {original_duration:.2}s → {processed_duration:.2}s ({} samples)",
            processed_audio.len()
        );
    }

    let encoder = WavEncoder::new(sample_rate, 1);
    let wav_data = match encoder.encode_to_wav(&processed_audio) {
        Ok(w) => w,
        Err(e) => {
            error!("WAV encoding failed: {e}");
            req.beep_player.play_async(BeepType::Error).await.ok();
            return Ok(1);
        }
    };

    debug!("WAV encoded: {} bytes", wav_data.len());

    if req.provider.is_none() {
        info!(
            "Sending to {} provider...",
            req.config.transcription_provider
        );
    }

    let language = transcription_language(req.config);
    let transcribe_result = transcribe_wav(wav_data, language, req.config, req.provider).await;

    let exit_code = match transcribe_result {
        Ok(transcribed_text) => {
            let text = transcribed_text.trim();
            if text.is_empty() {
                warn!("Empty transcription from provider");
                let code = pipe_or_print(req.pipe_command, "").await;
                req.beep_player.play_async(BeepType::Success).await.ok();
                return Ok(code);
            }

            info!("Transcription: \"{text}\"");
            let processed_text =
                match finalize_transcribed_text(text, req.config, req.dictation_mode).await {
                    Ok(t) => t,
                    Err(e) => {
                        error!("Text processing failed: {e}");
                        req.beep_player.play_async(BeepType::Error).await.ok();
                        return Ok(1);
                    }
                };

            if processed_text.is_empty() && req.config.profile == DictateProfile::SmartPaste {
                req.beep_player.play_async(BeepType::Error).await.ok();
                return Ok(1);
            }

            if let Err(e) = crate::history::append_transcript(
                &processed_text,
                req.config.profile.as_str(),
                req.config.save_transcript_history(),
            ) {
                warn!("Failed to append transcript history: {e}");
            }

            pipe_or_print(req.pipe_command, &processed_text).await
        }
        Err(e) => {
            error!("Transcription failed: {e}");
            print_transcription_error_hint(&e);
            1
        }
    };

    if exit_code == 0 {
        req.beep_player.play_async(BeepType::Success).await.ok();
    } else {
        req.beep_player.play_async(BeepType::Error).await.ok();
    }

    Ok(exit_code)
}

/// Clip mode without a pre-loaded provider; creates beeps internally (one-shot SIGUSR1 flow).
pub async fn process_audio_for_transcription(
    audio_data: Vec<f32>,
    sample_rate: u32,
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    dictation_mode: &str,
) -> Result<i32> {
    let beep_player = BeepPlayer::new(BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    })?;

    run_clip_transcription(
        audio_data,
        sample_rate,
        ClipTranscriptionRequest {
            config,
            pipe_command,
            beep_player: &beep_player,
            dictation_mode,
            provider: None,
        },
    )
    .await
}
