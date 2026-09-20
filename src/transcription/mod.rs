use std::collections::HashSet;
use std::fmt;

pub mod online;

#[cfg(feature = "local")]
pub mod local;

/// Detailed error information from API responses.
#[derive(Debug)]
pub struct ApiErrorDetails {
    pub provider: String,
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub error_message: String,
    /// Raw response body, kept for debugging.
    #[allow(dead_code)]
    pub raw_response: Option<String>,
}

/// Details about a network-level failure.
#[derive(Debug)]
pub struct NetworkErrorDetails {
    pub provider: String,
    pub error_type: String,
    pub error_message: String,
}

/// Unified error type for all transcription operations.
#[derive(Debug)]
pub enum TranscriptionError {
    AuthenticationFailed {
        provider: String,
        details: Option<String>,
    },
    NetworkError(NetworkErrorDetails),
    FileTooLarge(usize),
    ApiError(ApiErrorDetails),
    JsonError(String),
    ConfigurationError(String),
    UnsupportedProvider(String),
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticationFailed { provider, details } => {
                if let Some(details) = details {
                    write!(f, "Authentication failed with {provider}: {details}")
                } else {
                    write!(f, "Authentication failed with {provider}")
                }
            }
            Self::NetworkError(d) => {
                write!(
                    f,
                    "Network error with {}: {} - {}",
                    d.provider, d.error_type, d.error_message
                )
            }
            Self::FileTooLarge(size) => {
                write!(f, "File too large: {size} bytes (max 25MB)")
            }
            Self::ApiError(d) => {
                write!(f, "API error with {}", d.provider)?;
                if let Some(status) = d.status_code {
                    write!(f, " (HTTP {status})")?;
                }
                if let Some(code) = &d.error_code {
                    write!(f, " [{code}]")?;
                }
                write!(f, ": {}", d.error_message)
            }
            Self::JsonError(msg) => write!(f, "JSON error: {msg}"),
            Self::ConfigurationError(msg) => write!(f, "Configuration error: {msg}"),
            Self::UnsupportedProvider(provider) => write!(f, "Unsupported provider: {provider}"),
        }
    }
}

impl std::error::Error for TranscriptionError {}

/// Async trait for transcription providers.
#[async_trait::async_trait]
pub trait TranscriptionProvider: Send + Sync {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError>;
}

/// Type alias for the shared provider used in daemon/stream mode.
pub type SharedProvider = std::sync::Arc<tokio::sync::Mutex<Box<dyn TranscriptionProvider>>>;

/// Factory for creating transcription providers.
pub struct TranscriptionFactory;

/// Percent-encode a model id for use as a realtime query parameter.
///
/// Model ids in practice are `[A-Za-z0-9._~-]` (e.g. `nova-3`,
/// `voxtral-mini-latest`), which pass through untouched; anything else is
/// `%XX`-encoded so a future model with spaces or slashes cannot break the URL.
pub fn encode_model(model: &str) -> String {
    if model
        .bytes()
        .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        log::warn!("model id contains whitespace/control characters; percent-encoding it");
    }
    let mut out = String::with_capacity(model.len());
    for b in model.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Normalize a configured transcription language to a realtime query value.
///
/// Returns `None` for dictate's `auto` sentinel (empty or `auto`): callers omit
/// `&language=` and let the model use its default. Invalid codes warn and map
/// to `None` rather than being rejected upstream.
pub fn sanitize_language(raw: &str) -> Option<String> {
    let lang = raw.trim().to_lowercase();
    if lang.is_empty() || lang == "auto" {
        return None;
    }
    // Allow-list `[a-z-]{2,12}`: bare codes (`en`) and BCP-47 style
    // (`en-us`, `zh-hant-hk`); no empty segments (`-en`, `en--us`).
    let valid = (2..=12).contains(&lang.len())
        && lang.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
        && lang.split('-').all(|part| !part.is_empty());
    if !valid {
        log::warn!("invalid TRANSCRIPTION_LANGUAGE {raw:?}; using auto");
        return None;
    }
    Some(lang)
}

const MAX_STT_HINTS: usize = 80;

