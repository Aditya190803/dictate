//! Chat completions to frame and polish utterances (provider-agnostic STT).

#![cfg_attr(test, allow(dead_code))]

use crate::config::{Config, PolishBackend};
use crate::polish_styles::apply_style_to_system;
use crate::text_processing::{preferred_vocabulary, PolishConfig};
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

const SEGMENT_SYSTEM_SUFFIX: &str = r#"

You are polishing ONE segment of ongoing dictation. Prior context was already inserted in the app; output ONLY the polished form of the new segment (not the whole document). Use prior context for terminology and continuity. Fix self-corrections within this segment only.
"#;

const DEFAULT_OLLAMA_POLISH_MODEL: &str = "gemma-4";

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

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

pub async fn polish_segment(
    local_segment: &str,
    prior_context: &str,
    config: &Config,
    polish: &PolishConfig,
    dictation_mode: &str,
) -> Result<String> {
    if local_segment.trim().is_empty() {
        return Ok(String::new());
    }
    let system: String = [
        polish_system_prompt(polish),
        SEGMENT_SYSTEM_SUFFIX.to_string(),
    ]
    .concat();

    let prior = prior_context.trim();
    let vocabulary = vocabulary_prompt(config);
    let user_body = if prior.is_empty() {
        format!(
            "Dictation mode: {dictation_mode}\nLanguage: {}{vocabulary}\n\nNew segment (raw STT after local cleanup):\n{local_segment}",
            config.transcription_language
        )
    } else {
        format!(
            "Dictation mode: {dictation_mode}\nLanguage: {}{vocabulary}\n\nPrior context (already in the app):\n{prior}\n\nNew segment (raw STT after local cleanup):\n{local_segment}",
            config.transcription_language
        )
    };

    run_polish_chat(&user_body, &system, config, polish).await
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

    let system = polish_system_prompt(polish);

    let vocabulary = vocabulary_prompt(config);
    let user_body = format!(
        "Dictation mode: {dictation_mode}\nLanguage: {}{vocabulary}\n\nRaw transcript:\n{raw_after_local}",
        config.transcription_language
    );

    run_polish_chat(&user_body, &system, config, polish).await
}

fn polish_system_prompt(polish: &PolishConfig) -> String {
    let base = if polish.system_prompt.trim().is_empty() {
        DEFAULT_SYSTEM.to_string()
    } else {
        polish.system_prompt.clone()
    };
    apply_style_to_system(&base, &polish.style)
}

fn vocabulary_prompt(config: &Config) -> String {
    let vocabulary = preferred_vocabulary(&config.text_processing);
    if vocabulary.is_empty() {
        return String::new();
    }

    let terms = vocabulary
        .iter()
        .take(80)
        .map(|term| format!("- {term}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n\nPreferred vocabulary: if the transcript resembles one of these terms, use this spelling:\n{terms}"
    )
}

/// Voice command mode: transform clipboard text with LLM when local rules don't match.
pub async fn transform_with_llm(
    instruction: &str,
    source: &str,
    config: &Config,
    polish: &PolishConfig,
) -> Result<String> {
    let system = r#"You edit text according to a spoken instruction. Output ONLY the transformed text—no quotes, preamble, or markdown fences unless the user asked for markdown."#;
    let user_body = format!(
        "Instruction (from voice):\n{}\n\nSource text:\n{}",
        instruction.trim(),
        source.trim()
    );
    run_polish_chat(&user_body, system, config, polish).await
}

fn polish_model_for_backend(config: &Config, polish: &PolishConfig, backend: PolishBackend) -> String {
    let configured = polish.model.trim();
    match backend {
        PolishBackend::Mistral => {
            if configured.is_empty() {
                "mistral-small-latest".to_string()
            } else {
                configured.to_string()
            }
        }
        PolishBackend::Ollama => {
            if configured.is_empty() || configured.contains("mistral") {
                std::env::var("OLLAMA_POLISH_MODEL")
                    .unwrap_or_else(|_| DEFAULT_OLLAMA_POLISH_MODEL.to_string())
            } else {
                configured.to_string()
            }
        }
    }
}

async fn run_polish_chat(
    user_body: &str,
    system: &str,
    config: &Config,
    polish: &PolishConfig,
) -> Result<String> {
    let backend = config
        .resolve_polish_backend()
        .ok_or_else(|| polish_unavailable_error(config))?;

    let model = polish_model_for_backend(config, polish, backend);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.transcription_timeout_seconds))
        .build()?;

    let mut last_err = None;
    let attempts = config.transcription_max_retries.max(1);

    for attempt in 0..attempts {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
        }

        let result = match backend {
            PolishBackend::Mistral => {
                run_mistral_chat(
                    &client,
                    user_body,
                    system,
                    config,
                    polish,
                    &model,
                )
                .await
            }
            PolishBackend::Ollama => {
                run_ollama_chat(&client, user_body, system, config, polish, &model).await
            }
        };

        match result {
            Ok(content) => return Ok(strip_wrapping_quotes(&content)),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("Polish failed")))
}

