use regex::{Captures, RegexBuilder};

pub fn apply_developer_mode(input: &str, mode: &str) -> String {
    let text = apply_developer_vocabulary(input);
    match mode.trim().to_lowercase().replace('_', "-").as_str() {
        "plain" | "default" | "" => text,
        "markdown" | "md" => markdown_mode(&text),
        "git" | "git-commit" | "commit" => git_commit_mode(&text),
        "terminal" | "shell" | "command" => terminal_mode(&text),
        "code" | "code-symbol" | "code-symbols" => code_symbols_mode(&text),
        "file" | "path" | "file-path" | "filepath" => file_path_mode(&text),
        _ => text,
    }
}

fn apply_developer_vocabulary(input: &str) -> String {
    let mut text = input.to_string();
    for (spoken, replacement) in [
        ("pipe wire", "PipeWire"),
        ("way land", "Wayland"),
        ("hyper land", "Hyprland"),
        ("high per land", "Hyprland"),
        ("vox stroll", "Voxtral"),
        ("get hub", "GitHub"),
        ("get lab", "GitLab"),
        ("read me", "README"),
    ] {
        text = replace_phrase_case_insensitive(&text, spoken, replacement);
    }
    text
}

fn replace_phrase_case_insensitive(input: &str, spoken: &str, replacement: &str) -> String {
    let pattern = format!(
        r"(^|[^\p{{L}}\p{{N}}_])({})([^\p{{L}}\p{{N}}_]|$)",
        regex::escape(spoken)
    );
    let Ok(regex) = RegexBuilder::new(&pattern).case_insensitive(true).build() else {
        return input.to_string();
    };

    let mut text = input.to_string();
    loop {
        let next = regex
            .replace_all(&text, |captures: &Captures<'_>| {
                format!("{}{}{}", &captures[1], replacement, &captures[3])
            })
            .into_owned();
        if next == text {
            return text;
        }
        text = next;
    }
}

fn markdown_mode(input: &str) -> String {
    let trimmed = input.trim();
    let lower = trimmed.to_lowercase();

    for (prefix, marker) in [
        ("heading one ", "# "),
        ("heading 1 ", "# "),
        ("h one ", "# "),
        ("heading two ", "## "),
        ("heading 2 ", "## "),
        ("h two ", "## "),
        ("heading three ", "### "),
        ("heading 3 ", "### "),
        ("h three ", "### "),
        ("bullet ", "- "),
        ("dash ", "- "),
        ("quote ", "> "),
    ] {
        if lower.starts_with(prefix) {
            return format!("{}{}", marker, capitalize_first(&trimmed[prefix.len()..]));
        }
    }

    input
        .replace(" new paragraph ", "\n\n")
        .replace(" new line ", "\n")
        .replace(" code block ", "\n```\n")
        .replace(" end code block", "\n```")
}

fn git_commit_mode(input: &str) -> String {
    let trimmed = input.trim().trim_end_matches('.');
    if trimmed.contains(':') {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();
    for (spoken, commit_type) in [
        ("fix ", "fix"),
        ("fixed ", "fix"),
        ("add ", "feat"),
        ("added ", "feat"),
        ("implement ", "feat"),
        ("update ", "chore"),
        ("refactor ", "refactor"),
        ("document ", "docs"),
        ("docs ", "docs"),
        ("test ", "test"),
        ("chore ", "chore"),
    ] {
        if lower.starts_with(spoken) {
            let rest = trimmed[spoken.len()..].trim();
            return format!("{}: {}", commit_type, lowercase_first(rest));
        }
    }

    format!("chore: {}", lowercase_first(trimmed))
}

fn terminal_mode(input: &str) -> String {
    let mut words: Vec<String> = input
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|c: char| matches!(c, ',' | '.' | ';'))
                .to_string()
        })
        .collect();

    let mut output = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index].to_lowercase();
        let next = words.get(index + 1).map(|value| value.to_lowercase());

        match (word.as_str(), next.as_deref()) {
            ("dash", Some("dash")) | ("double", Some("dash")) => {
                output.push("--".to_string());
                index += 2;
            }
            ("pipe", _) => {
                output.push("|".to_string());
                index += 1;
            }
            ("release", _) if output.last().is_some_and(|prev| prev == "build") => {
                output.push("--release".to_string());
                index += 1;
            }
            ("features", _) => {
                output.push("--features".to_string());
                index += 1;
            }
            ("all", _) if output.last().is_some_and(|prev| prev == "git") => {
                output.push("add".to_string());
                output.push(".".to_string());
                index += 1;
            }
            _ => {
                output.push(words[index].clone());
                index += 1;
            }
        }
    }

    words.clear();
    output.join(" ").replace("-- ", "--")
}

