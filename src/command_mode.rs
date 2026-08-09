#[cfg(not(test))]
use crate::command;
use crate::text_processing::{apply_cleanup, CleanupConfig, CommandModeConfig};
use anyhow::{anyhow, Result};

pub async fn run_command_mode(
    instruction: &str,
    source_text: Option<&str>,
    config: &CommandModeConfig,
    full_config: Option<&crate::config::Config>,
) -> Result<String> {
    let source = match source_text {
        Some(text) => text.to_string(),
        None => read_clipboard(config).await?,
    };

    match transform_text(instruction, &source) {
        Ok(out) => Ok(out),
        Err(local_err) => {
            let Some(cfg) = full_config else {
                return Err(local_err);
            };
            if !cfg.command_mode_uses_llm() {
                return Err(local_err);
            }
            crate::llm_polish::transform_with_llm(
                instruction,
                &source,
                cfg,
                &cfg.text_processing.polish,
            )
            .await
        }
    }
}

/// Read clipboard without failing the session (for auto-detect).
pub async fn peek_clipboard_text(config: &CommandModeConfig) -> Option<String> {
    #[cfg(test)]
    {
        let _ = config;
        None
    }
    #[cfg(not(test))]
    {
        read_clipboard(config)
            .await
            .ok()
            .map(|s| s.trim().to_string())
    }
}

#[cfg(test)]
async fn read_clipboard(_config: &CommandModeConfig) -> Result<String> {
    Err(anyhow!("clipboard unavailable in tests"))
}

#[cfg(not(test))]
async fn read_clipboard(config: &CommandModeConfig) -> Result<String> {
    let command_args = config.clipboard_command_or_default();
    command::execute_capture(&command_args).await.map_err(|e| {
        anyhow!(
            "Failed to read clipboard using {:?}: {}. {}",
            command_args,
            e,
            crate::platform::CLIPBOARD_HINT
        )
    })
}

pub fn transform_text(instruction: &str, source: &str) -> Result<String> {
    let normalized = instruction.trim().to_lowercase();

    if normalized.contains("bullet") || normalized.contains("list") {
        return Ok(turn_into_bullet_points(source));
    }

    if normalized.contains("concise") || normalized.contains("shorter") {
        return Ok(make_concise(source));
    }

    if normalized.contains("casual") || normalized.contains("friendly") {
        return Ok(rewrite_casually(source));
    }

    if normalized.contains("summarize") || normalized.contains("summary") {
        return Ok(summarize(source));
    }

    if normalized.contains("grammar")
        || normalized.contains("punctuation")
        || normalized.contains("fix")
    {
        return Ok(fix_grammar(source));
    }

    Err(anyhow!(
        "Unsupported command mode instruction: '{}'. Supported local commands: fix grammar, turn this into bullet points, make this more concise, rewrite casually, summarize this paragraph.",
        instruction.trim()
    ))
}

fn fix_grammar(source: &str) -> String {
    apply_cleanup(
        source,
        &CleanupConfig {
            enabled: true,
            fix_spacing: true,
            capitalize_sentences: true,
            fix_punctuation: true,
            spoken_punctuation: true,
            remove_fillers: true,
            clean_repeated_words: true,
            spoken_lists: true,
        },
    )
}

fn turn_into_bullet_points(source: &str) -> String {
    let mut items = Vec::new();

    for line in source.lines() {
        for sentence in split_sentences(line) {
            let item = sentence
                .trim()
                .trim_matches(|c| matches!(c, '-' | '•' | ' '));
            if item.is_empty() {
                continue;
            }
            let cleaned = apply_cleanup(
                item,
                &CleanupConfig {
                    enabled: true,
                    fix_spacing: true,
                    capitalize_sentences: true,
                    fix_punctuation: false,
                    spoken_punctuation: true,
                    remove_fillers: true,
                    clean_repeated_words: true,
                    spoken_lists: false,
                },
            );
            items.push(format!("- {}", cleaned.trim_end_matches('.')));
        }
    }

    if items.is_empty() {
        source.to_string()
    } else {
        items.join("\n")
    }
}

fn make_concise(source: &str) -> String {
    let mut text = apply_cleanup(
        source,
        &CleanupConfig {
            enabled: true,
            fix_spacing: true,
            capitalize_sentences: true,
            fix_punctuation: true,
            spoken_punctuation: true,
            remove_fillers: true,
            clean_repeated_words: true,
            spoken_lists: false,
        },
    );

    for phrase in [
        "I think ",
        "Basically, ",
        "Basically ",
        "Actually, ",
        "Actually ",
        "In order to ",
        "Just ",
    ] {
        text = text.replace(phrase, "");
        text = text.replace(&phrase.to_lowercase(), "");
    }

    apply_cleanup(
        &text,
        &CleanupConfig {
            enabled: true,
            fix_spacing: true,
            capitalize_sentences: true,
            fix_punctuation: true,
            spoken_punctuation: true,
            remove_fillers: false,
            clean_repeated_words: true,
            spoken_lists: false,
        },
    )
}

fn rewrite_casually(source: &str) -> String {
    let mut text = fix_grammar(source);
    for (formal, casual) in [
        ("Do not", "Don't"),
        ("do not", "don't"),
        ("Cannot", "Can't"),
        ("cannot", "can't"),
        ("Will not", "Won't"),
        ("will not", "won't"),
        ("It is", "It's"),
        ("it is", "it's"),
        ("You are", "You're"),
        ("you are", "you're"),
        ("We are", "We're"),
        ("we are", "we're"),
    ] {
        text = text.replace(formal, casual);
    }
    text
}

fn summarize(source: &str) -> String {
    let sentences = split_sentences(source);
    if sentences.is_empty() {
        return source.trim().to_string();
    }

    let first = sentences[0].trim();
    let key = sentences
        .iter()
        .skip(1)
        .filter(|sentence| sentence.split_whitespace().count() >= 4)
        .min_by_key(|sentence| sentence.split_whitespace().count());

    let summary = if let Some(key_sentence) = key {
        if key_sentence.trim() == first {
            first.to_string()
        } else {
            format!("{} {}", first.trim_end_matches('.'), key_sentence.trim())
        }
    } else {
        first.to_string()
    };

    fix_grammar(&summary)
}

fn split_sentences(source: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in source.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }

    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_grammar_locally() {
        assert_eq!(
            transform_text("fix grammar", "um hello  world").unwrap(),
            "Hello world."
        );
    }

    #[test]
    fn turns_text_into_bullets() {
        assert_eq!(
            transform_text(
                "turn this into bullet points",
                "Install PipeWire. Set the keybind."
            )
            .unwrap(),
            "- Install PipeWire\n- Set the keybind"
        );
    }

    #[test]
    fn makes_text_concise() {
        assert_eq!(
            transform_text(
                "make this more concise",
                "I think basically we should just ship this"
            )
            .unwrap(),
            "We should ship this."
        );
    }

    #[test]
    fn rewrites_casually() {
        assert_eq!(
            transform_text("rewrite casually", "You are not required to do this").unwrap(),
            "You're not required to do this."
        );
    }

    #[test]
    fn summarizes_text() {
        assert_eq!(
            transform_text(
                "summarize this paragraph",
                "Dictate is a local speech tool. It works on Wayland. It supports providers."
            )
            .unwrap(),
            "Dictate is a local speech tool It works on Wayland."
        );
    }

    #[test]
    fn rejects_unknown_instruction() {
        assert!(transform_text("dance", "hello").is_err());
    }
}
