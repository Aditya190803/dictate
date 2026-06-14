use anyhow::Result;

/// Constants for audio processing.
const WINDOW_SIZE_MS: u32 = 10;
const SILENCE_RATIO: f32 = 0.1;
const TARGET_AMPLITUDE: f32 = 0.8;
const MIN_DURATION_SECONDS: f32 = 0.1;
const SIGNAL_THRESHOLD: f32 = 0.001;

/// Audio processing utilities for speech recognition optimization.
///
/// Provides silence detection, trimming, and normalization for
/// speech-to-text pipelines.
#[derive(Debug)]
pub struct AudioProcessor {
    sample_rate: u32,
}

impl AudioProcessor {
    /// Create a new audio processor for the given sample rate.
    pub fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }

    /// Calculate RMS (Root Mean Square) for a slice of audio samples.
    pub fn calculate_rms(&self, samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_squares: f32 = samples.iter().map(|&s| s * s).sum();
        (sum_squares / samples.len() as f32).sqrt()
    }

    /// Detect silence regions using an RMS-based threshold.
    ///
    /// Returns `(start, end)` indices of contiguous silence regions.
    pub fn detect_silence(&self, samples: &[f32], silence_threshold: f32) -> Vec<(usize, usize)> {
        let window_size = self.window_size_samples();
        let mut regions = Vec::new();
        let mut start = None;

        for (i, window) in samples.chunks(window_size).enumerate() {
            let rms = self.calculate_rms(window);
            let win_start = i * window_size;

            if rms <= silence_threshold {
                start.get_or_insert(win_start);
            } else if let Some(s) = start.take() {
                regions.push((s, win_start));
            }
        }

        // Silence extending to the end
        if let Some(s) = start {
            regions.push((s, samples.len()));
        }

        regions
    }

    /// Calculate an adaptive silence threshold at 10% of peak RMS.
    pub fn calculate_silence_threshold(&self, samples: &[f32]) -> f32 {
        let window_size = self.window_size_samples();
        let max_rms = samples
            .chunks(window_size)
            .map(|w| self.calculate_rms(w))
            .fold(0.0f32, f32::max);

        max_rms * SILENCE_RATIO
    }

    /// Trim silence from the start and end of an audio buffer.
    pub fn trim_silence(&self, samples: &[f32]) -> Result<Vec<f32>> {
        if samples.is_empty() {
            anyhow::bail!("Cannot trim silence from empty audio buffer");
        }

        let threshold = self.calculate_silence_threshold(samples);
        let regions = self.detect_silence(samples, threshold);

        let start = regions
            .iter()
            .find(|(s, _)| *s == 0)
            .map(|(_, e)| *e)
            .unwrap_or(0);

        let end = regions
            .iter()
            .rev()
            .find(|(_, e)| *e == samples.len())
            .map(|(s, _)| *s)
            .unwrap_or(samples.len());

        if start >= end {
            anyhow::bail!("Audio contains only silence");
        }

        Ok(samples[start..end].to_vec())
    }

    /// Peak-normalize audio to 80% of maximum amplitude.
    pub fn normalize_audio(&self, samples: &[f32]) -> Vec<f32> {
        let peak = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        if peak == 0.0 {
            return samples.to_vec();
        }

        let gain = TARGET_AMPLITUDE / peak;
        samples.iter().map(|&s| s * gain).collect()
    }

    /// Validate audio quality and minimum duration.
    pub fn validate_audio(&self, samples: &[f32]) -> Result<()> {
        if samples.is_empty() {
            anyhow::bail!("Audio buffer is empty");
        }

        let duration = samples.len() as f32 / self.sample_rate as f32;
        if duration < MIN_DURATION_SECONDS {
            anyhow::bail!(
                "Audio duration too short: {duration:.2}s (minimum {MIN_DURATION_SECONDS}s required)"
            );
        }

        if samples.iter().all(|&s| s.abs() < SIGNAL_THRESHOLD) {
            anyhow::bail!("Audio contains no detectable signal");
        }

        Ok(())
    }

    /// Complete processing pipeline: validate → trim → normalize.
    pub fn process_for_speech_recognition(&self, samples: &[f32]) -> Result<Vec<f32>> {
        self.validate_audio(samples)?;
        let trimmed = self.trim_silence(samples)?;
        self.validate_audio(&trimmed)?;
        Ok(self.normalize_audio(&trimmed))
    }

    /// Return the window size in samples for RMS calculation.
    fn window_size_samples(&self) -> usize {
        (self.sample_rate as f32 * WINDOW_SIZE_MS as f32 / 1000.0) as usize
    }

    /// Return the audio duration in seconds.
    pub fn get_duration_seconds(&self, samples: &[f32]) -> f32 {
        samples.len() as f32 / self.sample_rate as f32
    }
}

