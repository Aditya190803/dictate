#![allow(clippy::float_cmp)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Configuration for dictate loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    pub mistral_api_key: Option<String>,
    pub mistral_base_url: Option<String>,
    pub mistral_model: String,
    pub groq_api_key: Option<String>,
    pub groq_base_url: Option<String>,
    pub groq_model: String,
    pub transcription_provider: String,
    pub transcription_language: String,
    pub transcription_timeout_seconds: u64,
    pub transcription_max_retries: u32,
    pub audio_buffer_duration_seconds: usize,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    /// Local whisper.cpp model filename used only by the `local` provider.
    pub whisper_model: String,
    pub rust_log: String,
    pub enable_audio_feedback: bool,
    pub beep_volume: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mistral_api_key: None,
            mistral_base_url: None,
            mistral_model: "voxtral-mini-latest".to_string(),
            groq_api_key: None,
            groq_base_url: None,
            groq_model: "whisper-large-v3-turbo".to_string(),
            transcription_provider: "mistral".to_string(),
            transcription_language: "auto".to_string(),
            transcription_timeout_seconds: 60,
            transcription_max_retries: 3,
            audio_buffer_duration_seconds: 300,
            audio_sample_rate: 16000,
            audio_channels: 1,
            whisper_model: "ggml-base.en.bin".to_string(),
            rust_log: "info".to_string(),
            enable_audio_feedback: true,
            beep_volume: 0.1,
        }
    }
}

impl Config {
    /// Directory where local whisper models are stored
    pub fn model_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".local/share/applications/dictate/models")
    }

    /// Full path to a model file in the model directory
    pub fn model_path(model: &str) -> PathBuf {
        Self::model_dir().join(model)
    }

    /// Load configuration from environment variables
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_env() -> Self {
        let mut config = Config::default();

        config.mistral_api_key = std::env::var("MISTRAL_API_KEY").ok();
        config.mistral_base_url = std::env::var("MISTRAL_BASE_URL").ok();
        if let Ok(model) = std::env::var("MISTRAL_MODEL") {
            config.mistral_model = model;
        }

        config.groq_api_key = std::env::var("GROQ_API_KEY").ok();
        config.groq_base_url = std::env::var("GROQ_BASE_URL").ok();
        if let Ok(model) = std::env::var("GROQ_MODEL") {
            config.groq_model = model;
        }

        if let Ok(provider) = std::env::var("TRANSCRIPTION_PROVIDER") {
            config.transcription_provider = provider;
        }

        if let Ok(language) = std::env::var("TRANSCRIPTION_LANGUAGE") {
            config.transcription_language = language;
        }

        if let Ok(timeout) = std::env::var("TRANSCRIPTION_TIMEOUT_SECONDS") {
            if let Ok(parsed) = timeout.parse::<u64>() {
                config.transcription_timeout_seconds = parsed;
            }
        }

        if let Ok(retries) = std::env::var("TRANSCRIPTION_MAX_RETRIES") {
            if let Ok(parsed) = retries.parse::<u32>() {
                config.transcription_max_retries = parsed;
            }
        }

        if let Ok(duration) = std::env::var("AUDIO_BUFFER_DURATION_SECONDS") {
            if let Ok(parsed) = duration.parse::<usize>() {
                config.audio_buffer_duration_seconds = parsed;
            }
        }

        if let Ok(sample_rate) = std::env::var("AUDIO_SAMPLE_RATE") {
            if let Ok(parsed) = sample_rate.parse::<u32>() {
                config.audio_sample_rate = parsed;
            }
        }

        if let Ok(channels) = std::env::var("AUDIO_CHANNELS") {
            if let Ok(parsed) = channels.parse::<u16>() {
                config.audio_channels = parsed;
            }
        }

        if let Ok(model) = std::env::var("WHISPER_MODEL") {
            config.whisper_model = model;
        }

        if let Ok(log_level) = std::env::var("RUST_LOG") {
            config.rust_log = log_level;
        }

        if let Ok(enabled) = std::env::var("ENABLE_AUDIO_FEEDBACK") {
            config.enable_audio_feedback = enabled.to_lowercase() == "true";
        }

        if let Ok(volume) = std::env::var("BEEP_VOLUME") {
            if let Ok(parsed) = volume.parse::<f32>() {
                config.beep_volume = parsed.clamp(0.0, 1.0);
            }
        }

        config
    }

    /// Load environment file and return config
    pub fn load_env_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        dotenvy::from_path(path)?;
        Ok(Self::from_env())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        match self.transcription_provider.to_lowercase().as_str() {
            "mistral" => {
                if self.mistral_api_key.is_none() {
                    return Err(anyhow::anyhow!(
                        "MISTRAL_API_KEY is required when using Mistral provider. Please set it in your .env file."
                    ));
                }
            }
            "groq" => {
                if self.groq_api_key.is_none() {
                    return Err(anyhow::anyhow!(
                        "GROQ_API_KEY is required when using Groq provider. Please set it in your .env file."
                    ));
                }
            }
            "local" => {
                let model_path = Config::model_path(&self.whisper_model);
                if !model_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Local model not found at {}. Use --download-model to fetch it.",
                        model_path.display()
                    ));
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported transcription provider: {}. Supported providers: mistral, groq, local",
                    self.transcription_provider
                ));
            }
        }

        if self.audio_buffer_duration_seconds == 0 {
            return Err(anyhow::anyhow!(
                "AUDIO_BUFFER_DURATION_SECONDS must be greater than 0"
            ));
        }

        if self.audio_sample_rate == 0 {
            return Err(anyhow::anyhow!("AUDIO_SAMPLE_RATE must be greater than 0"));
        }

        if self.audio_channels == 0 {
            return Err(anyhow::anyhow!("AUDIO_CHANNELS must be greater than 0"));
        }

        if self.transcription_timeout_seconds == 0 {
            return Err(anyhow::anyhow!(
                "TRANSCRIPTION_TIMEOUT_SECONDS must be greater than 0"
            ));
        }

        if self.beep_volume < 0.0 || self.beep_volume > 1.0 {
            return Err(anyhow::anyhow!(
                "BEEP_VOLUME must be between 0.0 and 1.0, got: {}",
                self.beep_volume
            ));
        }

        Ok(())
    }
}

