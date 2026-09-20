//! User-facing dictation profiles (how dictate behaves), distinct from STT provider settings.

use serde::{Deserialize, Serialize};

/// Prior typed text included in per-segment LLM polish prompts.
pub const POLISH_CONTEXT_CHARS: usize = 2000;

/// Legacy CLI `--mode` aliases. Both map to the same dictation product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictateMode {
    #[default]
    Live,
    Smart,
}

impl DictateMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "live" | "live_typing" | "realtime" => Some(Self::Live),
            "smart" | "smart_paste" | "polish" => Some(Self::Smart),
            _ => None,
        }
    }

    pub fn profile(self) -> DictateProfile {
        DictateProfile::Segmented
    }
}

/// How dictation should feel for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictateProfile {
    /// Default product: type as you speak, voice corrections, polish on pause.
    #[default]
    Segmented,
    /// Alias of [`Self::Segmented`] (kept so old configs deserialize).
    LiveTyping,
    /// Alias of [`Self::Segmented`] (kept so old configs deserialize).
    SmartPaste,
    /// Whole-clip batch STT, local text processing only, paste once.
    BatchClip,
    /// Record a voice instruction; transform clipboard text (not free dictation).
    Command,
}

impl DictateProfile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "segmented" | "default" | "dictate" | "live_typing" | "live" | "realtime"
            | "stream" | "smart_paste" | "smart" | "polish" | "polished" => Some(Self::Segmented),
            "batch_clip" | "batch" | "clip" => Some(Self::BatchClip),
            "command" | "command_mode" => Some(Self::Command),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Segmented | Self::LiveTyping | Self::SmartPaste => "segmented",
            Self::BatchClip => "batch_clip",
            Self::Command => "command",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Segmented | Self::LiveTyping | Self::SmartPaste => "Dictation",
            Self::BatchClip => "Batch clip",
            Self::Command => "Command",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Segmented | Self::LiveTyping | Self::SmartPaste => {
                "Words appear as you speak; pauses polish the last phrase; say scratch that to edit."
            }
            Self::BatchClip => "Record, stop, transcribe once with local cleanup only.",
            Self::Command => "Speak an instruction; applies to clipboard text (e.g. fix grammar).",
        }
    }

    /// Resolve profile from env: `DICTATE_PROFILE` wins; legacy `BATCH_MODE` / `TRANSCRIPTION_MODE` otherwise.
    pub fn from_env_legacy(
        profile_var: Option<String>,
        batch_mode: bool,
        transcription_mode: &str,
    ) -> Self {
        if let Some(raw) = profile_var {
            if let Some(p) = Self::parse(&raw) {
                return p;
            }
        }
        if batch_mode || transcription_mode.eq_ignore_ascii_case("batch") {
            Self::BatchClip
        } else {
            Self::Segmented
        }
    }

    /// Apply profile to legacy STT flags (for code paths that still read batch_mode).
    pub fn implies_batch_stt(self) -> bool {
        matches!(self, Self::BatchClip | Self::Command)
    }

    pub fn wants_daemon(self) -> bool {
        !matches!(self, Self::BatchClip)
    }

    /// Free dictation (type + voice edits + polish), not clipboard-command or batch-only.
    pub fn is_dictation(self) -> bool {
        matches!(self, Self::Segmented | Self::LiveTyping | Self::SmartPaste)
    }

    /// Whole-clip polish (one-shot / groq-local clip path).
    pub fn uses_llm_polish(self) -> bool {
        self.is_dictation()
    }

    /// Per-utterance polish + session buffer.
    pub fn uses_segment_polish(self) -> bool {
        self.is_dictation()
    }

    /// Word-by-word typing when the STT provider streams deltas (Mistral).
    pub fn types_live_deltas(self) -> bool {
        self.is_dictation()
    }

    pub fn is_command_mode(self) -> bool {
        self == Self::Command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aliases() {
        assert_eq!(
            DictateProfile::parse("live_typing"),
            Some(DictateProfile::Segmented)
        );
        assert_eq!(
            DictateProfile::parse("smart-paste"),
            Some(DictateProfile::Segmented)
        );
        assert_eq!(
            DictateProfile::parse("batch"),
            Some(DictateProfile::BatchClip)
        );
    }

    #[test]
    fn legacy_batch_mode() {
        assert_eq!(
            DictateProfile::from_env_legacy(None, true, "auto"),
            DictateProfile::BatchClip
        );
    }

    #[test]
    fn default_profile_is_segmented() {
        assert_eq!(
            DictateProfile::from_env_legacy(None, false, "auto"),
            DictateProfile::Segmented
        );
    }

    #[test]
    fn polish_flags_by_profile() {
        assert!(DictateProfile::Segmented.uses_llm_polish());
        assert!(DictateProfile::Segmented.uses_segment_polish());
        assert!(DictateProfile::Segmented.types_live_deltas());
        assert!(!DictateProfile::BatchClip.uses_llm_polish());
        assert!(!DictateProfile::Command.types_live_deltas());
        assert_eq!(DictateMode::Live.profile(), DictateProfile::Segmented);
        assert_eq!(DictateMode::Smart.profile(), DictateProfile::Segmented);
    }
}
