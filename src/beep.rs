use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::warn;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Types of beeps for different events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeepType {
    /// Recording started — short ascending tone.
    RecordingStart,
    /// Recording stopped — short descending tone.
    RecordingStop,
    /// Success — double beep.
    Success,
    /// Error occurred — low warbling tone.
    Error,
}

/// Configuration for audio feedback beeps.
#[derive(Debug, Clone)]
pub struct BeepConfig {
    pub enabled: bool,
    pub volume: f32,
}

impl Default for BeepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.1,
        }
    }
}

/// Audio feedback player for user notifications.
pub struct BeepPlayer {
    config: BeepConfig,
}

// ─── Musical constants ───────────────────────────────────────────────────────
const C4: f32 = 261.63;
const E4: f32 = 329.63;
const ERROR_BASE: f32 = 200.0;

/// Describes a single beep sound in terms of its musical intervals and timing.
#[derive(Clone, Copy, Debug)]
struct BeepDescriptor {
    freq1: f32,
    freq2: f32,
    total_ms: f32,
    first_part: f32, // 0.0–1.0, fraction of total before gap
    gap: f32,        // 0.0–1.0, fraction of total for silence gap
    volume_mult: f32,
}

impl BeepDescriptor {
    /// Returns the frequency at normalized time `t` (0.0–1.0).
    fn frequency_at(&self, t: f32) -> f32 {
        let gap_end = self.first_part + self.gap;
        if t < self.first_part {
            self.freq1
        } else if t < gap_end {
            0.0 // gap / silence
        } else {
            self.freq2
        }
    }

    /// Returns the warbling offset (non-zero only for Error beep).
    fn wobble(&self, t: f32) -> f32 {
        if self.freq1 == ERROR_BASE {
            (t * 8.0 * 2.0 * std::f32::consts::PI).sin() * 20.0
        } else {
            0.0
        }
    }
}

impl From<BeepType> for BeepDescriptor {
    fn from(bt: BeepType) -> Self {
        match bt {
            BeepType::RecordingStart => Self {
                freq1: C4,
                freq2: E4,
                total_ms: 500.0,
                first_part: 0.45,
                gap: 0.10,
                volume_mult: 2.0,
            },
            BeepType::RecordingStop => Self {
                freq1: E4,
                freq2: C4,
                total_ms: 500.0,
                first_part: 0.45,
                gap: 0.10,
                volume_mult: 2.0,
            },
            BeepType::Success => Self {
                freq1: E4,
                freq2: E4,
                total_ms: 400.0,
                first_part: 0.40,
                gap: 0.20,
                volume_mult: 1.0,
            },
            BeepType::Error => Self {
                freq1: ERROR_BASE,
                freq2: ERROR_BASE,
                total_ms: 300.0,
                first_part: 1.0,
                gap: 0.0,
                volume_mult: 1.0,
            },
        }
    }
}

impl BeepPlayer {
    /// Create a new `BeepPlayer`.
    pub fn new(config: BeepConfig) -> Result<Self> {
        Ok(Self { config })
    }

    /// Play a beep asynchronously (non-blocking).
    pub async fn play_async(&self, beep_type: BeepType) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        let volume = self.config.volume;
        tokio::task::spawn_blocking(move || play_beep_internal(beep_type, volume)).await?
    }
}

// ─── Internal: play a beep via CPAL ───────────────────────────────────────

fn play_beep_internal(beep_type: BeepType, volume: f32) -> Result<()> {
    let host = cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            warn!("No audio output device available for beeps");
            return Ok(());
        }
    };

    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to get audio output config for beeps: {e}");
            return Ok(());
        }
    };

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let desc = BeepDescriptor::from(beep_type);
    let sample_count = (sample_rate * desc.total_ms / 1000.0) as usize;

    let playing = Arc::new(AtomicBool::new(true));
    let flag = playing.clone();
    let err_cb = |err: cpal::StreamError| warn!("Beep stream error: {err}");

    let result = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let cb = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                fill_audio(
                    data,
                    sample_count,
                    sample_rate,
                    channels,
                    volume,
                    &flag,
                    desc,
                );
            };
            device.build_output_stream(&config.into(), cb, err_cb, None)
        }
        cpal::SampleFormat::I16 => {
            let cb = move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                fill_audio(
                    data,
                    sample_count,
                    sample_rate,
                    channels,
                    volume,
                    &flag,
                    desc,
                );
            };
            device.build_output_stream(&config.into(), cb, err_cb, None)
        }
        fmt => {
            warn!("Unsupported audio format for beeps: {fmt:?}");
            return Ok(());
        }
    };

    let stream = match result {
        Ok(s) => s,
        Err(e) => {
            warn!("Failed to create audio output stream: {e}");
            return Ok(());
        }
    };

    if let Err(e) = stream.play() {
        warn!("Failed to start audio stream for beep: {e}");
        return Ok(());
    }

    while playing.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(stream);
    Ok(())
}

