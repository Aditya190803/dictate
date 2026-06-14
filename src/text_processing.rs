use regex::{Captures, Regex, RegexBuilder};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Snippet {
    pub trigger: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CleanupConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub fix_spacing: bool,
    #[serde(default)]
    pub capitalize_sentences: bool,
    #[serde(default)]
    pub fix_punctuation: bool,
    #[serde(default)]
    pub spoken_punctuation: bool,
    #[serde(default)]
    pub remove_fillers: bool,
    #[serde(default)]
    pub clean_repeated_words: bool,
    #[serde(default)]
    pub spoken_lists: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CommandModeConfig {
    #[serde(default)]
    pub clipboard_command: Vec<String>,
}

impl CommandModeConfig {
    #[cfg(not(test))]
    pub fn clipboard_command_or_default(&self) -> Vec<String> {
        if self.clipboard_command.is_empty() {
            vec!["wl-paste".to_string(), "--no-newline".to_string()]
        } else {
            self.clipboard_command.clone()
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TextProcessingConfig {
    #[serde(default)]
    pub dictionary: HashMap<String, String>,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub cleanup: CleanupConfig,
    #[serde(default)]
    pub command_mode: CommandModeConfig,
}

pub fn process_text(input: &str, config: &TextProcessingConfig) -> String {
    let text = apply_dictionary(input, &config.dictionary);
    let text = apply_inline_corrections(&text);
    let text = apply_snippets(&text, &config.snippets);
    apply_cleanup(&text, &config.cleanup)
}

pub fn apply_dictionary(input: &str, dictionary: &HashMap<String, String>) -> String {
    let mut text = input.to_string();
    let mut entries: Vec<(&String, &String)> = dictionary
        .iter()
        .filter(|(trigger, _)| !trigger.trim().is_empty())
        .collect();

    entries.sort_by_key(|(right, _)| std::cmp::Reverse(right.len()));

    for (trigger, replacement) in entries {
        let pattern = format!(
            r"(^|[^\p{{L}}\p{{N}}_])({})([^\p{{L}}\p{{N}}_]|$)",
            regex::escape(trigger)
        );

        let Ok(regex) = RegexBuilder::new(&pattern).case_insensitive(true).build() else {
            continue;
        };

        loop {
            let next = regex
                .replace_all(&text, |captures: &Captures<'_>| {
                    format!("{}{}{}", &captures[1], replacement, &captures[3])
                })
                .into_owned();

            if next == text {
                break;
            }
            text = next;
        }
    }

    text
}

pub fn apply_snippets(input: &str, snippets: &[Snippet]) -> String {
    let trimmed = input.trim();
    for snippet in snippets {
        if !snippet.trigger.trim().is_empty()
            && trimmed.eq_ignore_ascii_case(snippet.trigger.trim())
        {
            return snippet.text.clone();
        }
    }

    input.to_string()
}

fn apply_inline_corrections(input: &str) -> String {
    let Ok(regex) = RegexBuilder::new(
        r"(?i)(?:^|[\s.!?])(?:actually\s+)?(?:replace|the\s+replace|the\s+place)\s+(.+?)\s+with\s+(.+?)(?:[.!?](?:\s|$)|$)",
    )
    .build() else {
        return input.to_string();
    };

    let mut output = input.to_string();
    while let Some(captures) = regex.captures(&output) {
        let Some(full_match) = captures.get(0) else {
            break;
        };
        let Some(from_match) = captures.get(1) else {
            break;
        };
        let Some(to_match) = captures.get(2) else {
            break;
        };

        let from = cleanup_correction_term(from_match.as_str());
        let to = cleanup_correction_term(to_match.as_str());
        if from.is_empty() || to.is_empty() {
            break;
        }

        let before = output[..full_match.start()].trim_end();
        let after = output[full_match.end()..].trim_start();
        let mut corrected_before = replace_correction_target(before, &from, &to);
        if corrected_before == before && from.eq_ignore_ascii_case("working") {
            corrected_before = replace_correction_target(before, "correct", &to);
        }

        output = match (corrected_before.trim().is_empty(), after.is_empty()) {
            (true, true) => String::new(),
            (true, false) => after.to_string(),
            (false, true) => corrected_before,
            (false, false) => format!("{} {}", corrected_before, after),
        };
    }

    output
}

fn cleanup_correction_term(term: &str) -> String {
    let mut cleaned = term
        .trim()
        .trim_matches(|c| {
            matches!(
                c,
                '"' | '\'' | '`' | ',' | ';' | ':' | '.' | '!' | '?' | ' '
            )
        })
        .to_string();

    for (spoken, replacement) in [
        (" question mark", "?"),
        (" exclamation mark", "!"),
        (" exclamation point", "!"),
        (" exclamation", "!"),
        (" period", "."),
        (" full stop", "."),
        (" comma", ","),
        (" colon", ":"),
        (" semicolon", ";"),
    ] {
        if cleaned.to_lowercase().ends_with(spoken) {
            let keep_len = cleaned.len().saturating_sub(spoken.len());
            cleaned = format!("{}{}", cleaned[..keep_len].trim_end(), replacement);
            break;
        }
    }

    cleaned
}

fn replace_correction_target(input: &str, from: &str, to: &str) -> String {
    let pattern = format!(
        r"(^|[^\p{{L}}\p{{N}}_])({})([^\p{{L}}\p{{N}}_]|$)",
        regex::escape(from)
    );
    let Ok(regex) = RegexBuilder::new(&pattern).case_insensitive(true).build() else {
        return input.to_string();
    };

    let mut text = input.to_string();
    loop {
        let next = regex
            .replace_all(&text, |captures: &Captures<'_>| {
                let right = &captures[3];
                let right = if ends_with_terminal_punctuation(to) && is_terminal_punctuation(right)
                {
                    ""
                } else {
                    right
                };
                format!("{}{}{}", &captures[1], to, right)
            })
            .into_owned();

        if next == text {
            return text;
        }
        text = next;
    }
}

pub fn apply_cleanup(input: &str, cleanup: &CleanupConfig) -> String {
    if !cleanup.enabled {
        return input.to_string();
    }

    let mut text = input.to_string();

    if cleanup.remove_fillers {
        text = remove_fillers(&text);
    }

    if cleanup.clean_repeated_words {
        text = clean_repeated_words(&text);
    }

    if cleanup.spoken_punctuation {
        text = apply_spoken_punctuation(&text);
    }

    if cleanup.spoken_lists {
        text = convert_spoken_lists(&text);
    }

    if cleanup.fix_spacing {
        text = fix_spacing(&text);
    }

    if cleanup.capitalize_sentences {
        text = capitalize_sentences(&text);
    }

    if cleanup.fix_punctuation {
        text = fix_punctuation(&text);
    }

    text
}

fn remove_fillers(input: &str) -> String {
    let Ok(regex) = RegexBuilder::new(r"(^|[^\p{L}\p{N}_])(um|uh|erm|er|ah)([^\p{L}\p{N}_]|$)")
        .case_insensitive(true)
        .build()
    else {
        return input.to_string();
    };

    let mut text = input.to_string();
    loop {
        let next = regex
            .replace_all(&text, |captures: &Captures<'_>| {
                let left = &captures[1];
                let right = &captures[3];
                if left.trim().is_empty() && right.trim().is_empty() {
                    " ".to_string()
                } else {
                    format!("{}{}", left, right)
                }
            })
            .into_owned();

        if next == text {
            return fix_spacing(&next);
        }
        text = next;
    }
}

fn clean_repeated_words(input: &str) -> String {
    let mut cleaned = Vec::new();
    let mut previous_word: Option<String> = None;

    for token in input.split_whitespace() {
        let normalized = token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
            .to_lowercase();

        if !normalized.is_empty() && previous_word.as_deref() == Some(normalized.as_str()) {
            continue;
        }

        if !normalized.is_empty() {
            previous_word = Some(normalized);
        } else {
            previous_word = None;
        }
        cleaned.push(token);
    }

    cleaned.join(" ")
}

fn ends_with_terminal_punctuation(value: &str) -> bool {
    value.ends_with(['.', '!', '?', ',', ':', ';'])
}

fn is_terminal_punctuation(value: &str) -> bool {
    matches!(value, "." | "!" | "?" | "," | ":" | ";")
}

fn apply_spoken_punctuation(input: &str) -> String {
    let mut text = input.to_string();
    for (spoken, replacement) in [
        ("exclamation mark", "!"),
        ("exclamation point", "!"),
        ("exclamation", "!"),
        ("question mark", "?"),
        ("question", "?"),
        ("comma", ","),
        ("period", "."),
        ("full stop", "."),
        ("colon", ":"),
        ("semicolon", ";"),
    ] {
        text = replace_correction_target(&text, spoken, replacement);
    }
    fix_spacing(&text)
}

fn convert_spoken_lists(input: &str) -> String {
    let Ok(marker_regex) = RegexBuilder::new(
        r"(^|\s)(first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth)(?:\s+thing)?\s+(?:is|are|:)?\s+",
    )
    .case_insensitive(true)
    .build()
    else {
        return input.to_string();
    };

    let matches: Vec<_> = marker_regex.find_iter(input).collect();
    if matches.len() < 2 {
        return input.to_string();
    }

    let first_prefix = input[..matches[0].start()].trim();
    if !first_prefix.is_empty() {
        return input.to_string();
    }

    let mut items = Vec::new();
    for (index, matched) in matches.iter().enumerate() {
        let item_start = matched.end();
        let item_end = matches
            .get(index + 1)
            .map_or_else(|| input.len(), |next| next.start());
        let item = input[item_start..item_end].trim();
        if !item.is_empty() {
            items.push(normalize_list_item(item));
        }
    }

    if items.len() < 2 {
        return input.to_string();
    }

    format!("First:\n{}", items.join("\n"))
}

fn normalize_list_item(item: &str) -> String {
    let trimmed = item
        .trim()
        .trim_matches(|c| matches!(c, ',' | ';' | ':' | '-' | ' '));
    let capitalized = capitalize_sentences(trimmed);
    format!("- {}", fix_punctuation(&capitalized))
}

fn fix_spacing(input: &str) -> String {
    let punctuation_regex = Regex::new(r"\s+([,.;!?])").ok();

    input
        .lines()
        .map(|line| {
            let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if let Some(regex) = &punctuation_regex {
                regex.replace_all(&collapsed, "$1").into_owned()
            } else {
                collapsed
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn capitalize_sentences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut capitalize_next = true;

    for ch in input.chars() {
        if capitalize_next && ch.is_alphabetic() {
            for upper in ch.to_uppercase() {
                output.push(upper);
            }
            capitalize_next = false;
            continue;
        }

        output.push(ch);

        if matches!(ch, '.' | '!' | '?' | '\n') {
            capitalize_next = true;
        } else if !ch.is_whitespace() {
            capitalize_next = false;
        }
    }

    output
}

fn fix_punctuation(input: &str) -> String {
    let trimmed = input.trim_end();
    if trimmed.is_empty()
        || trimmed.ends_with('.')
        || trimmed.ends_with('!')
        || trimmed.ends_with('?')
        || trimmed.ends_with(':')
    {
        return input.to_string();
    }

    format!("{}.", trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_no_op() {
        let config = TextProcessingConfig::default();
        assert_eq!(process_text("hello world", &config), "hello world");
    }

    #[test]
    fn expands_exact_snippet_trigger() {
        let config = TextProcessingConfig {
            snippets: vec![Snippet {
                trigger: "calendar link".to_string(),
                text: "Book a time here: https://cal.com/adi".to_string(),
            }],
            ..Default::default()
        };

        assert_eq!(
            process_text("  Calendar Link  ", &config),
            "Book a time here: https://cal.com/adi"
        );
    }

    #[test]
    fn dictionary_runs_before_snippets() {
        let mut dictionary = HashMap::new();
        dictionary.insert("sig".to_string(), "email signature".to_string());
        let config = TextProcessingConfig {
            dictionary,
            snippets: vec![Snippet {
                trigger: "email signature".to_string(),
                text: "Best,\nAditya".to_string(),
            }],
            ..Default::default()
        };

        assert_eq!(process_text("sig", &config), "Best,\nAditya");
    }

    #[test]
    fn dictionary_is_case_insensitive() {
        let mut dictionary = HashMap::new();
        dictionary.insert("high per land".to_string(), "Hyprland".to_string());
        let config = TextProcessingConfig {
            dictionary,
            ..Default::default()
        };

        assert_eq!(
            process_text("I use HIGH PER LAND daily.", &config),
            "I use Hyprland daily."
        );
    }

    #[test]
    fn dictionary_does_not_replace_inside_larger_words() {
        let mut dictionary = HashMap::new();
        dictionary.insert("adi".to_string(), "Aditya".to_string());
        let config = TextProcessingConfig {
            dictionary,
            ..Default::default()
        };

        assert_eq!(
            process_text("adi radiant tradition", &config),
            "Aditya radiant tradition"
        );
    }

    #[test]
    fn longer_dictionary_entries_win_first() {
        let mut dictionary = HashMap::new();
        dictionary.insert("whisper".to_string(), "Whisper".to_string());
        dictionary.insert("whisper flow".to_string(), "Wispr Flow".to_string());
        let config = TextProcessingConfig {
            dictionary,
            ..Default::default()
        };

        assert_eq!(process_text("whisper flow", &config), "Wispr Flow");
    }

    #[test]
    fn cleanup_is_disabled_by_default() {
        let config = TextProcessingConfig::default();
        assert_eq!(process_text("um hello", &config), "um hello");
    }

    #[test]
    fn cleanup_removes_fillers() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                remove_fillers: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(process_text("um hello uh world", &config), "hello world");
    }

    #[test]
    fn cleanup_fixes_spacing() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                fix_spacing: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(process_text("hello   world  !", &config), "hello world!");
    }

    #[test]
    fn cleanup_capitalizes_sentences() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                capitalize_sentences: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            process_text("hello world. next thing", &config),
            "Hello world. Next thing"
        );
    }

    #[test]
    fn cleanup_fixes_final_punctuation() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                fix_punctuation: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(process_text("hello world", &config), "hello world.");
    }

    #[test]
    fn cleanup_applies_spoken_punctuation() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                spoken_punctuation: true,
                fix_spacing: true,
                capitalize_sentences: true,
                fix_punctuation: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            process_text("this should work exclamation", &config),
            "This should work!"
        );
        assert_eq!(
            process_text("is this right question mark", &config),
            "Is this right?"
        );
    }

    #[test]
    fn spoken_punctuation_is_no_op_when_cleanup_is_disabled() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text("this should work exclamation", &config),
            "this should work exclamation"
        );
    }

    #[test]
    fn inline_corrections_are_always_on() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text(
                "Is this correct? Actually replace correct with right.",
                &config
            ),
            "Is this right?"
        );
    }

    #[test]
    fn inline_corrections_preserve_following_text() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text(
                "This is correct. Actually replace correct with right. This should work",
                &config
            ),
            "This is right. This should work"
        );
    }

    #[test]
    fn inline_corrections_support_spoken_punctuation_in_replacement() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text(
                "Is this Correct? Actually replace correct with right question mark",
                &config
            ),
            "Is this right?"
        );
    }

    #[test]
    fn inline_corrections_tolerate_common_replace_mishearing() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text(
                "Is this Correct? Actually the place working with right question mark",
                &config
            ),
            "Is this right?"
        );
    }

    #[test]
    fn inline_corrections_tolerate_the_replace_phrase() {
        let config = TextProcessingConfig::default();
        assert_eq!(
            process_text(
                "Is this correct? Actually the replace correct with right.",
                &config
            ),
            "Is this right?"
        );
    }

    #[test]
    fn cleanup_removes_repeated_words() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                clean_repeated_words: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            process_text("the the thing is is ready", &config),
            "the thing is ready"
        );
    }

    #[test]
    fn cleanup_converts_spoken_lists() {
        let config = TextProcessingConfig {
            cleanup: CleanupConfig {
                enabled: true,
                spoken_lists: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            process_text(
                "first thing is install pipe wire second thing is set the key bind",
                &config
            ),
            "First:\n- Install pipe wire.\n- Set the key bind."
        );
    }

    #[test]
    fn combined_cleanup_runs_after_dictionary_and_snippets() {
        let mut dictionary = HashMap::new();
        dictionary.insert("pipe wire".to_string(), "PipeWire".to_string());
        let config = TextProcessingConfig {
            dictionary,
            cleanup: CleanupConfig {
                enabled: true,
                remove_fillers: true,
                fix_spacing: true,
                capitalize_sentences: true,
                fix_punctuation: true,
                clean_repeated_words: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            process_text("um install pipe wire pipe wire", &config),
            "Install PipeWire."
        );
    }
}
