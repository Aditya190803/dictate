use crate::audio::AudioRecorder;
use crate::audio_processing::AudioProcessor;
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::command;
use crate::config::Config;
use crate::transcription::{TranscriptionFactory, TranscriptionProvider};
use crate::wav::WavEncoder;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

// VAD constants
const FRAME_MS: usize = 30; // 30ms frames
const SPEECH_START_FRAMES: usize = 5; // 150ms to trigger speech start
const SPEECH_END_FRAMES: usize = 20; // 600ms silence to end speech
const MIN_SPEECH_MS: usize = 500; // minimum 500ms speech segment
const MAX_SPEECH_MS: usize = 15000; // max 15s segment (force split)
const RING_FRAMES: usize = 20; // 600ms lookback ring buffer

/// Voice Activity Detection segmenter
pub struct VADSegmenter {
    processor: AudioProcessor,
    frame_size: usize,
    threshold: f32,
    noise_floor: f32,
    buffer: Vec<f32>,
    speech_buffer: Vec<f32>,
    in_speech: bool,
    consecutive_speech: usize,
    consecutive_silence: usize,
    ring_buffer: Vec<Vec<f32>>,
    total_speech_frames: usize,
}

impl VADSegmenter {
    pub fn new(sample_rate: u32) -> Self {
        let processor = AudioProcessor::new(sample_rate);
        let frame_size = sample_rate as usize * FRAME_MS / 1000;
        Self {
            processor,
            frame_size,
            threshold: 0.02,
            noise_floor: 0.01,
            buffer: Vec::new(),
            speech_buffer: Vec::new(),
            in_speech: false,
            consecutive_speech: 0,
            consecutive_silence: 0,
            ring_buffer: Vec::with_capacity(RING_FRAMES),
            total_speech_frames: 0,
        }
    }

    /// Process audio chunks. Returns a complete speech segment when detected.
    pub fn process_chunk(&mut self, chunk: &[f32]) -> Option<Vec<f32>> {
        self.buffer.extend_from_slice(chunk);
        let mut result = None;

        while self.buffer.len() >= self.frame_size {
            let frame: Vec<f32> = self.buffer.drain(0..self.frame_size).collect();
            let rms = self.processor.calculate_rms(&frame);

            // Update adaptive noise floor when not in speech
            if !self.in_speech {
                self.noise_floor = self.noise_floor * 0.95 + rms * 0.05;
                self.threshold = self.noise_floor * 4.0 + 0.005;
            }

            let is_speech = rms > self.threshold;

            if is_speech {
                self.consecutive_speech += 1;
                self.consecutive_silence = 0;

                if !self.in_speech && self.consecutive_speech >= SPEECH_START_FRAMES {
                    self.in_speech = true;
                    // Include lookback from ring buffer
                    let lookback = self.consecutive_speech.min(self.ring_buffer.len());
                    self.speech_buffer.clear();
                    for f in &self.ring_buffer[self.ring_buffer.len() - lookback..] {
                        self.speech_buffer.extend_from_slice(f);
                    }
                    self.speech_buffer.extend_from_slice(&frame);
                    self.total_speech_frames = lookback + 1;
                } else if self.in_speech {
                    self.speech_buffer.extend_from_slice(&frame);
                    self.total_speech_frames += 1;
                }
            } else {
                self.consecutive_silence += 1;
                self.consecutive_speech = 0;

                if self.in_speech {
                    self.speech_buffer.extend_from_slice(&frame);
                    self.total_speech_frames += 1;

                    // Check end conditions
                    let should_end = self.consecutive_silence >= SPEECH_END_FRAMES
                        || self.total_speech_frames >= MAX_SPEECH_MS / FRAME_MS;

                    if should_end {
                        self.in_speech = false;
                        let min_frames = MIN_SPEECH_MS / FRAME_MS;
                        if self.total_speech_frames >= min_frames {
                            result = Some(std::mem::take(&mut self.speech_buffer));
                        } else {
                            self.speech_buffer.clear();
                        }
                        self.total_speech_frames = 0;
                    }
                }
            }

            // Update ring buffer
            self.ring_buffer.push(frame);
            if self.ring_buffer.len() > RING_FRAMES {
                self.ring_buffer.remove(0);
            }
        }

        result
    }

    /// Flush any remaining speech buffer
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        if self.in_speech && self.total_speech_frames >= MIN_SPEECH_MS / FRAME_MS {
            self.in_speech = false;
            // Include remaining buffer
            self.speech_buffer.extend_from_slice(&self.buffer);
            self.buffer.clear();
            Some(std::mem::take(&mut self.speech_buffer))
        } else {
            None
        }
    }
}

