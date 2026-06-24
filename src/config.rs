use crate::profile::DictateProfile;
use crate::text_processing::TextProcessingConfig;
use anyhow::Result;
use std::path::{Path, PathBuf};

fn parse_pipe_to_env(mode: Option<&str>) -> Option<Vec<String>> {
    match mode?.trim().to_lowercase().as_str() {
        "type" | "typing" => Some(vec![
            "ydotool".to_string(),
            "type".to_string(),
            "--file".to_string(),
            "-".to_string(),
        ]),
        "clipboard" | "copy" => Some(vec!["wl-copy".to_string()]),
        "paste" | "clipboard_paste" => Some(vec![
            "sh".to_string(),
            "-c".to_string(),
            "wl-copy && ydotool key 29:125".to_string(),
        ]),
        "stdout" | "" => None,
        _ => None,
    }
}

/// Configuration for dictate loaded from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub mistral_api_key: Option<String>,
    pub mistral_base_url: Option<String>,
    pub mistral_model: String,
    pub mistral_realtime_model: String,
    pub mistral_realtime_base_url: Option<String>,
    pub mistral_realtime_delay_ms: u32,
    pub transcription_mode: String,
    /// Legacy env flag; profile drives behavior but this is kept for compat and doctor.
    pub batch_mode: bool,
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
    /// Local whisper.cpp model filename used by the `local` provider.
    pub whisper_model: String,
    pub enable_audio_feedback: bool,
    pub beep_volume: f32,
    pub text_processing: TextProcessingConfig,
    /// User-facing behavior profile (live typing vs smart paste vs batch clip).
    pub profile: DictateProfile,
    /// Default `--pipe-to` when the CLI omits it (from SHORTCUT_OUTPUT in .env).
    pub default_pipe_to: Option<Vec<String>>,
    /// Saved for shortcut generation / install.sh (not used at runtime except default_pipe_to).
    pub shortcut_key: Option<String>,
    pub shortcut_key_live: Option<String>,
    pub shortcut_key_smart: Option<String>,
    pub shortcut_desktop: Option<String>,
    /// Voice corrections on typed/pasted session text (smart path, VAD segments).
    pub context_editing: bool,
    pub context_editing_max_delete_chars: usize,
    pub context_editing_max_delete_words: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mistral_api_key: None,
            mistral_base_url: None,
            mistral_model: "voxtral-mini-latest".to_string(),
            mistral_realtime_model: "voxtral-mini-transcribe-realtime-2602".to_string(),
            mistral_realtime_base_url: None,
            mistral_realtime_delay_ms: 480,
            transcription_mode: "auto".to_string(),
            batch_mode: false,
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
            enable_audio_feedback: true,
            beep_volume: 0.1,
            text_processing: TextProcessingConfig::default(),
            profile: DictateProfile::LiveTyping,
            default_pipe_to: None,
            shortcut_key: None,
            shortcut_key_live: None,
            shortcut_key_smart: None,
            shortcut_desktop: None,
            context_editing: true,
            context_editing_max_delete_chars: 300,
            context_editing_max_delete_words: 10,
        }
    }
}