/// Dictionary terms safe to send as STT hints (Mistral `context_bias`, Groq `prompt`,
/// Deepgram `keyterm`). Drops empties, commas, and query-breaking characters.
pub fn stt_hint_terms(terms: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for term in terms {
        let term = term.trim();
        if term.is_empty() || term.chars().count() > dictate::word_store::MAX_WORD_LEN {
            continue;
        }
        if term.contains(',')
            || term
                .bytes()
                .any(|b| b == b'&' || b == b'=' || b == b'%' || b == b'?' || b.is_ascii_control())
        {
            continue;
        }
        if seen.insert(term.to_lowercase()) {
            out.push(term.to_string());
        }
        if out.len() >= MAX_STT_HINTS {
            break;
        }
    }
    out
}

pub fn whisper_prompt(terms: &[String]) -> Option<String> {
    let hints = stt_hint_terms(terms);
    if hints.is_empty() {
        None
    } else {
        Some(hints.join(", "))
    }
}

pub fn append_deepgram_keyterms(url: &mut String, terms: &[String]) {
    for term in stt_hint_terms(terms) {
        url.push_str("&keyterm=");
        url.push_str(&encode_model(&term));
    }
}

impl TranscriptionFactory {
    /// Shared constructor for the three OpenAI-compatible online providers.
    ///
    /// The per-provider blocks only differed in key/model/base-url source,
    /// auth style, and dialect — all of which are parameters here. Callers
    /// resolve the base URL (with its provider default) so this stays at 7
    /// args for `clippy::too_many_arguments`.
    fn provider_opts(
        provider_name: &'static str,
        api_key: Option<String>,
        model: String,
        base_url: String,
        auth_style: online::AuthStyle,
        dialect: online::ApiDialect,
        config: &crate::config::Config,
    ) -> Result<online::OnlineProviderOptions, TranscriptionError> {
        let api_key = api_key.ok_or_else(|| {
            TranscriptionError::ConfigurationError(format!("{provider_name} API key not found"))
        })?;
        Ok(online::OnlineProviderOptions {
            provider_name,
            api_key,
            timeout_seconds: config.transcription_timeout_seconds,
            max_retries: config.transcription_max_retries,
            model,
            base_url,
            auth_style,
            dialect,
            vocabulary: crate::text_processing::preferred_vocabulary(&config.text_processing),
        })
    }