/// Run continuous streaming transcription with VAD
pub async fn run_stream(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    shutdown_rx: &mut tokio::sync::mpsc::Receiver<()>,
) -> Result<()> {
    eprintln!("🎙️  dictate stream mode — speak and text appears as you talk");
    eprintln!("   Press Super+R again or send SIGTERM to stop");

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config.clone())?;

    // Load transcription provider once (keeps model in memory for local)
    eprintln!("📦 Loading transcription provider...");
    let provider = TranscriptionFactory::create_provider(&config.transcription_provider).await?;
    let provider = Arc::new(Mutex::new(provider));
    eprintln!("✅ Provider ready");

    // Start continuous audio capture
    let mut recorder = AudioRecorder::new()?;
    let audio_rx = recorder.start_continuous()?;

    // Play start beep
    beep_player.play_async(BeepType::RecordingStart).await.ok();

    let mut segmenter = VADSegmenter::new(16000);
    let mut last_audio_time = Instant::now();

    loop {
        tokio::select! {
            // Check for shutdown signal
            _ = shutdown_rx.recv() => {
                eprintln!("\n🛑 Stream mode shutting down...");
                if let Some(segment) = segmenter.flush() {
                    let _ = process_segment(segment, config, Arc::clone(&provider), pipe_command, &beep_player).await;
                }
                beep_player.play_async(BeepType::RecordingStop).await.ok();
                break;
            }

            // Process audio chunks with timeout
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                match audio_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(chunk) => {
                        last_audio_time = Instant::now();

                        if let Some(segment) = segmenter.process_chunk(&chunk) {
                            // Speech segment complete — process in background
                            let provider_clone = Arc::clone(&provider);
                            let config_clone = config.clone();
                            let pipe_clone = pipe_command.cloned();
                            let beep_clone = BeepPlayer::new(beep_config.clone())?;

                            tokio::spawn(async move {
                                if let Err(e) = process_segment(
                                    segment,
                                    &config_clone,
                                    provider_clone,
                                    pipe_clone.as_ref(),
                                    &beep_clone,
                                ).await {
                                    eprintln!("❌ Segment processing error: {}", e);
                                }
                            });
                        }
                    }
                    Err(_) => {
                        // No audio chunk received in 50ms — normal
                        // If no audio for 30 seconds, maybe warn
                        if last_audio_time.elapsed() > Duration::from_secs(30) {
                            eprintln!("⚠️  No audio detected for 30s — mic may be muted or disconnected");
                            last_audio_time = Instant::now(); // Reset to avoid spam
                        }
                    }
                }
            }
        }
    }

    eprintln!("✅ Stream mode exited");
    Ok(())
}

async fn process_segment(
    samples: Vec<f32>,
    config: &Config,
    provider: Arc<Mutex<Box<dyn TranscriptionProvider>>>,
    pipe_command: Option<&Vec<String>>,
    beep_player: &BeepPlayer,
) -> Result<()> {
    // Process audio (trim silence, normalize)
    let processor = AudioProcessor::new(16000);
    let processed = match processor.process_for_speech_recognition(&samples) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("⚠️  Audio processing failed ({}), using raw samples", e);
            samples // Fallback
        }
    };

    let duration_ms = processed.len() * 1000 / 16000;
    eprintln!("🧠 Transcribing {}ms segment...", duration_ms);

    // Encode to WAV
    let encoder = WavEncoder::new(16000, 1);
    let wav_data = encoder.encode_to_wav(&processed)?;

    // Transcribe
    let language = if config.transcription_language == "auto" {
        None
    } else {
        Some(config.transcription_language.clone())
    };

    let provider_guard = provider.lock().await;
    match provider_guard
        .transcribe_with_language(wav_data, language)
        .await
    {
        Ok(text) => {
            let text = text.trim();
            if !text.is_empty() {
                eprintln!("📝 {}", text);

                if let Some(cmd) = pipe_command {
                    match command::execute_with_input(cmd, text).await {
                        Ok(code) => {
                            if code != 0 {
                                eprintln!("⚠️  Pipe command exited with code {}", code);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Pipe command failed: {}", e);
                        }
                    }
                } else {
                    println!("{}", text);
                }

                beep_player.play_async(BeepType::Success).await.ok();
            }
        }
        Err(e) => {
            eprintln!("❌ Transcription error: {}", e);
            beep_player.play_async(BeepType::Error).await.ok();
        }
    }

    Ok(())
}
