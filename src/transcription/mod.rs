use async_trait::async_trait;
use std::fmt;
use std::fmt::Write;

// Local whisper provider using whisper-rs
#[cfg(feature = "local")]
pub mod local;
// Mistral and Groq HTTP transcription providers
pub mod online;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ApiErrorDetails {
    pub provider: String,
    pub status_code: Option<u16>,
    pub error_code: Option<String>,
    pub error_message: String,
    pub raw_response: Option<String>,
}

#[derive(Debug)]
pub struct NetworkErrorDetails {
    pub provider: String,
    pub error_type: String,
    pub error_message: String,
}

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
            TranscriptionError::AuthenticationFailed { provider, details } => {
                if let Some(details) = details {
                    write!(f, "Authentication failed with {}: {}", provider, details)
                } else {
                    write!(f, "Authentication failed with {}", provider)
                }
            }
            TranscriptionError::NetworkError(details) => {
                write!(
                    f,
                    "Network error with {}: {} - {}",
                    details.provider, details.error_type, details.error_message
                )
            }
            TranscriptionError::FileTooLarge(size) => {
                write!(f, "File too large: {} bytes (max 25MB)", size)
            }
            TranscriptionError::ApiError(details) => {
                let mut msg = format!("API error with {}", details.provider);

                if let Some(status) = details.status_code {
                    write!(&mut msg, " (HTTP {})", status).unwrap();
                }

                if let Some(code) = &details.error_code {
                    write!(&mut msg, " [{}]", code).unwrap();
                }

                write!(&mut msg, ": {}", details.error_message).unwrap();

                write!(f, "{}", msg)
            }
            TranscriptionError::JsonError(msg) => write!(f, "JSON error: {}", msg),
            TranscriptionError::ConfigurationError(msg) => {
                write!(f, "Configuration error: {}", msg)
            }
            TranscriptionError::UnsupportedProvider(provider) => {
                write!(f, "Unsupported provider: {}", provider)
            }
        }
    }
}

impl std::error::Error for TranscriptionError {}

#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    async fn transcribe_with_language(
        &self,
        audio_data: Vec<u8>,
        language: Option<String>,
    ) -> Result<String, TranscriptionError>;
}

pub struct TranscriptionFactory;

impl TranscriptionFactory {
    pub async fn create_provider(
        provider_type: &str,
    ) -> Result<Box<dyn TranscriptionProvider>, TranscriptionError> {
        let config = crate::config::load_config();

        match provider_type.to_lowercase().as_str() {
            "mistral" => {
                let api_key = config.mistral_api_key.ok_or_else(|| {
                    TranscriptionError::ConfigurationError("Mistral API key not found".to_string())
                })?;

                let provider =
                    online::OnlineTranscriptionProvider::new(online::OnlineProviderOptions {
                        provider_name: "Mistral",
                        api_key,
                        timeout_seconds: config.transcription_timeout_seconds,
                        max_retries: config.transcription_max_retries,
                        model: config.mistral_model,
                        base_url: config
                            .mistral_base_url
                            .unwrap_or_else(|| "https://api.mistral.ai/v1".to_string()),
                        auth_style: online::AuthStyle::XApiKey,
                    })?;

                Ok(Box::new(provider))
            }
            "groq" => {
                let api_key = config.groq_api_key.ok_or_else(|| {
                    TranscriptionError::ConfigurationError("Groq API key not found".to_string())
                })?;

                let provider =
                    online::OnlineTranscriptionProvider::new(online::OnlineProviderOptions {
                        provider_name: "Groq",
                        api_key,
                        timeout_seconds: config.transcription_timeout_seconds,
                        max_retries: config.transcription_max_retries,
                        model: config.groq_model,
                        base_url: config
                            .groq_base_url
                            .unwrap_or_else(|| "https://api.groq.com/openai/v1".to_string()),
                        auth_style: online::AuthStyle::Bearer,
                    })?;

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
                        "Local transcription support is not compiled in this binary. Reinstall with DICTATE_BUILD_FEATURES=local or build with `cargo build --release --features local`.".to_string(),
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
    use crate::test_utils::ENV_MUTEX;

    fn clear_env_vars() {
        for key in [
            "MISTRAL_API_KEY",
            "GROQ_API_KEY",
            "TRANSCRIPTION_PROVIDER",
            "WHISPER_MODEL",
        ] {
            std::env::remove_var(key);
        }
    }

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

        let error = TranscriptionError::NetworkError(NetworkErrorDetails {
            provider: "Mistral".to_string(),
            error_type: "Connection timeout".to_string(),
            error_message: "Request timed out after 30s".to_string(),
        });
        assert_eq!(
            error.to_string(),
            "Network error with Mistral: Connection timeout - Request timed out after 30s"
        );

        let error = TranscriptionError::ApiError(ApiErrorDetails {
            provider: "Groq".to_string(),
            status_code: Some(400),
            error_code: Some("invalid_request".to_string()),
            error_message: "Invalid language code".to_string(),
            raw_response: None,
        });
        assert_eq!(
            error.to_string(),
            "API error with Groq (HTTP 400) [invalid_request]: Invalid language code"
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
        let result = TranscriptionFactory::create_provider("unsupported").await;
        assert!(result.is_err());

        if let Err(TranscriptionError::UnsupportedProvider(provider)) = result {
            assert_eq!(provider, "unsupported");
        } else {
            panic!("Expected UnsupportedProvider error");
        }
    }

    #[tokio::test]
    async fn test_factory_mistral_provider_missing_key() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            let result = TranscriptionFactory::create_provider("mistral").await;

            assert!(result.is_err());
            if let Err(TranscriptionError::ConfigurationError(msg)) = result {
                assert!(msg.contains("Mistral API key not found"));
            } else {
                panic!("Expected ConfigurationError for missing API key");
            }

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_factory_mistral_provider_creation() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();
            std::env::set_var("MISTRAL_API_KEY", "test-key");

            let result = TranscriptionFactory::create_provider("mistral").await;
            assert!(result.is_ok());

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_factory_groq_provider_missing_key() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            let result = TranscriptionFactory::create_provider("groq").await;

            assert!(result.is_err());
            if let Err(TranscriptionError::ConfigurationError(msg)) = result {
                assert!(msg.contains("Groq API key not found"));
            } else {
                panic!("Expected ConfigurationError for missing API key");
            }

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_factory_groq_provider_creation() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();
            std::env::set_var("GROQ_API_KEY", "test-key");

            let result = TranscriptionFactory::create_provider("groq").await;
            assert!(result.is_ok());

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_provider_switching_case_insensitive() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();
            std::env::set_var("MISTRAL_API_KEY", "test-key");

            let result = TranscriptionFactory::create_provider("Mistral").await;
            assert!(result.is_ok());

            let result = TranscriptionFactory::create_provider("MISTRAL").await;
            assert!(result.is_ok());

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_factory_local_provider_missing_model() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();
            let original_home = std::env::var("HOME").ok();
            let tmp_home = tempfile::tempdir().unwrap();
            std::env::set_var("HOME", tmp_home.path());
            std::env::set_var("WHISPER_MODEL", "missing.bin");

            let result = TranscriptionFactory::create_provider("local").await;

            if let Some(home) = original_home {
                std::env::set_var("HOME", home);
            } else {
                std::env::remove_var("HOME");
            }
            clear_env_vars();

            assert!(result.is_err());
        }
    }
}