    fn create_online_options(
        provider_type: &str,
        config: &crate::config::Config,
    ) -> Result<online::OnlineProviderOptions, TranscriptionError> {
        match provider_type.to_lowercase().as_str() {
            "mistral" => Self::provider_opts(
                "Mistral",
                config.mistral_api_key.clone(),
                config.mistral_model.clone(),
                config
                    .mistral_base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string()),
                online::AuthStyle::Bearer,
                online::ApiDialect::OpenAiCompatible,
                config,
            ),
            "groq" => Self::provider_opts(
                "Groq",
                config.groq_api_key.clone(),
                config.groq_model.clone(),
                config
                    .groq_base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string()),
                online::AuthStyle::Bearer,
                online::ApiDialect::OpenAiCompatible,
                config,
            ),
            // Deepgram is not versioned in the base path; `/v1/listen` is
            // appended by the Deepgram dialect.
            "deepgram" => Self::provider_opts(
                "Deepgram",
                config.deepgram_api_key.clone(),
                config.deepgram_model.clone(),
                config
                    .deepgram_base_url
                    .clone()
                    .unwrap_or_else(|| "https://api.deepgram.com".to_string()),
                online::AuthStyle::Token,
                online::ApiDialect::Deepgram,
                config,
            ),
            _ => Err(TranscriptionError::UnsupportedProvider(
                provider_type.to_string(),
            )),
        }
    }

    /// Create a transcription provider based on the provider type string.
    pub async fn create_provider(
        provider_type: &str,
        config: &crate::config::Config,
    ) -> Result<Box<dyn TranscriptionProvider>, TranscriptionError> {
        match provider_type.to_lowercase().as_str() {
            "mistral" | "groq" | "deepgram" => {
                let provider = online::OnlineTranscriptionProvider::new(
                    Self::create_online_options(provider_type, config)?,
                )?;
                Ok(Box::new(provider))
            }
            "local" => {
                #[cfg(feature = "local")]
                {
                    let model_path = crate::config::Config::model_path(&config.whisper_model);
                    let provider = local::LocalWhisperProvider::new(&model_path)?;
                    Ok(Box::new(provider))
                }
                #[cfg(not(feature = "local"))]
                {
                    Err(TranscriptionError::ConfigurationError(
                        "Local transcription is not compiled in. \
                         Rebuild with `cargo build --features local`."
                            .to_string(),
                    ))
                }
            }
            _ => Err(TranscriptionError::UnsupportedProvider(
                provider_type.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcription_error_display() {
        let error = TranscriptionError::AuthenticationFailed {
            provider: "Mistral".to_string(),
            details: None,
        };
        assert_eq!(error.to_string(), "Authentication failed with Mistral");

        let error = TranscriptionError::AuthenticationFailed {
            provider: "Groq".to_string(),
            details: Some("Invalid API key".to_string()),
        };
        assert_eq!(
            error.to_string(),
            "Authentication failed with Groq: Invalid API key"
        );

        let error = TranscriptionError::FileTooLarge(30_000_000);
        assert_eq!(
            error.to_string(),
            "File too large: 30000000 bytes (max 25MB)"
        );

        let error = TranscriptionError::UnsupportedProvider("azure".to_string());
        assert_eq!(error.to_string(), "Unsupported provider: azure");
    }

    #[tokio::test]
    async fn test_factory_unsupported_provider() {
        let config = crate::config::Config::default();
        let result = TranscriptionFactory::create_provider("unsupported", &config).await;
        assert!(matches!(
            result,
            Err(TranscriptionError::UnsupportedProvider(_))
        ));
    }

    #[tokio::test]
    async fn test_factory_mistral_provider_missing_key() {
        let config = crate::config::Config::default();
        let result = TranscriptionFactory::create_provider("mistral", &config).await;
        assert!(matches!(
            result,
            Err(TranscriptionError::ConfigurationError(_))
        ));
    }

    #[tokio::test]
    async fn test_factory_mistral_provider_creation() {
        let config = crate::config::Config {
            mistral_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        let result = TranscriptionFactory::create_provider("mistral", &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_provider_switching_case_insensitive() {
        let config = crate::config::Config {
            mistral_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        assert!(TranscriptionFactory::create_provider("Mistral", &config)
            .await
            .is_ok());
        assert!(TranscriptionFactory::create_provider("MISTRAL", &config)
            .await
            .is_ok());
    }

    #[test]
    fn sanitize_language_allows_codes_and_auto() {
        assert_eq!(sanitize_language("auto"), None);
        assert_eq!(sanitize_language("AUTO"), None);
        assert_eq!(sanitize_language(""), None);
        assert_eq!(sanitize_language("en"), Some("en".to_string()));
        assert_eq!(sanitize_language("en-US"), Some("en-us".to_string()));
    }

    #[test]
    fn sanitize_language_rejects_injection() {
        assert_eq!(sanitize_language("en&foo=bar"), None);
        assert_eq!(sanitize_language("en?x=1"), None);
        assert_eq!(sanitize_language("../x"), None);
        assert_eq!(sanitize_language("e"), None);
        assert_eq!(sanitize_language("averylonglanguagename"), None);
        assert_eq!(sanitize_language("-en"), None);
        assert_eq!(sanitize_language("en--us"), None);
    }

    #[test]
    fn encode_model_neutralizes_query_breakout() {
        assert_eq!(encode_model("nova-3"), "nova-3");
        assert_eq!(
            encode_model("m&smart_format=false"),
            "m%26smart_format%3Dfalse"
        );
    }

    #[test]
    fn stt_hint_terms_keep_names_and_drop_query_breakers() {
        let terms = [
            "Hyprland".to_string(),
            "  Supabase ".to_string(),
            "bad&x=1".to_string(),
            "a,b".to_string(),
            String::new(),
            "hyprland".to_string(),
        ];
        assert_eq!(
            stt_hint_terms(&terms),
            vec!["Hyprland".to_string(), "Supabase".to_string()]
        );
        assert_eq!(
            whisper_prompt(&terms).as_deref(),
            Some("Hyprland, Supabase")
        );
        let mut url = "https://api.deepgram.com/v1/listen?model=nova-3".to_string();
        append_deepgram_keyterms(&mut url, &terms);
        assert_eq!(
            url,
            "https://api.deepgram.com/v1/listen?model=nova-3&keyterm=Hyprland&keyterm=Supabase"
        );
    }

    #[test]
    fn deepgram_keyterms_encode_spaces_in_phrases() {
        let mut url = String::from("https://example.test/v1/listen?");
        append_deepgram_keyterms(&mut url, &["Wispr Flow".to_string()]);
        assert!(url.contains("keyterm=Wispr%20Flow"));
        assert!(!url.contains("keyterm=Wispr Flow"));
    }
}
