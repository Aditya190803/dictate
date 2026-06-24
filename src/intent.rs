#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationIntent {
    InsertText(String),
    DeleteLastSentence,
    DeleteLastLine,
    DeleteLastWords(usize),
    ReplaceRecentExact { from: String, to: String },
    ReplaceRecentImplicit { replacement: String },
    ResetContext,
    CommandRejected { reason: String },
}

pub fn detect_intent(text: &str) -> DictationIntent {
    let normalized = normalize_command(text);
    let words: Vec<&str> = normalized.split_whitespace().collect();

    if normalized == "reset dictate context" || normalized == "clear dictate context" {
        return DictationIntent::ResetContext;
    }

    if matches!(
        normalized.as_str(),
        "scratch that" | "forget that" | "remove that" | "delete that"
    ) || normalized.contains("remove last sentence")
        || normalized.contains("delete last sentence")
        || normalized.contains("remove that sentence")
        || normalized.contains("delete that sentence")
    {
        return DictationIntent::DeleteLastSentence;
    }

    if normalized.contains("delete last line")
        || normalized.contains("remove last line")
        || normalized.contains("remove that last line")
        || normalized.contains("delete that last line")
        || normalized.contains("scratch that line")
    {
        return DictationIntent::DeleteLastLine;
    }

    if let Some(count) = parse_delete_words(&words) {
        return DictationIntent::DeleteLastWords(count);
    }

    if let Some(intent) = parse_explicit_replacement(&words) {
        return intent;
    }

    if let Some(replacement) = normalized.strip_prefix("actually ") {
        return replacement_intent(replacement);
    }
    if let Some(replacement) = normalized.strip_prefix("no i mean ") {
        return replacement_intent(replacement);
    }
    if let Some(replacement) = normalized.strip_prefix("i mean ") {
        return replacement_intent(replacement);
    }
    if let Some(replacement) = normalized.strip_prefix("no ") {
        return replacement_intent(replacement);
    }

    DictationIntent::InsertText(text.to_string())
}

fn replacement_intent(replacement: &str) -> DictationIntent {
    let replacement = replacement.trim();
    if replacement.is_empty() {
        DictationIntent::CommandRejected {
            reason: "Replacement text is empty".to_string(),
        }
    } else {
        DictationIntent::ReplaceRecentImplicit {
            replacement: replacement.to_string(),
        }
    }
}

fn parse_delete_words(words: &[&str]) -> Option<usize> {
    if words.len() < 3 {
        return None;
    }
    if !matches!(words[0], "delete" | "remove") || words[1] != "last" {
        return None;
    }
    if words[2] == "word" || words[2] == "words" {
        return Some(1);
    }
    let count = parse_count(words[2])?;
    if words
        .get(3)
        .is_some_and(|word| *word == "word" || *word == "words")
    {
        Some(count)
    } else {
        None
    }
}

fn parse_explicit_replacement(words: &[&str]) -> Option<DictationIntent> {
    let (verb, separator) = match words.first().copied()? {
        "replace" => ("replace", "with"),
        "change" => ("change", "to"),
        _ => return None,
    };
    let separator_index = words.iter().position(|word| *word == separator)?;
    if separator_index <= 1 || separator_index + 1 >= words.len() {
        return Some(DictationIntent::CommandRejected {
            reason: format!("Incomplete {} command", verb),
        });
    }
    Some(DictationIntent::ReplaceRecentExact {
        from: words[1..separator_index].join(" "),
        to: words[separator_index + 1..].join(" "),
    })
}

fn parse_count(word: &str) -> Option<usize> {
    word.parse::<usize>().ok().or(match word {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => None,
    })
}

fn normalize_command(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_punctuation() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .filter(|word| !matches!(*word, "sorry" | "please" | "uh" | "um"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_delete_commands() {
        assert_eq!(
            detect_intent("sorry remove that last line"),
            DictationIntent::DeleteLastLine
        );
        assert_eq!(
            detect_intent("scratch that"),
            DictationIntent::DeleteLastSentence
        );
        assert_eq!(
            detect_intent("delete last two words"),
            DictationIntent::DeleteLastWords(2)
        );
        assert_eq!(
            detect_intent("delete last 5 words"),
            DictationIntent::DeleteLastWords(5)
        );
    }

    #[test]
    fn detects_replacements() {
        assert_eq!(
            detect_intent("replace five with six"),
            DictationIntent::ReplaceRecentExact {
                from: "five".into(),
                to: "six".into()
            }
        );
        assert_eq!(
            detect_intent("change monday to tuesday"),
            DictationIntent::ReplaceRecentExact {
                from: "monday".into(),
                to: "tuesday".into()
            }
        );
        assert_eq!(
            detect_intent("no I mean Friday"),
            DictationIntent::ReplaceRecentImplicit {
                replacement: "friday".into()
            }
        );
    }
}
