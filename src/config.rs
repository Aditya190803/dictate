use crate::platform;
use crate::profile::DictateProfile;
use crate::text_processing::TextProcessingConfig;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Chat backend for transcript polish (not the STT provider).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolishBackend {
    /// OpenCode Zen (OpenAI-compatible, `big-pickle`). Text-only — never used for STT.
    OpenCode,
    Mistral,
    Ollama,
}

/// Ctrl+V via ydotool with explicit press/release. Never use `29:125` — that leaves Ctrl+Super stuck down.
#[cfg(unix)]
pub const YDOTOOL_PASTE_SHELL: &str = "wl-copy && ydotool key 29:1 47:1 47:0 29:0";

/// Resolve `SHORTCUT_OUTPUT` to a pipe target for the current platform.
///
/// Wayland shells out to ydotool/wl-copy; Windows uses in-process sinks. See
/// [`crate::platform`].
fn parse_pipe_to_env(mode: Option<&str>) -> Option<Vec<String>> {
    match mode?.trim().to_lowercase().as_str() {
        "stdout" | "" => None,
        mode => platform::pipe_to_for_mode(mode),
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
    pub deepgram_api_key: Option<String>,
    pub deepgram_base_url: Option<String>,
    pub deepgram_model: String,
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
    /// Legacy second shortcut; default install uses `shortcut_key` only.
    #[allow(dead_code)]
    pub shortcut_key_smart: Option<String>,
    pub shortcut_desktop: Option<String>,
    /// Voice corrections on typed/pasted session text (smart path, VAD segments).
    pub context_editing: bool,
    pub context_editing_max_delete_chars: usize,
    pub context_editing_max_delete_words: usize,
    /// LLM for polish / command-mode fallback: `auto`, `opencode`, `mistral`, or `ollama`.
    pub polish_provider: String,
    pub ollama_base_url: String,
    pub ollama_api_key: Option<String>,
    /// OpenCode Zen key for text polish only. Separate from `mistral_api_key`, which
    /// stays the speech-to-text credential — OpenCode Zen cannot do transcription.
    pub opencode_api_key: Option<String>,
    pub opencode_base_url: Option<String>,
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
            deepgram_api_key: None,
            deepgram_base_url: None,
            deepgram_model: "nova-3".to_string(),
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
            profile: DictateProfile::Segmented,
            default_pipe_to: None,
            shortcut_key: None,
            shortcut_key_live: None,
            shortcut_key_smart: None,
            shortcut_desktop: None,
            context_editing: true,
            context_editing_max_delete_chars: 300,
            context_editing_max_delete_words: 10,
            polish_provider: "auto".to_string(),
            ollama_base_url: "http://127.0.0.1:11434".to_string(),
            ollama_api_key: None,
            opencode_api_key: None,
            opencode_base_url: None,
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
            deepgram_api_key: std::env::var("DEEPGRAM_API_KEY").ok(),
            deepgram_base_url: std::env::var("DEEPGRAM_BASE_URL").ok(),
            deepgram_model: env_str_or("DEEPGRAM_MODEL", "nova-3"),
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
            polish_provider: env_str_or("POLISH_PROVIDER", "auto"),
            ollama_base_url: env_str_or("OLLAMA_BASE_URL", "http://127.0.0.1:11434"),
            ollama_api_key: std::env::var("OLLAMA_API_KEY").ok(),
            opencode_api_key: std::env::var("OPENCODE_API_KEY").ok(),
            opencode_base_url: std::env::var("OPENCODE_BASE_URL").ok(),
        }
    }

    pub fn save_transcript_history(&self) -> bool {
        self.text_processing.history.enabled
    }

    /// Mistral LLM for unrecognized `--command` voice instructions (text.toml).
    #[cfg_attr(test, allow(dead_code))]
    pub fn command_mode_uses_llm(&self) -> bool {
        self.text_processing.command_mode.use_llm && self.polish_available()
    }

    /// Whether LLM polish / command LLM can run (independent of STT provider).
    pub fn polish_available(&self) -> bool {
        self.resolve_polish_backend().is_some()
    }

    /// `auto`: OpenCode Zen if its key is set, else Mistral if its key is set, else Ollama.
    /// Explicit `opencode` / `mistral` / `ollama` require that backend.
    ///
    /// This only ever picks a *polish* (text) backend. STT provider selection is
    /// independent and still driven by `transcription_provider`.
    pub fn resolve_polish_backend(&self) -> Option<PolishBackend> {
        let mode = self.polish_provider.trim().to_lowercase();
        let opencode = self
            .opencode_api_key
            .as_ref()
            .is_some_and(|k| !k.trim().is_empty());
        let mistral = self.mistral_api_key.as_ref().is_some_and(|k| !k.is_empty());
        match mode.as_str() {
            "opencode" | "zen" | "big-pickle" => {
                if opencode {
                    Some(PolishBackend::OpenCode)
                } else {
                    None
                }
            }
            "mistral" => {
                if mistral {
                    Some(PolishBackend::Mistral)
                } else {
                    None
                }
            }
            "ollama" => Some(PolishBackend::Ollama),
            "auto" | "" => {
                if opencode {
                    Some(PolishBackend::OpenCode)
                } else if mistral {
                    Some(PolishBackend::Mistral)
                } else {
                    Some(PolishBackend::Ollama)
                }
            }
            _ => None,
        }
    }

    /// Realtime daemons launched from shortcuts should listen immediately on the first press.
    pub fn realtime_daemon_active_on_start(&self) -> bool {
        matches!(
            self.profile,
            DictateProfile::Segmented | DictateProfile::LiveTyping
        ) && self.use_mistral_realtime_stt()
    }

    /// Mistral realtime WebSocket STT (default segmented + legacy live typing).
    pub fn use_mistral_realtime_stt(&self) -> bool {
        matches!(
            self.profile,
            DictateProfile::Segmented | DictateProfile::LiveTyping
        ) && self.transcription_provider.eq_ignore_ascii_case("mistral")
            && !self.transcription_mode.eq_ignore_ascii_case("batch")
    }

    /// Effective pipe command: CLI override or configured default.
    pub fn resolve_pipe_to<'a>(&'a self, cli: Option<&'a Vec<String>>) -> Option<&'a Vec<String>> {
        cli.or(self.default_pipe_to.as_ref())
    }

    /// Keys dropped from `.env` (features removed); stripped on load.
    fn is_retired_env_key(key: &str) -> bool {
        matches!(
            key.trim().to_uppercase().as_str(),
            "ENABLE_OVERLAY" | "DICTATE_OVERLAY_SOCKET"
        )
    }

    /// Remove obsolete `KEY=value` lines from the config file (in place).
    pub fn prune_retired_env_keys<P: AsRef<Path>>(path: P) -> Result<bool> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(false);
        }
        let contents = std::fs::read_to_string(path)?;
        let mut kept = Vec::new();
        let mut removed = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            let drop_line = !trimmed.starts_with('#')
                && !trimmed.is_empty()
                && trimmed
                    .split_once('=')
                    .is_some_and(|(key, _)| Self::is_retired_env_key(key));
            if drop_line {
                removed = true;
                continue;
            }
            kept.push(line);
        }
        if !removed {
            return Ok(false);
        }
        let mut out = kept.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        std::fs::write(path, out)?;
        Ok(true)
    }

    /// Load environment file and return config.
    pub fn load_env_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Err(err) = Self::prune_retired_env_keys(path) {
            log::warn!(
                "failed to prune retired env keys from {}: {err}",
                path.display()
            );
        }
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
            "deepgram" => {
                if self.deepgram_api_key.is_none() {
                    anyhow::bail!(
                        "DEEPGRAM_API_KEY is required when using Deepgram provider. \
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
                     Supported providers: mistral, groq, deepgram, local"
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

        if self.profile == DictateProfile::SmartPaste && !self.polish_available() {
            anyhow::bail!(
                "DICTATE_PROFILE=smart_paste requires a polish backend (MISTRAL_API_KEY or Ollama)."
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
            "DEEPGRAM_API_KEY",
            "DEEPGRAM_BASE_URL",
            "DEEPGRAM_MODEL",
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
    fn prune_retired_env_keys_drops_enable_overlay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".env");
        std::fs::write(
            &path,
            "MISTRAL_API_KEY=x\nENABLE_OVERLAY=true\nTRANSCRIPTION_PROVIDER=mistral\n",
        )
        .unwrap();
        assert!(Config::prune_retired_env_keys(&path).unwrap());
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(!saved.contains("ENABLE_OVERLAY"));
        assert!(saved.contains("MISTRAL_API_KEY"));
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
    fn polish_auto_prefers_mistral_when_key_set() {
        let config = Config {
            mistral_api_key: Some("k".to_string()),
            polish_provider: "auto".to_string(),
            ..Default::default()
        };
        assert_eq!(
            config.resolve_polish_backend(),
            Some(PolishBackend::Mistral)
        );
    }

    #[test]
    fn polish_auto_prefers_opencode_over_mistral() {
        let config = Config {
            opencode_api_key: Some("k".to_string()),
            mistral_api_key: Some("k".to_string()),
            polish_provider: "auto".to_string(),
            ..Default::default()
        };
        assert_eq!(
            config.resolve_polish_backend(),
            Some(PolishBackend::OpenCode)
        );
    }

    /// An OpenCode key must never be mistaken for an STT credential.
    #[test]
    fn opencode_key_alone_does_not_enable_mistral_polish() {
        let config = Config {
            opencode_api_key: Some("k".to_string()),
            mistral_api_key: None,
            polish_provider: "mistral".to_string(),
            ..Default::default()
        };
        assert_eq!(config.resolve_polish_backend(), None);
    }

    #[test]
    fn polish_auto_uses_ollama_without_mistral_key() {
        let config = Config {
            mistral_api_key: None,
            polish_provider: "auto".to_string(),
            ..Default::default()
        };
        assert_eq!(config.resolve_polish_backend(), Some(PolishBackend::Ollama));
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
