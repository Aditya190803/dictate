use crate::audio::AudioRecorder;
use crate::audio_processing::AudioProcessor;
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::command;
use crate::config::Config;
use crate::context_session::handle_final_segment;
use crate::developer_modes::apply_developer_mode;
use crate::text_processing::process_text;
use crate::transcript::TranscriptBuffer;
use crate::transcription::{SharedProvider, TranscriptionFactory};
use crate::typing::OutputBackend;
use crate::wav::WavEncoder;
use anyhow::{anyhow, Result};
use base64::Engine;
use dictate::overlay_ipc::{level_from_samples, OverlayPublisher, OverlayState};
use futures::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

// VAD constants
const FRAME_MS: usize = 30; // 30ms frames
const SPEECH_START_FRAMES: usize = 5; // 150ms to trigger speech start
const SPEECH_END_FRAMES: usize = 20; // 600ms silence to end speech
const MIN_SPEECH_MS: usize = 500; // minimum 500ms speech segment
const MAX_SPEECH_MS: usize = 15000; // max 15s segment (force split)
const RING_FRAMES: usize = 20; // 600ms lookback ring buffer
const SEGMENT_QUEUE_CAPACITY: usize = 8;

/// Voice Activity Detection segmenter
pub struct VADSegmenter {
    processor: AudioProcessor,
    frame_size: usize,
    threshold: f32,
    noise_floor: f32,
    buffer: VecDeque<f32>,
    speech_buffer: Vec<f32>,
    in_speech: bool,
    consecutive_speech: usize,
    consecutive_silence: usize,
    ring_buffer: VecDeque<Vec<f32>>,
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
            buffer: VecDeque::new(),
            speech_buffer: Vec::new(),
            in_speech: false,
            consecutive_speech: 0,
            consecutive_silence: 0,
            ring_buffer: VecDeque::with_capacity(RING_FRAMES),
            total_speech_frames: 0,
        }
    }

    /// Process audio chunks. Returns a complete speech segment when detected.
    pub fn process_chunk(&mut self, chunk: &[f32]) -> Option<Vec<f32>> {
        self.buffer.extend(chunk.iter().copied());
        let mut result = None;

        while self.buffer.len() >= self.frame_size {
            let frame: Vec<f32> = self.buffer.drain(..self.frame_size).collect();
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
                    let start = self.ring_buffer.len().saturating_sub(lookback);
                    for prior_frame in self.ring_buffer.iter().skip(start) {
                        self.speech_buffer.extend_from_slice(prior_frame);
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
            self.ring_buffer.push_back(frame);
            if self.ring_buffer.len() > RING_FRAMES {
                self.ring_buffer.pop_front();
            }
        }

        result
    }

    /// Flush any remaining speech buffer
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        if self.in_speech && self.total_speech_frames >= MIN_SPEECH_MS / FRAME_MS {
            self.in_speech = false;
            // Include remaining buffer
            self.speech_buffer.extend(self.buffer.drain(..));
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
    dictation_mode: &str,
) -> Result<()> {
    if should_use_mistral_realtime(config) {
        return run_mistral_realtime_stream(config, pipe_command, shutdown_rx).await;
    }

    if config.transcription_mode.eq_ignore_ascii_case("realtime") {
        eprintln!(
            "⚠️  Realtime WebSocket STT is only available for Mistral; using batch/VAD streaming for {}",
            config.transcription_provider
        );
    }

    eprintln!("🎙️  dictate stream mode — speak and text appears as you talk");
    eprintln!("   Press Super+R again or send SIGTERM to stop");

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config.clone())?;

    // Load transcription provider once (keeps model in memory for local)
    eprintln!("📦 Loading transcription provider...");
    let provider =
        TranscriptionFactory::create_provider(&config.transcription_provider, config).await?;
    let provider: SharedProvider = Arc::new(tokio::sync::Mutex::new(provider));
    eprintln!("✅ Provider ready");

    // Start continuous audio capture
    let mut recorder = AudioRecorder::from_config(config)?;
    let mut audio_rx = recorder.start_continuous()?;

    // Process segments through a bounded queue and a single worker.
    // This prevents task pile-ups when speaking continuously.
    let (segment_tx, mut segment_rx) =
        tokio::sync::mpsc::channel::<Vec<f32>>(SEGMENT_QUEUE_CAPACITY);
    let provider_for_worker = Arc::clone(&provider);
    let worker_config = config.clone();
    let worker_pipe = pipe_command.cloned();
    let worker_mode = dictation_mode.to_string();
    let worker_beep = BeepPlayer::new(beep_config.clone())?;
    let session_buffer = Arc::new(Mutex::new(TranscriptBuffer::new()));
    let worker = tokio::spawn(async move {
        while let Some(segment) = segment_rx.recv().await {
            if let Err(e) = process_segment(
                segment,
                &worker_config,
                Arc::clone(&provider_for_worker),
                worker_pipe.as_ref(),
                &worker_beep,
                &worker_mode,
                Arc::clone(&session_buffer),
            )
            .await
            {
                eprintln!("❌ Segment processing error: {}", e);
            }
        }
    });

    // Play start beep
    beep_player.play_async(BeepType::RecordingStart).await.ok();

    let mut segmenter = VADSegmenter::new(config.audio_sample_rate);
    let mut last_audio_time = Instant::now();
    let mut silent_interval = tokio::time::interval(Duration::from_secs(30));
    silent_interval.tick().await; // Skip first immediate tick

    loop {
        tokio::select! {
            // Check for shutdown signal
            _ = shutdown_rx.recv() => {
                eprintln!("\n🛑 Stream mode shutting down...");
                if let Some(segment) = segmenter.flush() {
                    if segment_tx.try_send(segment).is_err() {
                        eprintln!("⚠️  Segment queue full during shutdown; dropping trailing segment");
                    }
                }
                break;
            }

            // Process audio chunks as they arrive (async, non-blocking)
            chunk = audio_rx.recv() => {
                match chunk {
                    Some(chunk) => {
                        last_audio_time = Instant::now();

                        if let Some(segment) = segmenter.process_chunk(&chunk) {
                            if segment_tx.try_send(segment).is_err() {
                                eprintln!("⚠️  Segment queue full; dropping incoming segment");
                            }
                        }
                    }
                    None => {
                        // Audio channel closed — recorder stopped
                        eprintln!("\n⚠️  Audio capture ended unexpectedly");
                        break;
                    }
                }
            }

            // Warn if no audio received for 30+ seconds
            _ = silent_interval.tick() => {
                if last_audio_time.elapsed() > Duration::from_secs(30) {
                    eprintln!("⚠️  No audio detected for 30s — mic may be muted or disconnected");
                    last_audio_time = Instant::now();
                }
            }
        }
    }

    drop(segment_tx);
    if let Err(e) = worker.await {
        eprintln!("⚠️  Segment worker terminated unexpectedly: {}", e);
    }

    beep_player.play_async(BeepType::RecordingStop).await.ok();
    eprintln!("✅ Stream mode exited");
    Ok(())
}