fn polish_unavailable_error(config: &Config) -> anyhow::Error {
    let mode = config.polish_provider.trim().to_lowercase();
    match mode.as_str() {
        "mistral" => anyhow!(
            "POLISH_PROVIDER=mistral but MISTRAL_API_KEY is missing (set key or use POLISH_PROVIDER=ollama)"
        ),
        "ollama" => anyhow!(
            "Polish via Ollama failed — is Ollama running? (ollama serve). Set OLLAMA_BASE_URL / OLLAMA_POLISH_MODEL if needed."
        ),
        _ => anyhow!(
            "No polish backend: set MISTRAL_API_KEY or run Ollama locally (POLISH_PROVIDER=auto|ollama)"
        ),
    }
}

async fn run_mistral_chat(
    client: &reqwest::Client,
    user_body: &str,
    system: &str,
    config: &Config,
    polish: &PolishConfig,
    model: &str,
) -> Result<String> {
    let api_key = config
        .mistral_api_key
        .as_ref()
        .ok_or_else(|| anyhow!("MISTRAL_API_KEY required for polish"))?;

    let base = config
        .mistral_base_url
        .clone()
        .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string());

    let url = format!("{}/chat/completions", base.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "temperature": polish.temperature,
        "max_tokens": polish.max_tokens,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_body}
        ]
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("Mistral chat HTTP {status}: {text}"));
    }
    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid chat response: {e}"))?;
    parsed
        .choices
        .first()
        .map(|c| c.message.content.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("Empty polish response"))
}

async fn run_ollama_chat(
    client: &reqwest::Client,
    user_body: &str,
    system: &str,
    config: &Config,
    polish: &PolishConfig,
    model: &str,
) -> Result<String> {
    let base = config.ollama_base_url.trim_end_matches('/');
    let url = format!("{base}/api/chat");

    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "options": {
            "temperature": polish.temperature,
            "num_predict": polish.max_tokens,
        },
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user_body}
        ]
    });

    let mut req = client.post(&url).json(&body);
    if let Some(key) = config
        .ollama_api_key
        .as_ref()
        .filter(|k| !k.is_empty())
    {
        req = req.header("Authorization", format!("Bearer {key}"));
    }

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("Ollama chat HTTP {status}: {text}"));
    }
    let parsed: OllamaChatResponse =
        serde_json::from_str(&text).map_err(|e| anyhow!("Invalid Ollama response: {e}"))?;
    let content = parsed.message.content.trim().to_string();
    if content.is_empty() {
        return Err(anyhow!("Empty polish response from Ollama"));
    }
    Ok(content)
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

fn is_fence_lang_tag(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty()
        && line
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn strip_wrapping_quotes(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        return t[1..t.len() - 1].trim().to_string();
    }
    if let Some(inner) = t
        .strip_prefix("```")
        .and_then(|rest| rest.strip_suffix("```"))
    {
        let inner = inner.trim();
        if let Some((first, rest)) = inner.split_once('\n') {
            if is_fence_lang_tag(first) {
                return rest.trim().to_string();
            }
        }
        return inner.to_string();
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_quotes() {
        assert_eq!(strip_wrapping_quotes("\"hello\""), "hello");
    }

    #[test]
    fn ollama_model_when_polish_model_is_mistral_default() {
        let config = Config::default();
        let polish = PolishConfig::default();
        let m = polish_model_for_backend(&config, &polish, PolishBackend::Ollama);
        assert_eq!(m, DEFAULT_OLLAMA_POLISH_MODEL);
    }
}