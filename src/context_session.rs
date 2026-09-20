//! Session output: type live deltas, apply voice edits, insert finalized segments.

use crate::config::Config;
use crate::editing::{apply_edit, plan_edit, EditPolicy, TextEdit};
use crate::intent::{detect_intent, DictationIntent};
use crate::transcript::TranscriptBuffer;
use crate::typing::TypingBackend;
use anyhow::Result;
use std::ops::Range;

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

/// Type a live realtime delta, then apply in-stream voice edits when enabled.
///
/// Returns `true` when a voice command changed the buffer (caller should not
/// treat this delta as polish-preview text).
pub async fn handle_live_delta<B: TypingBackend + ?Sized>(
    backend: &B,
    buffer: &mut TranscriptBuffer,
    config: &Config,
    delta: &str,
) -> Result<bool> {
    if delta.is_empty() {
        return Ok(false);
    }
    backend.type_text(delta).await?;
    buffer.append_typed(delta);
    if !config.context_editing {
        return Ok(false);
    }

    let policy = edit_policy(config);
    if let Some(edit) = live_correction_edit(buffer) {
        apply_edit(backend, buffer, edit, policy).await?;
        return Ok(true);
    }
    if let Some((command_range, intent)) = trailing_command(buffer.text()) {
        let cmd_chars = buffer.text()[command_range.clone()].chars().count();
        backend.backspace(cmd_chars).await?;
        buffer.delete_range(command_range);
        let edit = plan_edit(buffer, intent, policy);
        apply_edit(backend, buffer, edit, policy).await?;
        return Ok(true);
    }
    Ok(false)
}

/// After live typing an utterance, replace that suffix with polished text.
pub async fn replace_typed_suffix<B: TypingBackend + ?Sized>(
    backend: &B,
    buffer: &mut TranscriptBuffer,
    typed_suffix: &str,
    replacement: &str,
    policy: EditPolicy,
) -> Result<()> {
    if typed_suffix.is_empty() || replacement == typed_suffix {
        return Ok(());
    }
    let text = buffer.text();
    if !text.ends_with(typed_suffix) {
        return Ok(());
    }
    let start = text.len() - typed_suffix.len();
    let edit = TextEdit::ReplaceSuffix {
        range: start..text.len(),
        replacement: replacement.to_string(),
    };
    apply_edit(backend, buffer, edit, policy).await
}

fn live_correction_edit(buffer: &TranscriptBuffer) -> Option<TextEdit> {
    let text = buffer.text();
    let lower = text.to_lowercase();
    let markers = [
        "actually scratch that",
        "scratch that",
        "actually no",
        "no i mean",
        "i mean",
    ];

    for marker in markers {
        let Some(marker_start) = lower.rfind(marker) else {
            continue;
        };
        let after_marker = marker_start + marker.len();
        let replacement = text[after_marker..]
            .trim_start_matches(|ch: char| {
                ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?' | ':' | ';')
            })
            .trim();
        let replacement = normalize_replacement_phrase(replacement);
        if replacement.is_empty() {
            continue;
        }

        let replacement_words = replacement.split_whitespace().count().max(1);
        let target_words = replacement_words.min(4);
        let before_command = &text[..marker_start];
        let target_range = last_words_range_in_text(before_command, target_words)?;
        let replacement = normalize_insert_spacing(&text[..target_range.start], &replacement);
        let end = text.trim_end_matches(char::is_whitespace).len();
        return Some(TextEdit::ReplaceSuffix {
            range: target_range.start..end,
            replacement,
        });
    }

    None
}

fn normalize_replacement_phrase(replacement: &str) -> String {
    let trimmed = replacement.trim();
    let normalized = trimmed
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_punctuation() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    for prefix in [
        "let s make it ",
        "lets make it ",
        "let us make it ",
        "make it ",
        "change it to ",
        "change that to ",
        "to ",
        "by ",
    ] {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }

    normalized
}

