use anyhow::Result;

/// WAV file encoder for converting f32 audio samples to 16-bit PCM WAV format.
///
/// Produces standard 44-byte header WAV files compatible with speech-to-text APIs.
#[derive(Debug)]
pub struct WavEncoder {
    sample_rate: u32,
    channels: u16,
}

impl WavEncoder {
    /// Create a new WAV encoder.
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }

    /// Generate a 44-byte WAV header for the given number of PCM samples.
    fn generate_header(&self, num_samples: usize) -> Vec<u8> {
        let bits_per_sample = 16u16;
        let byte_rate = self.sample_rate * self.channels as u32 * (bits_per_sample as u32 / 8);
        let block_align = self.channels * (bits_per_sample / 8);
        let data_size = (num_samples * (bits_per_sample as usize / 8)) as u32;
        let file_size = 36 + data_size;

        let header = [
            b"RIFF".as_ref(),
            &file_size.to_le_bytes(),
            b"WAVE".as_ref(),
            b"fmt ".as_ref(),
            &16u32.to_le_bytes(),      // fmt chunk size
            &1u16.to_le_bytes(),        // PCM format
            &self.channels.to_le_bytes(),
            &self.sample_rate.to_le_bytes(),
            &byte_rate.to_le_bytes(),
            &block_align.to_le_bytes(),
            &bits_per_sample.to_le_bytes(),
            b"data".as_ref(),
            &data_size.to_le_bytes(),
        ];
        header.concat()
    }

    /// Clamp f32 samples to [-1.0, 1.0] and convert to i16 PCM.
    fn convert_samples(&self, samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect()
    }

    /// Encode f32 samples into a complete WAV file (header + PCM data).
    pub fn encode_to_wav(&self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.is_empty() {
            anyhow::bail!("Cannot encode empty audio buffer to WAV");
        }

        let pcm_samples = self.convert_samples(samples);
        let header = self.generate_header(pcm_samples.len());

        let mut wav_data = Vec::with_capacity(header.len() + pcm_samples.len() * 2);
        wav_data.extend_from_slice(&header);
        for sample in pcm_samples {
            wav_data.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(wav_data)
    }
}

impl Default for WavEncoder {
    fn default() -> Self {
        Self::new(16000, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_encoder_creation() {
        let encoder = WavEncoder::new(16000, 1);
        assert_eq!(encoder.sample_rate, 16000);
        assert_eq!(encoder.channels, 1);
    }

    #[test]
    fn test_wav_encoder_default() {
        let encoder = WavEncoder::default();
        assert_eq!(encoder.sample_rate, 16000);
        assert_eq!(encoder.channels, 1);
    }

    #[test]
    fn test_wav_header_generation_mono_16khz() {
        let encoder = WavEncoder::new(16000, 1);
        let header = encoder.generate_header(1000);

        assert_eq!(header.len(), 44);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[12..16], b"fmt ");
        assert_eq!(u16::from_le_bytes([header[20], header[21]]), 1);
        assert_eq!(u16::from_le_bytes([header[22], header[23]]), 1);
        assert_eq!(
            u32::from_le_bytes([header[24], header[25], header[26], header[27]]),
            16000
        );
        assert_eq!(u16::from_le_bytes([header[34], header[35]]), 16);
        assert_eq!(&header[36..40], b"data");
    }

    #[test]
    fn test_f32_to_i16_conversion() {
        let encoder = WavEncoder::default();
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let converted = encoder.convert_samples(&samples);

        assert_eq!(converted.len(), 5);
        assert_eq!(converted[0], 0);
        assert_eq!(converted[1], 16383);
        assert_eq!(converted[2], -16383);
        assert_eq!(converted[3], i16::MAX);
        assert_eq!(converted[4], i16::MIN + 1); // -32767 due to f32→i16 asymmetry
    }

    #[test]
    fn test_f32_to_i16_conversion_clamping() {
        let encoder = WavEncoder::default();
        let samples = vec![2.0, -2.0, 1.5, -1.5];
        let converted = encoder.convert_samples(&samples);

        assert_eq!(converted.len(), 4);
        assert_eq!(converted[0], i16::MAX);
        assert_eq!(converted[1], i16::MIN + 1);
    }

    #[test]
    fn test_empty_audio_buffer_handling() {
        let encoder = WavEncoder::default();
        let result = encoder.encode_to_wav(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty audio"));
    }

    #[test]
    fn test_wav_bytes_output_structure() {
        let encoder = WavEncoder::default();
        let samples = vec![0.0, 0.5, -0.5];
        let wav_data = encoder.encode_to_wav(&samples).unwrap();

        assert_eq!(wav_data.len(), 50); // 44 header + 6 PCM (3×2)
        assert_eq!(&wav_data[0..4], b"RIFF");
        assert_eq!(&wav_data[8..12], b"WAVE");
        assert_eq!(&wav_data[36..40], b"data");

        let data_size =
            u32::from_le_bytes([wav_data[40], wav_data[41], wav_data[42], wav_data[43]]);
        assert_eq!(data_size, 6);
    }

    #[test]
    fn test_large_audio_buffer() {
        let encoder = WavEncoder::default();
        let samples: Vec<f32> = (0..16000).map(|i| (i as f32 / 16000.0).sin()).collect();
        let wav_data = encoder.encode_to_wav(&samples).unwrap();

        assert_eq!(wav_data.len(), 44 + 32000); // 44 header + 16000*2 data

        let data_size =
            u32::from_le_bytes([wav_data[40], wav_data[41], wav_data[42], wav_data[43]]);
        assert_eq!(data_size, 32000);
    }
}