impl Default for AudioProcessor {
    fn default() -> Self {
        Self::new(16000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_processor_creation() {
        let processor = AudioProcessor::new(16000);
        assert_eq!(processor.sample_rate, 16000);
    }

    #[test]
    fn test_audio_processor_default() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.sample_rate, 16000);
    }

    #[test]
    fn test_calculate_rms_empty() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.calculate_rms(&[]), 0.0);
    }

    #[test]
    fn test_calculate_rms_silent() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.calculate_rms(&[0.0, 0.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn test_calculate_rms_signal() {
        let processor = AudioProcessor::default();
        let samples = vec![0.5, -0.5, 0.5, -0.5];
        let rms = processor.calculate_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_silence_detection_threshold() {
        let processor = AudioProcessor::default();
        let mut samples = vec![0.01; 1600];
        samples.extend(vec![0.5; 1600]);
        samples.extend(vec![0.01; 1600]);

        let threshold = processor.calculate_silence_threshold(&samples);
        assert!((threshold - 0.05).abs() < 0.01);
    }

    #[test]
    fn test_detect_silence_regions() {
        let processor = AudioProcessor::default();
        let ws = processor.window_size_samples();

        let mut samples = vec![0.0; ws * 2];
        samples.extend(vec![0.5; ws * 3]);
        samples.extend(vec![0.0; ws]);

        let regions = processor.detect_silence(&samples, 0.1);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], (0, ws * 2));
        assert_eq!(regions[1], (ws * 5, ws * 6));
    }

    #[test]
    fn test_trim_silence_normal() {
        let processor = AudioProcessor::default();
        let ws = processor.window_size_samples();

        let mut samples = vec![0.0; ws];
        let speech = vec![0.5; ws * 2];
        samples.extend(&speech);
        samples.extend(vec![0.0; ws]);

        let trimmed = processor.trim_silence(&samples).unwrap();
        assert_eq!(trimmed.len(), ws * 2);
        assert!((trimmed[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_trim_silence_empty() {
        let processor = AudioProcessor::default();
        assert!(processor.trim_silence(&[]).is_err());
    }

    #[test]
    fn test_trim_silence_only_silence() {
        let processor = AudioProcessor::default();
        assert!(processor.trim_silence(&[0.0; 1600]).is_err());
    }

    #[test]
    fn test_normalize_audio_empty() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.normalize_audio(&[] as &[f32]), vec![] as Vec<f32>);
    }

    #[test]
    fn test_normalize_audio_peak() {
        let processor = AudioProcessor::default();
        let samples = vec![0.5, -0.5, 0.25];
        let normalized = processor.normalize_audio(&samples);

        assert!((normalized[0] - 0.8).abs() < 0.001);
        assert!((normalized[1] - (-0.8)).abs() < 0.001);
        assert!((normalized[2] - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_validate_audio_empty() {
        let processor = AudioProcessor::default();
        assert!(processor.validate_audio(&[]).is_err());
    }

    #[test]
    fn test_validate_audio_too_short() {
        let processor = AudioProcessor::default();
        assert!(processor.validate_audio(&[0.5; 160]).is_err());
    }

    #[test]
    fn test_validate_audio_no_signal() {
        let processor = AudioProcessor::default();
        assert!(processor.validate_audio(&[0.0; 1600]).is_err());
    }

    #[test]
    fn test_validate_audio_valid() {
        let processor = AudioProcessor::default();
        assert!(processor.validate_audio(&[0.1; 1600]).is_ok());
    }

    #[test]
    fn test_get_duration_seconds() {
        let processor = AudioProcessor::default();
        assert_eq!(processor.get_duration_seconds(&vec![0.0; 16000]), 1.0);
        assert_eq!(processor.get_duration_seconds(&vec![0.0; 8000]), 0.5);
    }

    #[test]
    fn test_process_for_speech_recognition_valid() {
        let processor = AudioProcessor::default();
        let ws = processor.window_size_samples();

        let mut samples = vec![0.0; ws];
        samples.extend(vec![0.2; ws * 10]);
        samples.extend(vec![0.0; ws]);

        let processed = processor.process_for_speech_recognition(&samples).unwrap();
        assert!(processed.len() < samples.len());
        assert!(processed.len() >= ws * 10);

        let peak = processed.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
        assert!((peak - 0.8).abs() < 0.1);
    }

    #[test]
    fn test_window_size_samples() {
        assert_eq!(AudioProcessor::new(16000).window_size_samples(), 160);
        assert_eq!(AudioProcessor::new(44100).window_size_samples(), 441);
    }
}
