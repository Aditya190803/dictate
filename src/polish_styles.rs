//! Named polish tone presets (Flow-style), merged into Mistral system prompt.

/// Extra system instructions for `[polish].style` in text.toml.
pub fn style_system_suffix(style: &str) -> Option<&'static str> {
    match style.trim().to_lowercase().as_str() {
        "" | "default" | "neutral" => None,
        "casual" | "friendly" => Some(
            "Tone: casual and friendly. Prefer contractions and plain words; stay professional enough for work chat.",
        ),
        "formal" | "professional" => Some(
            "Tone: formal and professional. Complete sentences, no slang, suitable for email or docs.",
        ),
        "concise" | "brief" => Some(
            "Tone: concise. Remove filler and redundancy; keep only necessary detail.",
        ),
        "email" => Some(
            "Format: polished email body. Greeting and sign-off only if the speaker implied them.",
        ),
        "bullets" | "bullet" => Some(
            "Format: use bullet lines when the speaker lists items; otherwise short paragraphs.",
        ),
        _ => None,
    }
}

pub fn apply_style_to_system(base: &str, style: &str) -> String {
    match style_system_suffix(style) {
        Some(suffix) => format!("{base}\n\n{suffix}"),
        None => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casual_adds_suffix() {
        let out = apply_style_to_system("base", "casual");
        assert!(out.contains("casual"));
        assert!(out.starts_with("base"));
    }
}
