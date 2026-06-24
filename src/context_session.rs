//! Finalized-segment output with optional context-aware edits (ported from context-aware-editing branch).

use crate::config::Config;
use crate::editing::{apply_edit, plan_edit, EditPolicy};
use crate::intent::{detect_intent, DictationIntent};
use crate::transcript::TranscriptBuffer;
use crate::typing::TypingBackend;
use anyhow::Result;

pub fn edit_policy(config: &Config) -> EditPolicy {
    EditPolicy {
        max_delete_chars: config.context_editing_max_delete_chars,
        max_delete_words: config.context_editing_max_delete_words,
        type_rejected_commands: false,
    }
}

fn normalize_insert_spacing(previous: &str, next: &str) -> String {
    if previous.is_empty() || next.is_empty() {
        return next.to_string();
    }
    let prev_needs_space = !previous.ends_with(char::is_whitespace);
    let next_allows_space =
        !next.starts_with(char::is_whitespace) && !next.starts_with(['.', ',', '!', '?', ':', ';']);
    if prev_needs_space && next_allows_space {
        format!(" {}", next)
    } else {
        next.to_string()
    }
}

/// Apply dictionary/snippets/cleanup to plain insert text only (not voice commands).
pub fn preprocess_insert(text: &str, config: &Config, dictation_mode: &str) -> String {
    let t = crate::text_processing::process_text(text, &config.text_processing);
    crate::developer_modes::apply_developer_mode(&t, dictation_mode)
}

pub async fn handle_final_segment<B: TypingBackend + ?Sized>(
    backend: &B,
    buffer: &mut TranscriptBuffer,
    config: &Config,
    raw_text: &str,
    dictation_mode: &str,
) -> Result<()> {
    let intent = detect_intent(raw_text);
    let intent = match intent {
        DictationIntent::InsertText(insert) => {
            let processed = preprocess_insert(&insert, config, dictation_mode);
            DictationIntent::InsertText(normalize_insert_spacing(buffer.text(), &processed))
        }
        other => other,
    };
    let policy = edit_policy(config);
    let edit = plan_edit(buffer, intent, policy);
    apply_edit(backend, buffer, edit, policy).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::{MockTypingBackend, TypingOperation};

    #[tokio::test]
    async fn scratch_that_deletes_sentence() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("Hello world. This is wrong.");
        let config = Config::default();
        handle_final_segment(&backend, &mut buffer, &config, "scratch that", "plain")
            .await
            .unwrap();
        assert_eq!(buffer.text(), "Hello world. ");
        assert!(backend
            .operations()
            .iter()
            .any(|op| matches!(op, TypingOperation::Backspace(_))));
    }
}