fn should_use_mistral_realtime(config: &Config) -> bool {
    config.use_mistral_realtime_stt()
}

fn mistral_realtime_url(config: &Config) -> String {
    let base = config
        .mistral_realtime_base_url
        .clone()
        .or_else(|| config.mistral_base_url.clone())
        .unwrap_or_else(|| "wss://api.mistral.ai".to_string())
        .replace("https://", "wss://")
        .replace("http://", "ws://")
        .trim_end_matches("/v1")
        .trim_end_matches('/')
        .to_string();

    format!(
        "{}/v1/audio/transcriptions/realtime?model={}&target_streaming_delay_ms={}",
        base, config.mistral_realtime_model, config.mistral_realtime_delay_ms
    )
}

fn f32_samples_to_pcm_s16le(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    bytes
}

async fn emit_text(text: &str, pipe_command: Option<&Vec<String>>) {
    // Preserve whitespace in realtime deltas. Providers often send spaces as
    // leading/trailing characters; trimming here makes typed words jam together.
    if text.is_empty() {
        return;
    }

    if let Some(cmd) = pipe_command {
        if let Err(e) = command::execute_with_input(cmd, text).await {
            eprintln!("❌ Pipe command failed: {}", e);
        }
    } else {
        print!("{}", text);
        use std::io::Write;
        std::io::stdout().flush().ok();
    }
}