impl Config {
    /// Directory where local whisper models are stored (`~/.local/share/dictate/models`).
    pub fn model_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            })
            .join("dictate/models")
    }

    /// Full path to a model file in the model directory.
    pub fn model_path(model: &str) -> PathBuf {
        Self::model_dir().join(model)
    }

    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        /// Helper: read an env var that maps directly to a string field,
        /// falling back to the default if unset.
        fn env_str_or(name: &str, default: &str) -> String {
            std::env::var(name).unwrap_or_else(|_| default.to_string())
        }

        /// Helper: read an env var and parse into a numeric type with fallback.
        fn env_parse_or<T: std::str::FromStr>(name: &str, default: T) -> T {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }

        /// Helper: read an env var as a boolean (true/1/yes/on).
        fn env_bool_or(name: &str, default: bool) -> bool {
            std::env::var(name)
                .map(|v| {
                    matches!(
                        v.trim().to_lowercase().as_str(),
                        "true" | "1" | "yes" | "on"
                    )
                })
                .unwrap_or(default)
        }

        Config {
            mistral_api_key: std::env::var("MISTRAL_API_KEY").ok(),
            mistral_base_url: std::env::var("MISTRAL_BASE_URL").ok(),
            mistral_model: env_str_or("MISTRAL_MODEL", "voxtral-mini-latest"),
            mistral_realtime_model: env_str_or(
                "MISTRAL_REALTIME_MODEL",
                "voxtral-mini-transcribe-realtime-2602",
            ),
            mistral_realtime_base_url: std::env::var("MISTRAL_REALTIME_BASE_URL").ok(),
            mistral_realtime_delay_ms: env_parse_or("MISTRAL_REALTIME_DELAY_MS", 480u32),
            transcription_mode: env_str_or("TRANSCRIPTION_MODE", "auto"),
            batch_mode: env_bool_or("BATCH_MODE", false),
            groq_api_key: std::env::var("GROQ_API_KEY").ok(),
            groq_base_url: std::env::var("GROQ_BASE_URL").ok(),
            groq_model: env_str_or("GROQ_MODEL", "whisper-large-v3-turbo"),
            transcription_provider: env_str_or("TRANSCRIPTION_PROVIDER", "mistral"),
            transcription_language: env_str_or("TRANSCRIPTION_LANGUAGE", "auto"),
            transcription_timeout_seconds: env_parse_or("TRANSCRIPTION_TIMEOUT_SECONDS", 60u64),
            transcription_max_retries: env_parse_or("TRANSCRIPTION_MAX_RETRIES", 3u32),
            audio_buffer_duration_seconds: env_parse_or("AUDIO_BUFFER_DURATION_SECONDS", 300usize),
            audio_sample_rate: env_parse_or("AUDIO_SAMPLE_RATE", 16000u32),
            audio_channels: env_parse_or("AUDIO_CHANNELS", 1u16),
            whisper_model: env_str_or("WHISPER_MODEL", "ggml-base.en.bin"),
            enable_audio_feedback: env_bool_or("ENABLE_AUDIO_FEEDBACK", true),
            beep_volume: std::env::var("BEEP_VOLUME")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .map(|v| v.clamp(0.0, 1.0))
                .unwrap_or(0.1),
            text_processing: Default::default(),
            profile: DictateProfile::from_env_legacy(
                std::env::var("DICTATE_PROFILE").ok(),
                env_bool_or("BATCH_MODE", false),
                &env_str_or("TRANSCRIPTION_MODE", "auto"),
            ),
            default_pipe_to: parse_pipe_to_env(
                std::env::var("SHORTCUT_OUTPUT")
                    .ok()
                    .or_else(|| std::env::var("OUTPUT_MODE").ok())
                    .as_deref(),
            ),
            shortcut_key: std::env::var("SHORTCUT_KEY").ok(),
            shortcut_key_live: std::env::var("SHORTCUT_KEY_LIVE").ok(),
            shortcut_key_smart: std::env::var("SHORTCUT_KEY_SMART").ok(),
            shortcut_desktop: std::env::var("SHORTCUT_DESKTOP").ok(),
            context_editing: env_bool_or("CONTEXT_EDITING", true),
            context_editing_max_delete_chars: env_parse_or(
                "CONTEXT_EDITING_MAX_DELETE_CHARS",
                300usize,
            ),
            context_editing_max_delete_words: env_parse_or(
                "CONTEXT_EDITING_MAX_DELETE_WORDS",
                10usize,
            ),
        }
    }

    /// Whether Mistral realtime WebSocket STT should be used (live typing profile + mistral).
    pub fn use_mistral_realtime_stt(&self) -> bool {
        self.profile == DictateProfile::LiveTyping
            && self.transcription_provider.eq_ignore_ascii_case("mistral")
            && !self.transcription_mode.eq_ignore_ascii_case("batch")
    }

    /// Effective pipe command: CLI override or configured default.
    pub fn resolve_pipe_to<'a>(&'a self, cli: Option<&'a Vec<String>>) -> Option<&'a Vec<String>> {
        cli.or(self.default_pipe_to.as_ref())
    }

    /// Load environment file and return config.
    pub fn load_env_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        dotenvy::from_path(path)?;
        Ok(Self::from_env())
    }

    /// Return the text-processing config path that sits beside the env file.
    pub fn text_config_path_for_env_file<P: AsRef<Path>>(path: P) -> PathBuf {
        path.as_ref().with_file_name("text.toml")
    }

    /// Load local dictionary/snippet configuration. Missing files are a no-op.
    pub fn load_text_config_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(());
        }
        let contents = std::fs::read_to_string(path)?;
        self.text_processing = toml::from_str(&contents)?;
        Ok(())
    }

    /// Validate configuration.
    pub fn validate(&self) -> Result<()> {
        match self.transcription_provider.to_lowercase().as_str() {
            "mistral" => {
                if self.mistral_api_key.is_none() {
                    anyhow::bail!(
                        "MISTRAL_API_KEY is required when using Mistral provider. \
                         Please set it in your .env file."
                    );
                }
            }
            "groq" => {
                if self.groq_api_key.is_none() {
                    anyhow::bail!(
                        "GROQ_API_KEY is required when using Groq provider. \
                         Please set it in your .env file."
                    );
                }
            }
            "local" => {
                let model_path = Config::model_path(&self.whisper_model);
                if !model_path.exists() {
                    anyhow::bail!(
                        "Local model not found at {}. Use --download-model to fetch it.",
                        model_path.display()
                    );
                }
            }
            other => {
                anyhow::bail!(
                    "Unsupported transcription provider: {other}. \
                     Supported providers: mistral, groq, local"
                );
            }
        }

        if self.audio_buffer_duration_seconds == 0 {
            anyhow::bail!("AUDIO_BUFFER_DURATION_SECONDS must be greater than 0");
        }
        if self.audio_sample_rate == 0 {
            anyhow::bail!("AUDIO_SAMPLE_RATE must be greater than 0");
        }
        if self.audio_channels == 0 {
            anyhow::bail!("AUDIO_CHANNELS must be greater than 0");
        }
        if self.transcription_timeout_seconds == 0 {
            anyhow::bail!("TRANSCRIPTION_TIMEOUT_SECONDS must be greater than 0");
        }

        match self.transcription_mode.to_lowercase().as_str() {
            "auto" | "realtime" | "batch" => {}
            other => {
                anyhow::bail!(
                    "Unsupported TRANSCRIPTION_MODE: {other}. Supported modes: auto, realtime, batch"
                );
            }
        }

        if self.profile == DictateProfile::SmartPaste
            && !self.transcription_provider.eq_ignore_ascii_case("mistral")
        {
            anyhow::bail!(
                "DICTATE_PROFILE=smart_paste requires TRANSCRIPTION_PROVIDER=mistral (for LLM polish)."
            );
        }

        if self.mistral_realtime_delay_ms == 0 {
            anyhow::bail!("MISTRAL_REALTIME_DELAY_MS must be greater than 0");
        }

        if !(0.0..=1.0).contains(&self.beep_volume) {
            anyhow::bail!(
                "BEEP_VOLUME must be between 0.0 and 1.0, got: {}",
                self.beep_volume
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_MUTEX;
    use std::io::Write;

    fn clear_env_vars() {
        for key in [
            "DICTATE_PROFILE",
            "BATCH_MODE",
            "TRANSCRIPTION_MODE",
            "SHORTCUT_OUTPUT",
            "OUTPUT_MODE",
            "SHORTCUT_KEY",
            "SHORTCUT_DESKTOP",
            "MISTRAL_API_KEY",
            "MISTRAL_BASE_URL",
            "MISTRAL_MODEL",
            "MISTRAL_REALTIME_MODEL",
            "MISTRAL_REALTIME_BASE_URL",
            "MISTRAL_REALTIME_DELAY_MS",
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
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.mistral_api_key, None);
        assert_eq!(config.mistral_model, "voxtral-mini-latest");
        assert_eq!(config.transcription_provider, "mistral");
        assert_eq!(config.transcription_language, "auto");
        assert_eq!(config.transcription_timeout_seconds, 60);
        assert_eq!(config.audio_sample_rate, 16000);
        assert!(config.enable_audio_feedback);
        assert!((config.beep_volume - 0.1).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_config_from_env_defaults() {
        let _lock = ENV_MUTEX.lock().await;
        clear_env_vars();

        let config = Config::from_env();
        assert_eq!(config.transcription_provider, "mistral");
        assert_eq!(config.transcription_language, "auto");
        assert_eq!(config.mistral_api_key, None);

        clear_env_vars();
    }

    #[tokio::test]
    async fn test_config_from_env_variables() {
        let _lock = ENV_MUTEX.lock().await;
        clear_env_vars();

        std::env::set_var("MISTRAL_API_KEY", "mistral-key");
        std::env::set_var("MISTRAL_MODEL", "voxtral-mini-2602");
        std::env::set_var("GROQ_API_KEY", "groq-key");
        std::env::set_var("GROQ_MODEL", "whisper-large-v3-turbo");
        std::env::set_var("TRANSCRIPTION_PROVIDER", "groq");
        std::env::set_var("TRANSCRIPTION_LANGUAGE", "en");
        std::env::set_var("TRANSCRIPTION_TIMEOUT_SECONDS", "120");
        std::env::set_var("AUDIO_SAMPLE_RATE", "44100");

        let config = Config::from_env();
        assert_eq!(config.mistral_api_key, Some("mistral-key".to_string()));
        assert_eq!(config.mistral_model, "voxtral-mini-2602");
        assert_eq!(config.groq_api_key, Some("groq-key".to_string()));
        assert_eq!(config.transcription_provider, "groq");
        assert_eq!(config.transcription_language, "en");
        assert_eq!(config.transcription_timeout_seconds, 120);
        assert_eq!(config.audio_sample_rate, 44100);

        clear_env_vars();
    }

    #[tokio::test]
    async fn test_config_from_env_invalid_numbers() {
        let _lock = ENV_MUTEX.lock().await;
        clear_env_vars();

        std::env::set_var("AUDIO_BUFFER_DURATION_SECONDS", "invalid");
        std::env::set_var("AUDIO_SAMPLE_RATE", "not-a-number");
        std::env::set_var("TRANSCRIPTION_TIMEOUT_SECONDS", "invalid");

        let config = Config::from_env();
        assert_eq!(config.audio_buffer_duration_seconds, 300);
        assert_eq!(config.audio_sample_rate, 16000);
        assert_eq!(config.transcription_timeout_seconds, 60);

        clear_env_vars();
    }

    #[tokio::test]
    async fn test_load_env_file() {
        let _lock = ENV_MUTEX.lock().await;
        clear_env_vars();

        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        writeln!(temp_file, "MISTRAL_API_KEY=file-api-key").unwrap();
        writeln!(temp_file, "TRANSCRIPTION_PROVIDER=mistral").unwrap();

        let config = Config::load_env_file(temp_file.path()).unwrap();
        assert_eq!(config.mistral_api_key, Some("file-api-key".to_string()));
        assert_eq!(config.transcription_provider, "mistral");

        clear_env_vars();
    }

    #[test]
    fn test_load_nonexistent_env_file() {
        assert!(Config::load_env_file("/nonexistent/path/.env").is_err());
    }

    #[test]
    fn test_missing_text_config_is_no_op() {
        let mut config = Config::default();
        let dir = tempfile::tempdir().unwrap();
        config
            .load_text_config_file(dir.path().join("text.toml"))
            .unwrap();
        assert_eq!(config.text_processing, TextProcessingConfig::default());
    }

    #[test]
    fn test_load_text_config_file() {
        let mut config = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.toml");
        std::fs::write(
            &path,
            "[dictionary]\n\"whisper flow\" = \"Wispr Flow\"\n\n[cleanup]\nenabled = true\nfix_spacing = true\n\n[[snippets]]\ntrigger = \"calendar link\"\ntext = \"Book a time here\"\n",
        )
        .unwrap();

        config.load_text_config_file(&path).unwrap();
        assert_eq!(
            config
                .text_processing
                .dictionary
                .get("whisper flow")
                .map(String::as_str),
            Some("Wispr Flow")
        );
        assert_eq!(config.text_processing.snippets.len(), 1);
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
        let result = Config::default().validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MISTRAL_API_KEY"));
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
    fn test_config_validation_unsupported_provider() {
        let config = Config {
            transcription_provider: "azure".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("azure"));
    }

    #[test]
    fn test_config_validation_invalid_beep_volume() {
        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            beep_volume: -0.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = Config {
            mistral_api_key: Some("test-key".to_string()),
            beep_volume: 1.1,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