// ─── Generic sample-type audio filler ───────────────────────────────────────

trait Sample: Copy + Default {
    fn from_float(v: f32) -> Self;
}

impl Sample for f32 {
    fn from_float(v: f32) -> Self {
        v.clamp(-1.0, 1.0)
    }
}

impl Sample for i16 {
    fn from_float(v: f32) -> Self {
        (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
    }
}

#[allow(clippy::cast_possible_truncation)]
fn fill_audio<T: Sample>(
    data: &mut [T],
    sample_count: usize,
    sample_rate: f32,
    channels: usize,
    volume: f32,
    playing: &AtomicBool,
    desc: BeepDescriptor,
) {
    let mut phase = 0.0f32;

    for (idx, frame) in data.chunks_mut(channels).enumerate() {
        if idx >= sample_count {
            playing.store(false, Ordering::Relaxed);
            frame.fill(T::default());
            continue;
        }

        let progress = idx as f32 / sample_count as f32;
        let freq = desc.frequency_at(progress);
        let sample = if freq == 0.0 {
            T::default()
        } else {
            let raw = (phase * 2.0 * std::f32::consts::PI).sin();
            T::from_float(raw * volume * desc.volume_mult)
        };

        frame.fill(sample);

        if freq != 0.0 {
            let effective_freq = freq + desc.wobble(progress);
            phase += effective_freq / sample_rate;
            if phase > 1.0 {
                phase -= 1.0;
            }
        }
    }
}

/// ─── Tests ──────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beep_config_default() {
        let config = BeepConfig::default();
        assert!(config.enabled);
        assert_eq!(config.volume, 0.1);
    }

    #[tokio::test]
    async fn test_beep_player_disabled() {
        let player = BeepPlayer::new(BeepConfig {
            enabled: false,
            volume: 0.1,
        })
        .unwrap();
        assert!(player.play_async(BeepType::RecordingStart).await.is_ok());
        assert!(player.play_async(BeepType::Success).await.is_ok());
    }

    #[test]
    fn test_beep_types_distinct() {
        assert_ne!(BeepType::RecordingStart, BeepType::RecordingStop);
        assert_ne!(BeepType::Success, BeepType::Error);
    }

    #[test]
    fn test_descriptor_from_type() {
        let desc = BeepDescriptor::from(BeepType::RecordingStart);
        assert!((desc.freq1 - C4).abs() < 0.01);
        assert!((desc.freq2 - E4).abs() < 0.01);
        assert!((desc.total_ms - 500.0).abs() < 0.01);

        let desc = BeepDescriptor::from(BeepType::Error);
        assert!((desc.freq1 - ERROR_BASE).abs() < 0.01);
    }

    #[test]
    fn test_descriptor_frequency_at() {
        let desc = BeepDescriptor::from(BeepType::Success);
        assert!((desc.frequency_at(0.1) - E4).abs() < 0.01);
        assert!(desc.frequency_at(0.5).abs() < 0.01); // gap
        assert!((desc.frequency_at(0.8) - E4).abs() < 0.01);
    }

    #[test]
    fn test_descriptor_wobble() {
        let desc = BeepDescriptor::from(BeepType::Error);
        assert!(desc.wobble(0.0).abs() < 0.001); // sin(0)
                                                 // sin(0.015625 * 8 * 2π) = sin(π/4) ≈ 0.707, then * 20 ≈ 14.14
        let expected = (0.015625 * 8.0 * 2.0 * std::f32::consts::PI).sin() * 20.0;
        assert!((desc.wobble(0.015625) - expected).abs() < 0.001);

        let desc = BeepDescriptor::from(BeepType::Success);
        assert_eq!(desc.wobble(0.5), 0.0);
    }
}