async fn run_mistral_realtime_stream(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    shutdown_rx: &mut tokio::sync::mpsc::Receiver<()>,
) -> Result<()> {
    run_mistral_realtime_inner(config, pipe_command, shutdown_rx, true, true).await
}

pub async fn run_mistral_realtime_daemon(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
) -> Result<()> {
    run_mistral_realtime_inner(config, pipe_command, control_rx, false, false).await
}

async fn run_mistral_realtime_inner(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
    active_on_start: bool,
    exit_on_signal: bool,
) -> Result<()> {
    let api_key = config
        .mistral_api_key
        .clone()
        .ok_or_else(|| anyhow!("MISTRAL_API_KEY is required for Mistral realtime STT"))?;

    eprintln!("🎙️  dictate realtime mode — Mistral WebSocket STT");
    eprintln!("   Model: {}", config.mistral_realtime_model);
    if active_on_start {
        eprintln!("   Press the shortcut again, send SIGUSR1, or send SIGTERM to stop");
    } else {
        eprintln!("   Warm daemon ready; press Super+R/SIGUSR1 to start or stop typing");
    }

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    let mut request = mistral_realtime_url(config).into_client_request()?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", api_key)
            .parse()
            .map_err(|e| anyhow!("Invalid auth header: {}", e))?,
    );

    let (ws_stream, _) = connect_async(request).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let session_update = serde_json::json!({
        "type": "session.update",
        "session": {
            "audio_format": {
                "encoding": "pcm_s16le",
                "sample_rate": 16000
            }
        }
    });
    ws_write
        .send(Message::Text(session_update.to_string()))
        .await?;

    if config.audio_sample_rate != 16000 || config.audio_channels != 1 {
        eprintln!(
            "⚠️  Mistral realtime requires 16kHz mono capture; overriding AUDIO_SAMPLE_RATE/AUDIO_CHANNELS for this mode"
        );
    }
    let mut recorder =
        AudioRecorder::with_settings(16000, 1, config.audio_buffer_duration_seconds)?;
    let mut audio_rx = if active_on_start {
        Some(recorder.start_continuous()?)
    } else {
        None
    };
    if active_on_start {
        beep_player.play_async(BeepType::RecordingStart).await.ok();
    }

    let mut last_audio_time = Instant::now();
    let mut active = active_on_start;
    let mut silent_interval = tokio::time::interval(Duration::from_secs(30));
    silent_interval.tick().await;
    let overlay = OverlayPublisher::from_env_enabled(config.enable_overlay);
    overlay.set_state(if active_on_start {
        OverlayState::Listening
    } else {
        OverlayState::Idle
    });

    loop {
        // Poll audio asynchronously when active, otherwise park the future
        let audio_fut = async {
            match audio_rx.as_mut() {
                Some(rx) => rx.recv().await,
                None => std::future::pending::<Option<Vec<f32>>>().await,
            }
        };

        tokio::select! {
            _ = control_rx.recv() => {
                if exit_on_signal {
                    eprintln!("\n🛑 Realtime mode shutting down...");
                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.flush"}).to_string())).await.ok();
                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.end"}).to_string())).await.ok();
                    recorder.stop_recording().ok();
                    drop(audio_rx.take());
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                    break;
                }

                active = !active;
                if active {
                    eprintln!("\n▶️  Realtime typing started");
                    audio_rx = Some(recorder.start_continuous()?);
                    last_audio_time = Instant::now();
                    overlay.set_state(OverlayState::Listening);
                    beep_player.play_async(BeepType::RecordingStart).await.ok();
                } else {
                    eprintln!("\n⏹️  Realtime typing stopped");
                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.flush"}).to_string())).await.ok();
                    recorder.stop_recording().ok();
                    audio_rx = None;
                    overlay.set_state(OverlayState::Idle);
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                            match event.get("type").and_then(|t| t.as_str()) {
                                Some("transcription.text.delta") => {
                                    if let Some(delta) = event.get("text").and_then(|t| t.as_str()) {
                                        emit_text(delta, pipe_command).await;
                                    }
                                }
                                Some("transcription.segment") => {
                                    if let Some(segment) = event.get("text").and_then(|t| t.as_str()) {
                                        eprintln!("\n📝 {}", segment.trim());
                                    }
                                }
                                Some("error") => {
                                    eprintln!("❌ Mistral realtime error: {}", event);
                                    beep_player.play_async(BeepType::Error).await.ok();
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => return Err(e.into()),
                    _ => {}
                }
            }
            chunk = audio_fut => {
                if let Some(chunk) = chunk {
                    last_audio_time = Instant::now();
                    if active {
                        overlay.set_level(level_from_samples(&chunk));
                        let pcm = f32_samples_to_pcm_s16le(&chunk);
                        let encoded = base64::engine::general_purpose::STANDARD.encode(pcm);
                        let msg = serde_json::json!({"type":"input_audio.append", "audio": encoded});
                        ws_write.send(Message::Text(msg.to_string())).await?;
                    }
                } else {
                    // Channel closed
                    audio_rx = None;
                }
            }
            _ = silent_interval.tick() => {
                if last_audio_time.elapsed() > Duration::from_secs(30) {
                    eprintln!("⚠️  No audio detected for 30s — mic may be muted or disconnected");
                    last_audio_time = Instant::now();
                }
            }
        }
    }

    eprintln!("✅ Realtime mode exited");
    Ok(())
}

async fn process_segment(
    samples: Vec<f32>,
    config: &Config,
    provider: SharedProvider,
    pipe_command: Option<&Vec<String>>,
    beep_player: &BeepPlayer,
    dictation_mode: &str,
    session_buffer: Arc<Mutex<TranscriptBuffer>>,
) -> Result<()> {
    // Process audio (trim silence, normalize)
    let processor = AudioProcessor::new(config.audio_sample_rate);
    let processed = match processor.process_for_speech_recognition(&samples) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("⚠️  Audio processing failed ({}), using raw samples", e);
            samples // Fallback
        }
    };

    let duration_ms = processed.len() * 1000 / config.audio_sample_rate as usize;
    eprintln!("🧠 Transcribing {}ms segment...", duration_ms);

    // Encode to WAV
    let encoder = WavEncoder::new(config.audio_sample_rate, 1);
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
                if config.context_editing {
                    let backend = OutputBackend::new(pipe_command.cloned());
                    let mut buffer = session_buffer.lock().await;
                    if let Err(e) =
                        handle_final_segment(&backend, &mut buffer, config, text, dictation_mode)
                            .await
                    {
                        eprintln!("❌ Context output failed: {}", e);
                    }
                } else {
                    let processed_text = process_text(text, &config.text_processing);
                    let processed_text = apply_developer_mode(&processed_text, dictation_mode);
                    if processed_text != text {
                        eprintln!("🪄 {}", processed_text.trim());
                    }

                    if let Some(cmd) = pipe_command {
                        match command::execute_with_input(cmd, &processed_text).await {
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
                        println!("{}", processed_text);
                    }
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
