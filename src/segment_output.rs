//! Per-utterance finalize: local cleanup, optional LLM polish, context buffer + typing.

use crate::beep::{BeepPlayer, BeepType};
use crate::config::Config;
use crate::context_session::{handle_final_segment, preprocess_insert};
use crate::intent::{detect_intent, DictationIntent};
use crate::llm_polish::{handle_polish_failure, polish_segment};
use crate::profile::POLISH_CONTEXT_CHARS;
use crate::transcript::TranscriptBuffer;
use crate::typing::{OutputBackend, TypingBackend};
use anyhow::Result;
use dictate::overlay_ipc::{OverlayPublisher, OverlayState};

pub async fn emit_finalized_segment(
    config: &Config,
    pipe_command: Option<Vec<String>>,
    session_buffer: &tokio::sync::Mutex<TranscriptBuffer>,
    raw_text: &str,
    dictation_mode: &str,
    beep_player: &BeepPlayer,
    overlay: &OverlayPublisher,
) -> Result<()> {
    let text = raw_text.trim();
    if text.is_empty() {
        return Ok(());
    }

    overlay.set_state(OverlayState::Processing);
    eprintln!("📝 {}", text);

    let backend = OutputBackend::new(pipe_command);
    let mut buffer = session_buffer.lock().await;

    if config.profile.uses_segment_polish() {
        match detect_intent(text) {
            DictationIntent::InsertText(insert) => {
                let prior = buffer.recent_window(POLISH_CONTEXT_CHARS).to_string();
                let local = preprocess_insert(&insert, config, dictation_mode);
                let polish = &config.text_processing.polish;
                let to_insert = if polish.effective_enabled(true) && config.mistral_api_key.is_some() {
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

    beep_player.play_async(BeepType::Success).await.ok();
    overlay.clear_preview();
    overlay.set_state(OverlayState::Listening);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::DictateProfile;
    #[tokio::test]
    async fn segment_polish_off_types_local_processed() {
        let mut config = Config::default();
        config.profile = DictateProfile::Segmented;
        config.mistral_api_key = None;
        let beep = BeepPlayer::new(crate::beep::BeepConfig {
            enabled: false,
            volume: 0.0,
        })
        .unwrap();
        let overlay = OverlayPublisher::from_env_enabled(false);
        let buffer = tokio::sync::Mutex::new(TranscriptBuffer::new());
        emit_finalized_segment(
            &config,
            None,
            &buffer,
            "hello world",
            "plain",
            &beep,
            &overlay,
        )
        .await
        .unwrap();
        assert!(buffer.lock().await.text().contains("hello"));
    }

}