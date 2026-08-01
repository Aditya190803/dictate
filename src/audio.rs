use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Stream, StreamConfig,
};
use log::{debug, error, info};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[cfg(test)]
const DEFAULT_SAMPLE_RATE: u32 = 16000;
#[cfg(test)]
const DEFAULT_CHANNELS: u16 = 1;
#[cfg(test)]
const DEFAULT_MAX_RECORDING_DURATION_SECONDS: usize = 300;

/// Name of the microphone recording would use, or `None` if there is none.
#[cfg_attr(unix, allow(dead_code))]
pub fn default_input_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|device| device.name().ok())
}

/// Captures audio from the default input device using CPAL.
///
/// Supports both clip mode (record → stop → retrieve) and continuous streaming.
pub struct AudioRecorder {
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    stream: Option<Stream>,
    device: Option<Device>,
    chunk_sender: Option<mpsc::Sender<Vec<f32>>>,
    /// Rate we advertise to consumers (e.g. 16000 for Mistral).
    sample_rate: u32,
    channels: u16,
    /// Actual CPAL stream rate (may differ if hardware rejects 16 kHz).
    capture_sample_rate: u32,
    max_buffer_size: usize,
}

impl AudioRecorder {
    /// Create a new recorder with default settings (16kHz mono, 300s buffer).
    #[cfg(test)]
    pub fn new() -> Result<Self> {
        Self::with_settings(
            DEFAULT_SAMPLE_RATE,
            DEFAULT_CHANNELS,
            DEFAULT_MAX_RECORDING_DURATION_SECONDS,
        )
    }

    /// Create a recorder from runtime config.
    pub fn from_config(config: &crate::config::Config) -> Result<Self> {
        Self::with_settings(
            config.audio_sample_rate,
            config.audio_channels,
            config.audio_buffer_duration_seconds,
        )
    }

    /// Create a recorder with explicit capture settings.
    pub fn with_settings(
        sample_rate: u32,
        channels: u16,
        max_duration_seconds: usize,
    ) -> Result<Self> {
        if sample_rate == 0 {
            anyhow::bail!("sample_rate must be greater than 0");
        }
        if channels == 0 {
            anyhow::bail!("channels must be greater than 0");
        }
        if max_duration_seconds == 0 {
            anyhow::bail!("max_duration_seconds must be greater than 0");
        }

        let max_buffer_size = sample_rate as usize * max_duration_seconds;

        Ok(Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            device: None,
            chunk_sender: None,
            sample_rate,
            channels,
            capture_sample_rate: sample_rate,
            max_buffer_size,
        })
    }

    pub fn capture_sample_rate(&self) -> u32 {
        self.capture_sample_rate
    }

    /// Audio channel capacity: 100 chunks ≈ 3 seconds of 30ms frames
    const CHUNK_CHANNEL_CAPACITY: usize = 100;

    /// Start recording from the default input device.
    pub fn start_recording(&mut self) -> Result<()> {
        if self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No default input device available")?;

        info!("Using audio device: {}", device.name().unwrap_or_default());

        let config = pick_input_stream_config(&device, self.sample_rate, self.channels)?;
        self.capture_sample_rate = config.sample_rate.0;
        if self.capture_sample_rate != self.sample_rate {
            info!(
                "Mic opened at {} Hz (will resample to {} Hz for STT)",
                self.capture_sample_rate, self.sample_rate
            );
        }

        let buffer_clone = Arc::clone(&self.buffer);
        let chunk_sender_clone = self.chunk_sender.clone();
        let configured_channels = self.channels as usize;
        let max_buffer_size = self.max_buffer_size;

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let mono_chunk: Vec<f32> = if configured_channels == 1 {
                    data.to_vec()
                } else {
                    data.chunks(configured_channels)
                        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
                        .collect()
                };

                if let Ok(mut buf) = buffer_clone.try_lock() {
                    let new_total = buf.len() + mono_chunk.len();
                    if new_total > max_buffer_size {
                        let excess = new_total - max_buffer_size;
                        if excess < buf.len() {
                            buf.drain(0..excess);
                        } else {
                            buf.clear();
                        }
                    }

                    let was_empty = buf.is_empty();
                    buf.extend_from_slice(&mono_chunk);
                    if was_empty {
                        debug!("First audio samples captured: {} samples", mono_chunk.len());
                    }
                }

                if let Some(ref sender) = chunk_sender_clone {
                    // Non-blocking send — if the channel is full we drop the chunk;
                    // this prevents backpressure on the real-time audio thread.
                    let _ = sender.try_send(mono_chunk);
                }
            },
            |err| error!("Audio stream error: {err}"),
            None,
        )?;

        stream.play()?;

        self.is_recording.store(true, Ordering::Relaxed);
        self.stream = Some(stream);
        self.device = Some(device);

        info!("CPAL audio recording started successfully");
        Ok(())
    }

    /// Start continuous recording and return a receiver for audio chunks.
    pub fn start_continuous(&mut self) -> Result<mpsc::Receiver<Vec<f32>>> {
        let (tx, rx) = mpsc::channel(Self::CHUNK_CHANNEL_CAPACITY);
        self.chunk_sender = Some(tx);
        self.start_recording()?;
        Ok(rx)
    }

    /// Stop recording and release audio resources.
    pub fn stop_recording(&mut self) -> Result<()> {
        if !self.is_recording.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_recording.store(false, Ordering::Relaxed);

        if let Some(stream) = self.stream.take() {
            stream.pause()?;
            drop(stream);
        }

        // Drop device and chunk sender so PipeWire/CPAL can release the capture node.
        drop(self.device.take());
        self.chunk_sender = None;

        debug!("Recording stopped");
        Ok(())
    }

    /// Return a clone of the recorded audio buffer.
    pub fn get_audio_data(&self) -> Result<Vec<f32>> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock audio buffer"))?;
        Ok(buffer.clone())
    }

    /// Clear the recorded audio buffer.
    pub fn clear_buffer(&self) -> Result<()> {
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock audio buffer"))?;
        buffer.clear();
        Ok(())
    }

    /// Return the recording duration in seconds based on buffer length.
    pub fn get_recording_duration_seconds(&self) -> Result<f32> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock audio buffer"))?;
        Ok(buffer.len() as f32 / self.sample_rate as f32)
    }

    /// No-op for compatibility with the main loop (CPAL runs callbacks in the background).
    pub fn process_audio_events(&self) -> Result<()> {
        Ok(())
    }
}

