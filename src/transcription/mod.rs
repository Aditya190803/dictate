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

impl TranscriptionFactory {
    fn create_online_options(
        provider_type: &str,
        config: &crate::config::Config,
    ) -> Result<online::OnlineProviderOptions, TranscriptionError> {
        match provider_type.to_lowercase().as_str() {
            "mistral" => {
                let api_key = config.mistral_api_key.clone().ok_or_else(|| {
                    TranscriptionError::ConfigurationError("Mistral API key not found".to_string())
                })?;

                Ok(online::OnlineProviderOptions {
                    provider_name: "Mistral",
                    api_key,
                    timeout_seconds: config.transcription_timeout_seconds,
                    max_retries: config.transcription_max_retries,
                    model: config.mistral_model.clone(),
                    base_url: config
                        .mistral_base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string()),
                    auth_style: online::AuthStyle::Bearer,
                })
            }
            "groq" => {
                let api_key = config.groq_api_key.clone().ok_or_else(|| {
                    TranscriptionError::ConfigurationError("Groq API key not found".to_string())
                })?;

                Ok(online::OnlineProviderOptions {
                    provider_name: "Groq",
                    api_key,
                    timeout_seconds: config.transcription_timeout_seconds,
                    max_retries: config.transcription_max_retries,
                    model: config.groq_model.clone(),
                    base_url: config
                        .groq_base_url
                        .clone()
                        .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string()),
                    auth_style: online::AuthStyle::Bearer,
                })
            }
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
            "mistral" | "groq" => {
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
}
