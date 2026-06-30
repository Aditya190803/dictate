//! Per-utterance finalize: local cleanup, optional LLM polish, context buffer + typing.

use crate::beep::{BeepPlayer, BeepType};
use crate::command_mode;
use crate::config::Config;
use crate::context_session::{handle_final_segment, preprocess_insert};
use crate::intent::{detect_intent, DictationIntent};
use crate::llm_polish::{handle_polish_failure, polish_segment};
use crate::profile::POLISH_CONTEXT_CHARS;
use crate::transcript::TranscriptBuffer;
use crate::typing::{OutputBackend, TypingBackend};
use anyhow::Result;

pub async fn emit_finalized_segment(
    config: &Config,
    pipe_command: Option<Vec<String>>,
    session_buffer: &tokio::sync::Mutex<TranscriptBuffer>,
    raw_text: &str,
    dictation_mode: &str,
    beep_player: &BeepPlayer,
) -> Result<()> {
    let text = raw_text.trim();
    if text.is_empty() {
        return Ok(());
    }

    eprintln!("📝 {}", text);

    let backend = OutputBackend::new(pipe_command.clone());
    let cm = &config.text_processing.command_mode;
    if let Some(clip) = command_mode::peek_clipboard_text(cm).await {
        let force = config.profile.is_command_mode();
        if force
            || (crate::clipboard_intent::should_apply_clipboard_command(text, &clip)
                && !clip.is_empty())
        {
            eprintln!("📋 Clipboard command");
            match command_mode::run_command_mode(text, None, cm, Some(config)).await {
                Ok(out) => {
                    if !out.is_empty() {
                        backend.type_text(&out).await?;
                    }
                    let _ = crate::history::append_transcript(
                        text,
                        config.profile.as_str(),
                        config.save_transcript_history(),
                    );
                    beep_player.play_async(BeepType::Success).await.ok();
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("⚠️  Clipboard command failed: {e}");
                    beep_player.play_async(BeepType::Error).await.ok();
                    return Ok(());
                }
            }
        }
    }

    let mut buffer = session_buffer.lock().await;

    if config.profile.uses_segment_polish() {
        match detect_intent(text) {
            DictationIntent::InsertText(insert) => {
                let prior = buffer.recent_window(POLISH_CONTEXT_CHARS).to_string();
                let local = preprocess_insert(&insert, config, dictation_mode);
                let polish = &config.text_processing.polish;
                let to_insert =
                    if polish.effective_enabled(true) && config.polish_available() {
                        eprintln!("✨ Polishing…");
                        match polish_segment(&local, &prior, config, polish, dictation_mode).await {
                            Ok(p) => p,
                            Err(e) => handle_polish_failure(polish, &local, &e),
                        }
                    } else {
                        local
                    };
                if !to_insert.is_empty() {
                    handle_final_segment(&backend, &mut buffer, config, &to_insert, dictation_mode)
                        .await?;
                }
            }
            _ => {
                handle_final_segment(&backend, &mut buffer, config, text, dictation_mode).await?;
            }
        }
    } else if config.context_editing {
        handle_final_segment(&backend, &mut buffer, config, text, dictation_mode).await?;
    } else {
        let processed = crate::text_processing::process_text(text, &config.text_processing);
        let processed = crate::developer_modes::apply_developer_mode(&processed, dictation_mode);
        backend.type_text(&processed).await?;
        buffer.append_typed(&processed);
    }

    let _ = crate::history::append_transcript(
        text,
        config.profile.as_str(),
        config.save_transcript_history(),
    );

    beep_player.play_async(BeepType::Success).await.ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::DictateProfile;
    #[tokio::test]
    async fn segment_polish_off_types_local_processed() {
        let config = Config {
            profile: DictateProfile::Segmented,
            mistral_api_key: None,
            ..Default::default()
        };
        let beep = BeepPlayer::new(crate::beep::BeepConfig {
            enabled: false,
            volume: 0.0,
        })
        .unwrap();
        let buffer = tokio::sync::Mutex::new(TranscriptBuffer::new());
        emit_finalized_segment(
            &config,
            None,
            &buffer,
            "hello world",
            "plain",
            &beep,
        )
        .await
        .unwrap();
        assert!(buffer.lock().await.text().contains("hello"));
    }
}