fn pick_input_stream_config(
    device: &Device,
    want_rate: u32,
    want_channels: u16,
) -> Result<StreamConfig> {
    let mut candidates: Vec<StreamConfig> = Vec::new();
    if let Ok(ranges) = device.supported_input_configs() {
        for range in ranges {
            let ch = want_channels.min(range.channels());
            for &rate in &[want_rate, 48_000, 44_100, 32_000, 16_000, 8_000] {
                if rate < range.min_sample_rate().0 || rate > range.max_sample_rate().0 {
                    continue;
                }
                let cfg = range.with_sample_rate(cpal::SampleRate(rate)).config();
                candidates.push(StreamConfig {
                    channels: ch,
                    sample_rate: cfg.sample_rate,
                    buffer_size: cpal::BufferSize::Default,
                });
            }
        }
    }
    candidates.sort_by_key(|c| {
        let dr = (c.sample_rate.0 as i64 - want_rate as i64).unsigned_abs();
        let dc = (c.channels as u32).saturating_sub(want_channels as u32);
        dr + u64::from(dc) * 50_000
    });
    candidates.into_iter().next().ok_or_else(|| {
        anyhow::anyhow!(
            "No supported input config for {} Hz / {} ch",
            want_rate,
            want_channels
        )
    })
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        let _ = self.stop_recording();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_audio_recorder_creation() {
        assert!(AudioRecorder::new().is_ok());
    }

    #[test]
    fn test_initial_state() {
        let recorder = AudioRecorder::new().unwrap();
        assert_eq!(recorder.get_audio_data().unwrap().len(), 0);
    }

    #[test]
    fn test_buffer_operations() {
        let recorder = AudioRecorder::new().unwrap();

        assert_eq!(recorder.get_audio_data().unwrap().len(), 0);
        assert!(recorder.clear_buffer().is_ok());
        assert_eq!(recorder.get_audio_data().unwrap().len(), 0);
    }

    #[test]
    fn test_recording_lifecycle() {
        let mut recorder = AudioRecorder::new().unwrap();
        assert!(recorder.stop_recording().is_ok());
        assert!(recorder.stop_recording().is_ok());
    }

    #[test]
    #[ignore = "requires an available audio input device"]
    fn test_cpal_recording_initialization() {
        let mut recorder = AudioRecorder::new().unwrap();

        match recorder.start_recording() {
            Ok(()) => {
                std::thread::sleep(Duration::from_millis(100));
                for _ in 0..10 {
                    let _ = recorder.process_audio_events();
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(recorder.stop_recording().is_ok());
                println!("CPAL recording test succeeded");
            }
            Err(e) => {
                println!("CPAL recording test skipped (no audio device): {e}");
            }
        }
    }

    #[test]
    fn test_audio_format_constants() {
        assert_eq!(DEFAULT_SAMPLE_RATE, 16000);
        assert_eq!(DEFAULT_CHANNELS, 1);
        assert_eq!(DEFAULT_MAX_RECORDING_DURATION_SECONDS, 300);
    }

    #[test]
    fn test_memory_management() {
        let recorder = AudioRecorder::new().unwrap();
        assert_eq!(recorder.get_audio_data().unwrap().len(), 0);
        assert_eq!(recorder.get_recording_duration_seconds().unwrap(), 0.0);
        assert!(recorder.clear_buffer().is_ok());
    }

    #[test]
    fn test_buffer_thread_safety() {
        let recorder = AudioRecorder::new().unwrap();
        let data1 = recorder.get_audio_data().unwrap();
        let data2 = recorder.get_audio_data().unwrap();
        assert_eq!(data1, data2);
    }

    #[test]
    fn test_audio_processing_events() {
        let recorder = AudioRecorder::new().unwrap();
        assert!(recorder.process_audio_events().is_ok());
    }
}
