//! Mistral chat completions to frame and polish whole utterances (smart paste).

#![cfg_attr(test, allow(dead_code))]

use crate::config::Config;
use crate::text_processing::PolishConfig;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_SYSTEM: &str = r#"You receive a raw speech-to-text transcript. Output ONLY the final text to insert into the focused application.

Rules:
- Apply the speaker's latest correction when they contradict themselves (e.g. "by 130 no actually by two" → "I will reach by 2.").
- Honor spoken layout: "next line", "new paragraph", "bullet", "point one/two" become real newlines or list formatting.
- Fix grammar, punctuation, and sentence boundaries; preserve meaning.
- Do not add quotes, preamble, or markdown fences unless the dictation mode is markdown.
"#;

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

pub async fn polish_transcript(
    raw_after_local: &str,
    config: &Config,
    polish: &PolishConfig,
    dictation_mode: &str,
) -> Result<String> {
    if !polish.effective_enabled(config.profile.uses_llm_polish()) {
        return Ok(raw_after_local.to_string());
    }

    let api_key = config
        .mistral_api_key
        .as_ref()
        .ok_or_else(|| anyhow!("MISTRAL_API_KEY required for smart paste polish"))?;

    let base = config
        .mistral_base_url
        .clone()
        .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());

    let url = format!("{}/chat/completions", base.trim_end_matches('/'));

    let system = if polish.system_prompt.trim().is_empty() {
        DEFAULT_SYSTEM.to_string()
    } else {
        polish.system_prompt.clone()
    };

    let user_body = format!(
        "Dictation mode: {dictation_mode}\nLanguage: {}\n\nRaw transcript:\n{raw_after_local}",
        config.transcription_language
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.transcription_timeout_seconds))
        .build()?;

    let mut last_err = None;
    let attempts = config.transcription_max_retries.max(1);

    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
        }

        let body = serde_json::json!({
            "model": polish.model,
            "temperature": polish.temperature,
            "max_tokens": polish.max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user_body}
            ]
        });

        match client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    last_err = Some(anyhow!("Mistral chat HTTP {status}: {text}"));
                    continue;
                }
                let parsed: ChatResponse = serde_json::from_str(&text)
                    .map_err(|e| anyhow!("Invalid chat response: {e}"))?;
                let content = parsed
                    .choices
                    .first()
                    .map(|c| c.message.content.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow!("Empty polish response"))?;
                return Ok(strip_wrapping_quotes(&content));
            }
            Err(e) => {
                last_err = Some(anyhow!("{e}"));
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("Polish failed")))
}

pub fn handle_polish_failure(
    polish: &PolishConfig,
    fallback_text: &str,
    err: &anyhow::Error,
) -> String {
    eprintln!("⚠️  Polish unavailable: {err}");
    match polish.on_failure.as_str() {
        "error" => {
            eprintln!("   (on_failure=error — no text inserted)");
            String::new()
        }
        _ => {
            eprintln!("   Inserting locally processed transcript instead.");
            fallback_text.to_string()
        }
    }
}

fn strip_wrapping_quotes(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        t[1..t.len() - 1].trim().to_string()
    } else if t.starts_with("```") {
        t.trim_start_matches('`')
            .trim_start_matches(|c: char| c.is_alphanumeric() || c == '\n')
            .trim_end_matches('`')
            .trim()
            .to_string()
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_quotes() {
        assert_eq!(strip_wrapping_quotes("\"hello\""), "hello");
    }
}