fn last_words_range_in_text(text: &str, count: usize) -> Option<Range<usize>> {
    if count == 0 || text.trim_end().is_empty() {
        return None;
    }

    let end = text.trim_end_matches(char::is_whitespace).len();
    let prefix = &text[..end];
    let mut ranges = Vec::new();
    let mut in_word = false;
    let mut word_end = end;

    for (idx, ch) in prefix.char_indices().rev() {
        if ch.is_whitespace() {
            if in_word {
                ranges.push(idx + ch.len_utf8()..word_end);
                in_word = false;
                if ranges.len() == count {
                    break;
                }
            }
        } else if !in_word {
            in_word = true;
            word_end = idx + ch.len_utf8();
        }
    }

    if in_word && ranges.len() < count {
        ranges.push(0..word_end);
    }

    if ranges.len() < count {
        return None;
    }

    let start = ranges.last().unwrap().start;
    Some(start..end)
}

/// Conservative trailing voice command (delete / reset / explicit replace).
/// Implicit "no …" replacements are handled by [`live_correction_edit`] instead —
/// `detect_intent("no thanks")` would otherwise rewrite the previous word.
fn trailing_command(text: &str) -> Option<(Range<usize>, DictationIntent)> {
    let end = text.trim_end_matches(char::is_whitespace).len();
    if end == 0 {
        return None;
    }
    let prefix = &text[..end];
    let mut word_starts = Vec::new();
    let mut in_word = false;
    for (idx, ch) in prefix.char_indices() {
        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            word_starts.push(idx);
            in_word = true;
        }
    }
    if word_starts.is_empty() {
        return None;
    }
    let max_n = word_starts.len().min(8);
    for n in (1..=max_n).rev() {
        let start = word_starts[word_starts.len() - n];
        let suffix = &prefix[start..];
        let intent = detect_intent(suffix);
        match intent {
            DictationIntent::DeleteLastSentence
            | DictationIntent::DeleteLastLine
            | DictationIntent::DeleteLastWords(_)
            | DictationIntent::ResetContext
            | DictationIntent::ReplaceRecentExact { .. } => {
                let mut start = start;
                if start > 0 {
                    start = text[..start].trim_end_matches(char::is_whitespace).len();
                }
                let mut range = start..end;
                if text[end..].chars().all(char::is_whitespace) {
                    range.end = text.len();
                }
                return Some((range, intent));
            }
            _ => continue,
        }
    }
    None
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

    #[tokio::test]
    async fn live_scratch_that_deletes_typed_command_and_sentence() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        let config = Config::default();
        handle_live_delta(
            &backend,
            &mut buffer,
            &config,
            "Hello world. This is wrong. ",
        )
        .await
        .unwrap();
        let edited = handle_live_delta(&backend, &mut buffer, &config, "scratch that")
            .await
            .unwrap();
        assert!(edited);
        assert_eq!(buffer.text(), "Hello world. ");
        assert!(backend
            .operations()
            .iter()
            .any(|op| matches!(op, TypingOperation::Backspace(_))));
    }

    #[tokio::test]
    async fn live_no_i_mean_replaces_recent_words() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        let config = Config::default();
        handle_live_delta(&backend, &mut buffer, &config, "the meeting is at five")
            .await
            .unwrap();
        let edited = handle_live_delta(&backend, &mut buffer, &config, " no i mean six")
            .await
            .unwrap();
        assert!(edited);
        assert_eq!(buffer.text(), "the meeting is at six");
    }

    #[tokio::test]
    async fn live_no_thanks_is_not_a_command() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        let config = Config::default();
        let edited = handle_live_delta(&backend, &mut buffer, &config, "see you later no thanks")
            .await
            .unwrap();
        assert!(!edited);
        assert_eq!(buffer.text(), "see you later no thanks");
    }

    #[tokio::test]
    async fn live_delta_skips_edits_when_disabled() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        let config = Config {
            context_editing: false,
            ..Config::default()
        };
        let edited = handle_live_delta(&backend, &mut buffer, &config, "Hello world. scratch that")
            .await
            .unwrap();
        assert!(!edited);
        assert_eq!(buffer.text(), "Hello world. scratch that");
        assert!(backend
            .operations()
            .iter()
            .all(|op| matches!(op, TypingOperation::Type(_))));
    }

    #[tokio::test]
    async fn replace_typed_suffix_rewrites_preview() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("the meeting is at five");
        replace_typed_suffix(
            &backend,
            &mut buffer,
            "the meeting is at five",
            "The meeting is at 5.",
            EditPolicy::default(),
        )
        .await
        .unwrap();
        assert_eq!(buffer.text(), "The meeting is at 5.");
    }
}