fn code_symbols_mode(input: &str) -> String {
    let mut output = Vec::new();
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut index = 0;

    while index < tokens.len() {
        let token = tokens[index].to_lowercase();
        let next = tokens.get(index + 1).map(|value| value.to_lowercase());

        match (token.as_str(), next.as_deref()) {
            ("open", Some("paren" | "parenthesis")) | ("left", Some("paren" | "parenthesis")) => {
                output.push("(".to_string());
                index += 2;
            }
            ("close", Some("paren" | "parenthesis")) | ("right", Some("paren" | "parenthesis")) => {
                output.push(")".to_string());
                index += 2;
            }
            ("open", Some("brace")) | ("left", Some("brace")) => {
                output.push("{".to_string());
                index += 2;
            }
            ("close", Some("brace")) | ("right", Some("brace")) => {
                output.push("}".to_string());
                index += 2;
            }
            ("open", Some("bracket")) | ("left", Some("bracket")) => {
                output.push("[".to_string());
                index += 2;
            }
            ("close", Some("bracket")) | ("right", Some("bracket")) => {
                output.push("]".to_string());
                index += 2;
            }
            ("colon", _) => {
                output.push(":".to_string());
                index += 1;
            }
            ("comma", _) => {
                output.push(",".to_string());
                index += 1;
            }
            ("dot", _) | ("period", _) => {
                output.push(".".to_string());
                index += 1;
            }
            ("arrow", _) => {
                output.push("->".to_string());
                index += 1;
            }
            ("fat", Some("arrow")) => {
                output.push("=>".to_string());
                index += 2;
            }
            ("equals", _) => {
                output.push("=".to_string());
                index += 1;
            }
            ("string", _) => {
                output.push("String".to_string());
                index += 1;
            }
            ("result", _) => {
                output.push("Result".to_string());
                index += 1;
            }
            _ => {
                if index + 1 < tokens.len()
                    && is_identifier_word(tokens[index])
                    && is_identifier_word(tokens[index + 1])
                {
                    output.push(format!("{}_{}", tokens[index], tokens[index + 1]));
                    index += 2;
                } else {
                    output.push(tokens[index].to_string());
                    index += 1;
                }
            }
        }
    }

    join_code_tokens(&output)
}

fn file_path_mode(input: &str) -> String {
    input
        .split_whitespace()
        .map(|token| match token.to_lowercase().as_str() {
            "slash" | "forward-slash" => "/".to_string(),
            "backslash" => "\\".to_string(),
            "dot" | "period" => ".".to_string(),
            "dash" | "hyphen" => "-".to_string(),
            "underscore" => "_".to_string(),
            "tilde" => "~".to_string(),
            "home" => "~".to_string(),
            _ => token.to_string(),
        })
        .collect::<Vec<_>>()
        .join("")
}

fn join_code_tokens(tokens: &[String]) -> String {
    let mut output = String::new();
    for token in tokens {
        match token.as_str() {
            ")" | "]" | "}" | "," | "." | ":" => {
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push_str(token);
                output.push(' ');
            }
            "(" | "[" | "{" => {
                if !output.is_empty() && !output.ends_with(' ') {
                    output.push(' ');
                }
                output.push_str(token);
            }
            "->" | "=>" | "=" => {
                while output.ends_with(' ') {
                    output.pop();
                }
                output.push(' ');
                output.push_str(token);
                output.push(' ');
            }
            _ => {
                if !output.is_empty() && !output.ends_with([' ', '(', '[', '{']) {
                    output.push(' ');
                }
                output.push_str(token);
            }
        }
    }

    output.trim().to_string()
}

fn is_identifier_word(token: &str) -> bool {
    token.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
        && !matches!(
            token.to_lowercase().as_str(),
            "open" | "close" | "left" | "right" | "colon" | "comma" | "dot" | "arrow" | "equals"
        )
}

fn capitalize_first(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn lowercase_first(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_heading() {
        assert_eq!(
            apply_developer_mode("heading two install", "markdown"),
            "## Install"
        );
    }

    #[test]
    fn git_commit_conventionalizes() {
        assert_eq!(
            apply_developer_mode("fix release audio device", "git-commit"),
            "fix: release audio device"
        );
    }

    #[test]
    fn terminal_command_adds_flags() {
        assert_eq!(
            apply_developer_mode("cargo build release features local", "terminal"),
            "cargo build --release --features local"
        );
    }

    #[test]
    fn code_symbols_are_expanded() {
        assert_eq!(
            apply_developer_mode(
                "open paren user id colon string close paren arrow result",
                "code-symbols"
            ),
            "(user_id: String) -> Result"
        );
    }

    #[test]
    fn file_paths_are_compacted() {
        assert_eq!(
            apply_developer_mode(
                "home slash projects slash dictate slash read me dot md",
                "file-path"
            ),
            "~/projects/dictate/README.md"
        );
    }

    #[test]
    fn developer_vocabulary_applies_to_modes() {
        assert_eq!(
            apply_developer_mode("install pipe wire", "plain"),
            "install PipeWire"
        );
    }
}
