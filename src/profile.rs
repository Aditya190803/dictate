//! User-facing dictation profiles (how dictate behaves), distinct from STT provider settings.

use serde::{Deserialize, Serialize};

/// How dictation should feel for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictateProfile {
    /// Mistral realtime WebSocket: text appears as you speak.
    #[default]
    LiveTyping,
    /// Record until stop, transcribe whole clip, polish with LLM, paste once (daemon).
    SmartPaste,
    /// Whole-clip batch STT, local text processing only, paste once.
    BatchClip,
}

impl DictateProfile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "live_typing" | "live" | "realtime" | "stream" => Some(Self::LiveTyping),
            "smart_paste" | "smart" | "polish" | "polished" => Some(Self::SmartPaste),
            "batch_clip" | "batch" | "clip" => Some(Self::BatchClip),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveTyping => "live_typing",
            Self::SmartPaste => "smart_paste",
            Self::BatchClip => "batch_clip",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LiveTyping => "Live typing",
            Self::SmartPaste => "Smart paste",
            Self::BatchClip => "Batch clip",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
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
            Self::LiveTyping
        }
    }

    /// Apply profile to legacy STT flags (for code paths that still read batch_mode).
    pub fn implies_batch_stt(self) -> bool {
        matches!(self, Self::SmartPaste | Self::BatchClip)
    }

    pub fn wants_daemon(self) -> bool {
        matches!(self, Self::LiveTyping | Self::SmartPaste)
    }

    pub fn uses_llm_polish(self) -> bool {
        matches!(self, Self::SmartPaste)
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
    fn smart_paste_uses_llm_polish() {
        assert!(DictateProfile::SmartPaste.uses_llm_polish());
        assert!(!DictateProfile::LiveTyping.uses_llm_polish());
    }
}
