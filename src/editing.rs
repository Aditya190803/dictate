use crate::intent::DictationIntent;
use crate::transcript::TranscriptBuffer;
use crate::typing::TypingBackend;
use anyhow::Result;
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
pub struct EditPolicy {
    pub max_delete_chars: usize,
    pub max_delete_words: usize,
    pub type_rejected_commands: bool,
}

impl Default for EditPolicy {
    fn default() -> Self {
        Self {
            max_delete_chars: 300,
            max_delete_words: 10,
            type_rejected_commands: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextEdit {
    Insert(String),
    DeleteSuffix {
        range: Range<usize>,
    },
    ReplaceSuffix {
        range: Range<usize>,
        replacement: String,
    },
    ResetContext,
    Reject(String),
}

pub fn plan_edit(
    buffer: &TranscriptBuffer,
    intent: DictationIntent,
    policy: EditPolicy,
) -> TextEdit {
    let edit = match intent {
        DictationIntent::InsertText(text) => TextEdit::Insert(text),
        DictationIntent::DeleteLastSentence => buffer.last_sentence_range().map_or_else(
            || TextEdit::Reject("No previous sentence to delete".to_string()),
            |range| TextEdit::DeleteSuffix { range },
        ),
        DictationIntent::DeleteLastLine => buffer.last_line_range().map_or_else(
            || TextEdit::Reject("No previous line to delete".to_string()),
            |range| TextEdit::DeleteSuffix { range },
        ),
        DictationIntent::DeleteLastWords(count) => {
            if count > policy.max_delete_words {
                TextEdit::Reject(format!(
                    "Refusing to delete {} words; limit is {}",
                    count, policy.max_delete_words
                ))
            } else {
                buffer.last_words_range(count).map_or_else(
                    || TextEdit::Reject("Not enough previous words to delete".to_string()),
                    |range| TextEdit::DeleteSuffix { range },
                )
            }
        }
        DictationIntent::ReplaceRecentExact { from, to } => {
            plan_exact_replacement(buffer, &from, &to)
        }
        DictationIntent::ReplaceRecentImplicit { replacement } => {
            plan_implicit_replacement(buffer, &replacement)
        }
        DictationIntent::ResetContext => TextEdit::ResetContext,
        DictationIntent::CommandRejected { reason } => TextEdit::Reject(reason),
    };

    validate_edit(buffer, edit, policy)
}

fn plan_exact_replacement(buffer: &TranscriptBuffer, from: &str, to: &str) -> TextEdit {
    if to.trim().is_empty() {
        return TextEdit::Reject("Replacement text is empty".to_string());
    }
    buffer.last_exact_suffix_match(from).map_or_else(
        || TextEdit::Reject(format!("Could not safely find recent suffix '{}'", from)),
        |range| TextEdit::ReplaceSuffix {
            range,
            replacement: to.trim().to_string(),
        },
    )
}

fn plan_implicit_replacement(buffer: &TranscriptBuffer, replacement: &str) -> TextEdit {
    let replacement = replacement.trim();
    if replacement.is_empty() {
        return TextEdit::Reject("Replacement text is empty".to_string());
    }

    let replacement_words = replacement.split_whitespace().count().max(1);
    let target_words = replacement_words.min(4);
    buffer.last_words_range(target_words).map_or_else(
        || TextEdit::Reject("No recent words to replace".to_string()),
        |range| TextEdit::ReplaceSuffix {
            range,
            replacement: replacement.to_string(),
        },
    )
}

fn validate_edit(buffer: &TranscriptBuffer, edit: TextEdit, policy: EditPolicy) -> TextEdit {
    match &edit {
        TextEdit::DeleteSuffix { range } | TextEdit::ReplaceSuffix { range, .. } => {
            let suffix_end = buffer.text().trim_end_matches(char::is_whitespace).len();
            if range.end != suffix_end {
                return TextEdit::Reject("Edit target is not at the current suffix".to_string());
            }
            let delete_chars = buffer.text()[range.clone()].chars().count();
            if delete_chars > policy.max_delete_chars {
                return TextEdit::Reject(format!(
                    "Refusing to delete {} chars; limit is {}",
                    delete_chars, policy.max_delete_chars
                ));
            }
        }
        _ => {}
    }
    edit
}

pub async fn apply_edit<B: TypingBackend + ?Sized>(
    backend: &B,
    buffer: &mut TranscriptBuffer,
    edit: TextEdit,
    policy: EditPolicy,
) -> Result<()> {
    match edit {
        TextEdit::Insert(text) => {
            backend.type_text(&text).await?;
            buffer.append_typed(&text);
        }
        TextEdit::DeleteSuffix { range } => {
            let deleted_chars = buffer.text()[range.clone()].chars().count();
            backend.backspace(deleted_chars).await?;
            buffer.delete_range(range);
            eprintln!("✂️  Deleted {} chars", deleted_chars);
        }
        TextEdit::ReplaceSuffix { range, replacement } => {
            let deleted_chars = buffer.text()[range.clone()].chars().count();
            backend.backspace(deleted_chars).await?;
            backend.type_text(&replacement).await?;
            buffer.replace_range(range, &replacement);
            eprintln!("✂️  Replaced {} chars", deleted_chars);
        }
        TextEdit::ResetContext => {
            buffer.clear();
            eprintln!("🧹 Dictate context reset");
        }
        TextEdit::Reject(reason) => {
            eprintln!("⚠️  Context edit rejected: {}", reason);
            buffer.record_rejected_command(&reason);
            if policy.type_rejected_commands {
                backend.type_text(&reason).await?;
                buffer.append_typed(&reason);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::{MockTypingBackend, TypingOperation};

    #[tokio::test]
    async fn deletes_last_word() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("hello world");
        let edit = plan_edit(
            &buffer,
            DictationIntent::DeleteLastWords(1),
            EditPolicy::default(),
        );
        apply_edit(&backend, &mut buffer, edit, EditPolicy::default())
            .await
            .unwrap();
        assert_eq!(buffer.text(), "hello ");
        assert_eq!(backend.operations(), vec![TypingOperation::Backspace(5)]);
    }

    #[tokio::test]
    async fn replaces_exact_suffix() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("meeting at five");
        let edit = plan_edit(
            &buffer,
            DictationIntent::ReplaceRecentExact {
                from: "five".into(),
                to: "six".into(),
            },
            EditPolicy::default(),
        );
        apply_edit(&backend, &mut buffer, edit, EditPolicy::default())
            .await
            .unwrap();
        assert_eq!(buffer.text(), "meeting at six");
        assert_eq!(
            backend.operations(),
            vec![
                TypingOperation::Backspace(4),
                TypingOperation::Type("six".into())
            ]
        );
    }

    #[tokio::test]
    async fn rejects_large_delete() {
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("hello world");
        let policy = EditPolicy {
            max_delete_chars: 3,
            ..EditPolicy::default()
        };
        let edit = plan_edit(&buffer, DictationIntent::DeleteLastWords(1), policy);
        assert!(matches!(edit, TextEdit::Reject(_)));
    }

    #[tokio::test]
    async fn deletes_last_sentence() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("Hello world. This is wrong.");
        let edit = plan_edit(
            &buffer,
            DictationIntent::DeleteLastSentence,
            EditPolicy::default(),
        );
        apply_edit(&backend, &mut buffer, edit, EditPolicy::default())
            .await
            .unwrap();
        assert_eq!(buffer.text(), "Hello world. ");
    }

    #[tokio::test]
    async fn implicit_replacement_replaces_suffix_words() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("the meeting is at five");
        let edit = plan_edit(
            &buffer,
            DictationIntent::ReplaceRecentImplicit {
                replacement: "six".into(),
            },
            EditPolicy::default(),
        );
        apply_edit(&backend, &mut buffer, edit, EditPolicy::default())
            .await
            .unwrap();
        assert_eq!(buffer.text(), "the meeting is at six");
    }

    #[tokio::test]
    async fn reset_context_clears_buffer_without_typing() {
        let backend = MockTypingBackend::default();
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("hello");
        let edit = plan_edit(
            &buffer,
            DictationIntent::ResetContext,
            EditPolicy::default(),
        );
        apply_edit(&backend, &mut buffer, edit, EditPolicy::default())
            .await
            .unwrap();
        assert_eq!(buffer.text(), "");
        assert!(backend.operations().is_empty());
    }
}