/// Load configuration from environment variables
pub fn load_config() -> Config {
    Config::from_env()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_MUTEX;
    use std::env;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn clear_env_vars() {
        for key in [
            "MISTRAL_API_KEY",
            "MISTRAL_BASE_URL",
            "MISTRAL_MODEL",
            "GROQ_API_KEY",
            "GROQ_BASE_URL",
            "GROQ_MODEL",
            "TRANSCRIPTION_PROVIDER",
            "TRANSCRIPTION_LANGUAGE",
            "TRANSCRIPTION_TIMEOUT_SECONDS",
            "TRANSCRIPTION_MAX_RETRIES",
            "AUDIO_BUFFER_DURATION_SECONDS",
            "AUDIO_SAMPLE_RATE",
            "AUDIO_CHANNELS",
            "WHISPER_MODEL",
            "RUST_LOG",
            "ENABLE_AUDIO_FEEDBACK",
            "BEEP_VOLUME",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.mistral_api_key, None);
        assert_eq!(config.mistral_base_url, None);
        assert_eq!(config.mistral_model, "voxtral-mini-latest");
        assert_eq!(config.groq_api_key, None);
        assert_eq!(config.groq_base_url, None);
        assert_eq!(config.groq_model, "whisper-large-v3-turbo");
        assert_eq!(config.transcription_provider, "mistral");
        assert_eq!(config.transcription_language, "auto");
        assert_eq!(config.transcription_timeout_seconds, 60);
        assert_eq!(config.transcription_max_retries, 3);
        assert_eq!(config.audio_buffer_duration_seconds, 300);
        assert_eq!(config.audio_sample_rate, 16000);
        assert_eq!(config.audio_channels, 1);
        assert_eq!(config.whisper_model, "ggml-base.en.bin");
        assert_eq!(config.rust_log, "info");
        assert!(config.enable_audio_feedback);
        assert_eq!(config.beep_volume, 0.1);
    }

    #[tokio::test]
    async fn test_config_from_env_defaults() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            let config = Config::from_env();
            assert_eq!(config.transcription_provider, "mistral");
            assert_eq!(config.transcription_language, "auto");
            assert_eq!(config.transcription_timeout_seconds, 60);
            assert_eq!(config.transcription_max_retries, 3);
            assert_eq!(config.mistral_api_key, None);
            assert_eq!(config.groq_api_key, None);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_from_env_variables() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            env::set_var("MISTRAL_API_KEY", "mistral-key");
            env::set_var("MISTRAL_BASE_URL", "http://mistral.test/v1");
            env::set_var("MISTRAL_MODEL", "voxtral-mini-2602");
            env::set_var("GROQ_API_KEY", "groq-key");
            env::set_var("GROQ_BASE_URL", "http://groq.test/openai/v1");
            env::set_var("GROQ_MODEL", "whisper-large-v3-turbo");
            env::set_var("TRANSCRIPTION_PROVIDER", "groq");
            env::set_var("TRANSCRIPTION_LANGUAGE", "en");
            env::set_var("TRANSCRIPTION_TIMEOUT_SECONDS", "120");
            env::set_var("TRANSCRIPTION_MAX_RETRIES", "5");
            env::set_var("AUDIO_BUFFER_DURATION_SECONDS", "600");
            env::set_var("AUDIO_SAMPLE_RATE", "44100");
            env::set_var("AUDIO_CHANNELS", "2");
            env::set_var("WHISPER_MODEL", "ggml-small.en.bin");
            env::set_var("RUST_LOG", "debug");

            let config = Config::from_env();
            assert_eq!(config.mistral_api_key, Some("mistral-key".to_string()));
            assert_eq!(
                config.mistral_base_url,
                Some("http://mistral.test/v1".to_string())
            );
            assert_eq!(config.mistral_model, "voxtral-mini-2602");
            assert_eq!(config.groq_api_key, Some("groq-key".to_string()));
            assert_eq!(
                config.groq_base_url,
                Some("http://groq.test/openai/v1".to_string())
            );
            assert_eq!(config.groq_model, "whisper-large-v3-turbo");
            assert_eq!(config.transcription_provider, "groq");
            assert_eq!(config.transcription_language, "en");
            assert_eq!(config.transcription_timeout_seconds, 120);
            assert_eq!(config.transcription_max_retries, 5);
            assert_eq!(config.audio_buffer_duration_seconds, 600);
            assert_eq!(config.audio_sample_rate, 44100);
            assert_eq!(config.audio_channels, 2);
            assert_eq!(config.whisper_model, "ggml-small.en.bin");
            assert_eq!(config.rust_log, "debug");

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_from_env_invalid_numbers() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            env::set_var("AUDIO_BUFFER_DURATION_SECONDS", "invalid");
            env::set_var("AUDIO_SAMPLE_RATE", "not-a-number");
            env::set_var("AUDIO_CHANNELS", "bad");
            env::set_var("TRANSCRIPTION_TIMEOUT_SECONDS", "invalid");
            env::set_var("TRANSCRIPTION_MAX_RETRIES", "bad");

            let config = Config::from_env();
            assert_eq!(config.audio_buffer_duration_seconds, 300);
            assert_eq!(config.audio_sample_rate, 16000);
            assert_eq!(config.audio_channels, 1);
            assert_eq!(config.transcription_timeout_seconds, 60);
            assert_eq!(config.transcription_max_retries, 3);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_load_env_file() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            let mut temp_file = NamedTempFile::new().unwrap();
            writeln!(temp_file, "MISTRAL_API_KEY=file-api-key").unwrap();
            writeln!(temp_file, "MISTRAL_BASE_URL=http://localhost:8080").unwrap();
            writeln!(temp_file, "AUDIO_BUFFER_DURATION_SECONDS=120").unwrap();
            writeln!(temp_file, "WHISPER_MODEL=ggml-base.en.bin").unwrap();
            writeln!(temp_file, "RUST_LOG=warn").unwrap();
            writeln!(temp_file, "TRANSCRIPTION_PROVIDER=mistral").unwrap();

            let config = Config::load_env_file(temp_file.path()).unwrap();

            assert_eq!(config.mistral_api_key, Some("file-api-key".to_string()));
            assert_eq!(
                config.mistral_base_url,
                Some("http://localhost:8080".to_string())
            );
            assert_eq!(config.transcription_provider, "mistral");
            assert_eq!(config.audio_buffer_duration_seconds, 120);
            assert_eq!(config.whisper_model, "ggml-base.en.bin");
            assert_eq!(config.rust_log, "warn");
            assert_eq!(config.transcription_language, "auto");

            clear_env_vars();
        }
    }

    #[test]
    fn test_load_nonexistent_env_file() {
        let result = Config::load_env_file("/nonexistent/path/.env");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_mistral_success() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_mistral_missing_api_key() {
        let config = Config::default();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("MISTRAL_API_KEY is required"));
    }

    #[test]
    fn test_config_validation_groq_success() {
        let config = Config {
            transcription_provider: "groq".to_string(),
            groq_api_key: Some("test-key".to_string()),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_groq_missing_api_key() {
        let config = Config {
            transcription_provider: "groq".to_string(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("GROQ_API_KEY is required"));
    }

    #[test]
    fn test_config_validation_invalid_duration() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            audio_buffer_duration_seconds: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_BUFFER_DURATION_SECONDS"));
    }

    #[test]
    fn test_config_validation_invalid_sample_rate() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            audio_sample_rate: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("AUDIO_SAMPLE_RATE"));
    }

    #[test]
    fn test_config_validation_invalid_channels() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            audio_channels: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AUDIO_CHANNELS"));
    }

    #[test]
    fn test_config_validation_invalid_beep_volume() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            beep_volume: -0.1,
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));

        let config2 = Config {
            mistral_api_key: Some("test-key".to_string()),
            beep_volume: 1.1,
            ..Default::default()
        };
        let result = config2.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("BEEP_VOLUME"));
    }

    #[tokio::test]
    async fn test_config_audio_feedback_env_vars() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            env::set_var("ENABLE_AUDIO_FEEDBACK", "true");
            env::set_var("BEEP_VOLUME", "0.5");

            let config = Config::from_env();
            assert!(config.enable_audio_feedback);
            assert_eq!(config.beep_volume, 0.5);

            clear_env_vars();

            env::set_var("ENABLE_AUDIO_FEEDBACK", "false");
            env::set_var("BEEP_VOLUME", "0.8");

            let config = Config::from_env();
            assert!(!config.enable_audio_feedback);
            assert_eq!(config.beep_volume, 0.8);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_config_audio_feedback_invalid_env_vars() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            env::set_var("BEEP_VOLUME", "invalid");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 0.1);

            env::set_var("BEEP_VOLUME", "2.0");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 1.0);

            env::set_var("BEEP_VOLUME", "-0.5");
            let config = Config::from_env();
            assert_eq!(config.beep_volume, 0.0);

            clear_env_vars();
        }
    }

    #[tokio::test]
    async fn test_transcription_provider_configuration() {
        #[allow(clippy::await_holding_lock)]
        {
            let _lock = ENV_MUTEX.lock().await;
            clear_env_vars();

            let config = Config::from_env();
            assert_eq!(config.transcription_provider, "mistral");

            env::set_var("TRANSCRIPTION_PROVIDER", "groq");
            let config = Config::from_env();
            assert_eq!(config.transcription_provider, "groq");

            clear_env_vars();
        }
    }

    #[test]
    fn test_config_validation_unsupported_provider() {
        let config = Config {
            transcription_provider: "azure".to_string(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported transcription provider: azure"));
    }

    #[tokio::test]
    async fn test_config_validation_local_missing_model() {
        let _lock = ENV_MUTEX.lock().await;
        let original_home = std::env::var("HOME").ok();
        let tmp_home = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp_home.path());

        let config = Config {
            transcription_provider: "local".to_string(),
            whisper_model: "missing.bin".to_string(),
            ..Default::default()
        };

        let result = config.validate();

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_config_validation_local_success() {
        let _lock = ENV_MUTEX.lock().await;
        let original_home = std::env::var("HOME").ok();
        let tmp_home = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp_home.path());

        let model_path = Config::model_path("dummy.bin");
        std::fs::create_dir_all(model_path.parent().unwrap()).unwrap();
        std::fs::write(&model_path, b"test").unwrap();

        let config = Config {
            transcription_provider: "local".to_string(),
            whisper_model: "dummy.bin".to_string(),
            ..Default::default()
        };

        let result = config.validate();

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }

        assert!(result.is_ok());
    }
}
