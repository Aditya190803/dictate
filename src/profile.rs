//! User-facing dictation profiles (how dictate behaves), distinct from STT provider settings.

use serde::{Deserialize, Serialize};

/// Prior typed text included in per-segment LLM polish prompts.
pub const POLISH_CONTEXT_CHARS: usize = 2000;

/// Legacy CLI `--mode` aliases (default install uses segmented only).
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
        match self {
            Self::Live => DictateProfile::LiveTyping,
            Self::Smart => DictateProfile::SmartPaste,
        }
    }
}

/// How dictation should feel for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictateProfile {
    /// Default: pause-bound segments, session context, per-segment polish (Mistral realtime or VAD).
    #[default]
    Segmented,
    /// Legacy: raw Mistral realtime deltas.
    LiveTyping,
    /// Legacy: record until stop, one polished paste.
    SmartPaste,
    /// Whole-clip batch STT, local text processing only, paste once.
    BatchClip,
}

impl DictateProfile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "segmented" | "default" | "dictate" => Some(Self::Segmented),
            "live_typing" | "live" | "realtime" | "stream" => Some(Self::LiveTyping),
            "smart_paste" | "smart" | "polish" | "polished" => Some(Self::SmartPaste),
            "batch_clip" | "batch" | "clip" => Some(Self::BatchClip),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Segmented => "segmented",
            Self::LiveTyping => "live_typing",
            Self::SmartPaste => "smart_paste",
            Self::BatchClip => "batch_clip",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Segmented => "Dictation",
            Self::LiveTyping => "Live typing",
            Self::SmartPaste => "Smart paste",
            Self::BatchClip => "Batch clip",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Segmented => {
                "Speak in phrases; polished text inserts after each pause with voice corrections."
            }
            Self::LiveTyping => "Words appear as you speak (Mistral realtime).",
            Self::SmartPaste => "Speak, stop, get polished text pasted once (daemon).",
            Self::BatchClip => "Record, stop, transcribe once with local cleanup only.",
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
        matches!(self, Self::SmartPaste | Self::BatchClip)
    }

    pub fn wants_daemon(self) -> bool {
        matches!(self, Self::Segmented | Self::LiveTyping | Self::SmartPaste)
    }

    /// Whole-clip polish (smart_paste one-shot).
    pub fn uses_llm_polish(self) -> bool {
        matches!(self, Self::SmartPaste)
    }

    /// Per-utterance polish + session buffer (default product behavior).
    pub fn uses_segment_polish(self) -> bool {
        matches!(self, Self::Segmented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_aliases() {
        assert_eq!(
            DictateProfile::parse("live_typing"),
            Some(DictateProfile::LiveTyping)
        );
        assert_eq!(
            DictateProfile::parse("smart-paste"),
            Some(DictateProfile::SmartPaste)
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
        assert!(DictateProfile::SmartPaste.uses_llm_polish());
        assert!(DictateProfile::Segmented.uses_segment_polish());
        assert!(!DictateProfile::LiveTyping.uses_llm_polish());
    }
}
