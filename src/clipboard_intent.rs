//! One shortcut: treat speech as a clipboard command when context matches.

use crate::intent::detect_intent;
use crate::intent::DictationIntent;

/// Min clipboard size to consider "user copied something to edit".
const MIN_CLIPBOARD_CHARS: usize = 12;

/// Heuristic: short utterance about changing text, not dictating a paragraph.
pub fn looks_like_command_instruction(transcript: &str) -> bool {
    let t = transcript.trim();
    if t.is_empty() {
        return false;
    }
    let words: Vec<_> = t.split_whitespace().collect();
    if words.len() > 28 {
        return false;
    }
    let lower = t.to_lowercase();
    let normalized = lower
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_string();
    let first = words
        .first()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_lowercase()
        })
        .unwrap_or_default();

    const COMMAND_STARTS: &[&str] = &[
        "fix",
        "rewrite",
        "summarize",
        "translate",
        "rephrase",
        "polish",
        "format",
        "simplify",
        "expand",
        "shorten",
        "correct",
        "improve",
        "proofread",
    ];
    if COMMAND_STARTS.contains(&first.as_str()) {
        return true;
    }

    const EXPLICIT_PHRASES: &[&str] = &[
        "fix grammar",
        "fix the grammar",
        "make this",
        "make that",
        "turn this",
        "turn that",
        "turn it",
        "more concise",
        "shorter",
        "into bullet",
        "bullet points",
        "as bullets",
        "rewrite this",
        "rewrite that",
        "summarize this",
        "summarize that",
        "summarize the",
        "edit this",
        "edit that",
        "format this",
        "format that",
        "proofread this",
        "proofread that",
    ];
    if EXPLICIT_PHRASES
        .iter()
        .any(|phrase| normalized.contains(phrase))
    {
        return true;
    }

    false
}

/// Voice correction ("scratch that") is never a clipboard command.
pub fn is_voice_edit_command(transcript: &str) -> bool {
    !matches!(detect_intent(transcript), DictationIntent::InsertText(_))
}

/// Auto: clipboard has content + speech sounds like an instruction.
pub fn should_apply_clipboard_command(transcript: &str, clipboard: &str) -> bool {
    if is_voice_edit_command(transcript) {
        return false;
    }
    let clip = clipboard.trim();
    if clip.len() < MIN_CLIPBOARD_CHARS {
        return false;
    }
    looks_like_command_instruction(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_instruction_phrases() {
        assert!(looks_like_command_instruction("make this more concise"));
        assert!(looks_like_command_instruction("please fix grammar"));
        assert!(looks_like_command_instruction(
            "turn this into bullet points"
        ));
        assert!(!looks_like_command_instruction(
            "The quarterly report shows revenue increased across all regions and we should schedule a follow-up meeting with the finance team next Tuesday"
        ));
        assert!(!looks_like_command_instruction(
            "Is this working? Hello? Hello?"
        ));
    }

    #[test]
    fn scratch_not_clipboard_command() {
        assert!(!should_apply_clipboard_command(
            "scratch that",
            "some long clipboard text here ok"
        ));
    }

    #[test]
    fn needs_clipboard_and_instruction() {
        assert!(should_apply_clipboard_command(
            "fix grammar",
            "hello world this is a test"
        ));
        assert!(!should_apply_clipboard_command("fix grammar", "short"));
        assert!(!should_apply_clipboard_command(
            "hello world dictation only",
            "some long clipboard text here ok"
        ));
        assert!(!should_apply_clipboard_command(
            "Is this working? Hello? Hello?",
            "some long clipboard text here ok"
        ));
    }
}
