use crate::audio::AudioRecorder;
use crate::audio_processing::AudioProcessor;
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::command;
use crate::config::Config;
use crate::segment_output::emit_finalized_segment;
use crate::transcript::TranscriptBuffer;
use crate::transcription::{SharedProvider, TranscriptionFactory};
use crate::wav::WavEncoder;
use anyhow::{anyhow, Result};
use base64::Engine;
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
        return run_mistral_realtime_stream(config, pipe_command, shutdown_rx, dictation_mode)
            .await;
    }

    if should_use_deepgram_realtime(config) {
        return run_deepgram_realtime_stream(config, pipe_command, shutdown_rx, dictation_mode)
            .await;
    }

    if config.transcription_mode.eq_ignore_ascii_case("realtime") {
        eprintln!(
            "⚠️  Realtime WebSocket STT is available for Mistral and Deepgram; using batch/VAD streaming for {}",
            config.transcription_provider
        );
    }

    eprintln!("🎙️  dictate — polished segments after each pause");
    eprintln!("   {} to stop", crate::control::TOGGLE_HINT);

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
                worker_pipe.clone(),
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

fn peak_audio_level(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

fn should_use_mistral_realtime(config: &Config) -> bool {
    config.use_mistral_realtime_stt()
}

fn realtime_close_hint(
    frame: &tokio_tungstenite::tungstenite::protocol::CloseFrame<'_>,
) -> &'static str {
    let reason = frame.reason.trim();
    if reason.contains("Upstream connection failed") {
        " Check MISTRAL_API_KEY, model name (MISTRAL_REALTIME_MODEL), and that your account can use realtime STT. Try `curl https://api.mistral.ai/v1/models` with your key."
    } else {
        ""
    }
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

fn should_use_deepgram_realtime(config: &Config) -> bool {
    config.use_deepgram_realtime_stt()
}

/// Deepgram streaming endpoint.
///
/// Unlike Mistral there is no session handshake: every option is a query
/// parameter, audio goes up as raw binary PCM frames, and transcripts come back
/// as `Results` messages. `endpointing` is what makes Deepgram emit a final
/// result at a natural pause instead of only at end of stream.
fn deepgram_realtime_url(config: &Config) -> String {
    let base = config
        .deepgram_base_url
        .clone()
        .unwrap_or_else(|| "https://api.deepgram.com".to_string());
    let base = base
        .trim_end_matches('/')
        .replacen("https://", "wss://", 1)
        .replacen("http://", "ws://", 1);

    let mut url = format!(
        "{}/v1/listen?model={}&encoding=linear16&sample_rate=16000&channels=1\
         &smart_format=true&interim_results=false&endpointing=300",
        base, config.deepgram_model
    );
    // `auto` is dictate's sentinel, not a Deepgram code — omitting it lets the
    // model use its own default rather than being rejected.
    let lang = config.transcription_language.trim();
    if !lang.is_empty() && !lang.eq_ignore_ascii_case("auto") {
        url.push_str("&language=");
        url.push_str(lang);
    }
    url
}

async fn run_deepgram_realtime_stream(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    shutdown_rx: &mut tokio::sync::mpsc::Receiver<()>,
    dictation_mode: &str,
) -> Result<()> {
    run_deepgram_realtime_inner(
        config,
        pipe_command,
        shutdown_rx,
        dictation_mode,
        true,
        true,
    )
    .await
}

pub async fn run_deepgram_realtime_daemon(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
    dictation_mode: &str,
    active_on_start: bool,
) -> Result<()> {
    run_deepgram_realtime_inner(
        config,
        pipe_command,
        control_rx,
        dictation_mode,
        active_on_start,
        false,
    )
    .await
}

async fn run_deepgram_realtime_inner(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
    dictation_mode: &str,
    active_on_start: bool,
    exit_on_signal: bool,
) -> Result<()> {
    let api_key = config
        .deepgram_api_key
        .clone()
        .ok_or_else(|| anyhow!("DEEPGRAM_API_KEY is required for Deepgram realtime STT"))?;

    let type_deltas = config.profile == crate::profile::DictateProfile::LiveTyping;

    if config.profile.uses_segment_polish() {
        eprintln!("🎙️  dictate — polished segments (Deepgram realtime)");
    } else {
        eprintln!("🎙️  dictate realtime mode — Deepgram WebSocket STT");
    }
    eprintln!("   Model: {}", config.deepgram_model);
    if active_on_start {
        eprintln!("   {} to stop", crate::control::TOGGLE_HINT);
    } else {
        eprintln!(
            "   Warm daemon ready; {} to start or stop",
            crate::control::TOGGLE_HINT
        );
    }

    let beep_player = BeepPlayer::new(BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    })?;

    let ws_url = deepgram_realtime_url(config);
    if std::env::var("DICTATE_REALTIME_DEBUG").is_ok() {
        eprintln!("[realtime] connecting to {ws_url}");
    }

    let mut request = ws_url.into_client_request()?;
    request.headers_mut().insert(
        "Authorization",
        format!("Token {}", api_key)
            .parse()
            .map_err(|e| anyhow!("Invalid auth header: {}", e))?,
    );
    request.headers_mut().insert(
        "User-Agent",
        format!("dictate/{} (Deepgram realtime)", env!("CARGO_PKG_VERSION"))
            .parse()
            .map_err(|e| anyhow!("Invalid User-Agent: {}", e))?,
    );

    let (ws_stream, _) = connect_async(request)
        .await
        .map_err(|e| anyhow!("Deepgram realtime connect failed: {e}. Check DEEPGRAM_API_KEY."))?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    if config.audio_sample_rate != 16000 || config.audio_channels != 1 {
        eprintln!(
            "⚠️  Deepgram realtime requires 16kHz mono capture; overriding AUDIO_SAMPLE_RATE/AUDIO_CHANNELS for this mode"
        );
    }
    let mut recorder = AudioRecorder::with_settings(16000, 1, config.audio_buffer_duration_seconds)?;
    let mut audio_rx = if active_on_start {
        Some(recorder.start_continuous()?)
    } else {
        None
    };
    if active_on_start {
        beep_player.play_async(BeepType::RecordingStart).await.ok();
    }

    // Re-read after every start: the recorder reports the *requested* rate until
    // the device is actually opened, and a warm daemon opens it on first toggle.
    // Reading once here would pin 16000 while the mic really runs at 48000, and
    // the resample below would become a no-op — Deepgram then receives audio at
    // 3x speed and returns no transcripts at all.
    let mut capture_rate = recorder.capture_sample_rate();
    let session_buffer = Arc::new(Mutex::new(TranscriptBuffer::new()));
    let pipe_owned = pipe_command.cloned();
    let dictation_mode = dictation_mode.to_string();
    let config = config.clone();
    let mut preview_tail = String::new();
    let mut last_audio_time = Instant::now();
    let mut first_audio_logged = false;
    // Deepgram closes an idle socket after ~10s of silence; KeepAlive holds a
    // warm daemon's connection open between dictations.
    let mut keepalive = tokio::time::interval(Duration::from_secs(8));
    keepalive.tick().await;

    loop {
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
                    ws_write
                        .send(Message::Text(
                            serde_json::json!({"type":"CloseStream"}).to_string(),
                        ))
                        .await
                        .ok();
                    recorder.stop_recording().ok();
                    drop(audio_rx.take());
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                    break;
                }

                if audio_rx.is_none() {
                    match recorder.start_continuous() {
                        Ok(rx) => {
                            audio_rx = Some(rx);
                            // Only now does the recorder know the device's real rate.
                            capture_rate = recorder.capture_sample_rate();
                            beep_player.play_async(BeepType::RecordingStart).await.ok();
                            eprintln!("🎙️  Recording… (mic {capture_rate}Hz → 16000Hz)");
                        }
                        Err(e) => eprintln!("❌ Could not start recording: {e}"),
                    }
                } else {
                    // Deepgram buffers the tail of an utterance until it sees more
                    // audio or an explicit flush. Without this the last thing said
                    // before a toggle-off is never transcribed.
                    ws_write
                        .send(Message::Text(
                            serde_json::json!({"type":"Finalize"}).to_string(),
                        ))
                        .await
                        .ok();
                    recorder.stop_recording().ok();
                    drop(audio_rx.take());
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                    eprintln!("⏸️  Stopped; waiting for the next toggle");
                }
            }
            chunk = audio_fut => {
                match chunk {
                    Some(chunk) => {
                        last_audio_time = Instant::now();
                        let chunk = resample_linear(&chunk, capture_rate, 16000);
                        if !first_audio_logged {
                            first_audio_logged = true;
                            if std::env::var("DICTATE_REALTIME_DEBUG").is_ok() {
                                eprintln!(
                                    "[realtime] first audio chunk: {} samples @ {}Hz",
                                    chunk.len(),
                                    capture_rate
                                );
                            }
                        }
                        let pcm = f32_samples_to_pcm_s16le(&chunk);
                        if let Err(e) = ws_write.send(Message::Binary(pcm)).await {
                            eprintln!("❌ Deepgram realtime send failed: {e}");
                            beep_player.play_async(BeepType::Error).await.ok();
                        }
                    }
                    None => audio_rx = None,
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        let value: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        match value.get("type").and_then(|t| t.as_str()) {
                            Some("Results") => {
                                // interim_results is off, so anything with a
                                // transcript here is already final.
                                let transcript = value
                                    .get("channel")
                                    .and_then(|c| c.get("alternatives"))
                                    .and_then(|a| a.get(0))
                                    .and_then(|a| a.get("transcript"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                if !transcript.trim().is_empty() {
                                    handle_realtime_segment(
                                        transcript,
                                        &config,
                                        type_deltas,
                                        pipe_owned.as_ref(),
                                        &session_buffer,
                                        &dictation_mode,
                                        &beep_player,
                                        &mut preview_tail,
                                    )
                                    .await;
                                }
                            }
                            Some("Metadata") | Some("SpeechStarted") | Some("UtteranceEnd") => {}
                            _ => {
                                if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
                                    eprintln!("❌ Deepgram realtime error: {err}");
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
                        eprintln!("⚠️  Deepgram realtime closed: {frame:?}");
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("❌ Deepgram realtime socket error: {e}");
                        break;
                    }
                    None => break,
                }
            }
            _ = keepalive.tick() => {
                if audio_rx.is_none() || last_audio_time.elapsed() > Duration::from_secs(8) {
                    ws_write
                        .send(Message::Text(
                            serde_json::json!({"type":"KeepAlive"}).to_string(),
                        ))
                        .await
                        .ok();
                }
            }
        }
    }

    eprintln!("✅ Realtime mode exited");
    Ok(())
}

fn resample_linear(samples: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if from_hz == to_hz || samples.is_empty() {
        return samples.to_vec();
    }
    let out_len = (samples.len() as u64 * to_hz as u64 / from_hz as u64) as usize;
    let out_len = out_len.max(1);
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * from_hz as f64 / to_hz as f64;
        let idx = src.floor() as usize;
        let frac = src - idx as f64;
        let a = samples[idx.min(samples.len() - 1)];
        let b = samples[(idx + 1).min(samples.len() - 1)];
        out.push((a as f64 * (1.0 - frac) + b as f64 * frac) as f32);
    }
    out
}

fn f32_samples_to_pcm_s16le(samples: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let scaled = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&scaled.to_le_bytes());
    }
    bytes
}

async fn wait_for_realtime_session(
    ws_read: &mut futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(remaining, ws_read.next()).await;
        match msg {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                match event.get("type").and_then(|t| t.as_str()) {
                    Some("session.created") => return Ok(()),
                    Some("error") => {
                        return Err(anyhow!("Mistral realtime session error: {text}"));
                    }
                    _ => continue,
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                let hint = frame.as_ref().map(realtime_close_hint).unwrap_or("");
                return Err(anyhow!(
                    "Mistral realtime closed during handshake: {frame:?}.{hint}"
                ));
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => return Err(anyhow!("Mistral realtime closed before session.ready")),
            Ok(Some(Ok(_))) => continue,
            Err(_) => return Err(anyhow!("Timeout waiting for Mistral realtime session")),
        }
    }
    Err(anyhow!("Timeout waiting for Mistral realtime session"))
}

async fn wait_for_session_updated(
    ws_read: &mut futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let msg = tokio::time::timeout(remaining, ws_read.next()).await;
        match msg {
            Ok(Some(Ok(Message::Text(text)))) => {
                let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                match event.get("type").and_then(|t| t.as_str()) {
                    Some("session.updated") => return Ok(()),
                    Some("error") => return Err(anyhow!("Mistral session.update error: {text}")),
                    _ => continue,
                }
            }
            Ok(Some(Ok(Message::Close(frame)))) => {
                return Err(anyhow!("Mistral closed before session.updated: {frame:?}"));
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => return Err(anyhow!("Mistral closed before session.updated")),
            Ok(Some(Ok(_))) => continue,
            Err(_) => return Err(anyhow!("Timeout waiting for session.updated")),
        }
    }
    Err(anyhow!("Timeout waiting for session.updated"))
}

#[allow(clippy::too_many_arguments)]
async fn emit_preview_flush(
    text: String,
    config: &Config,
    pipe_owned: Option<Vec<String>>,
    session_buffer: Arc<Mutex<TranscriptBuffer>>,
    dictation_mode: String,
    beep_player: &BeepPlayer,
    type_deltas: bool,
) {
    if text.trim().is_empty() {
        return;
    }
    if config.profile.uses_segment_polish() {
        if let Err(e) = emit_finalized_segment(
            config,
            pipe_owned,
            &session_buffer,
            &text,
            &dictation_mode,
            beep_player,
        )
        .await
        {
            eprintln!("❌ Segment output failed: {e}");
        }
    } else if type_deltas {
        let text = crate::text_processing::process_text(&text, &config.text_processing);
        emit_text(&text, pipe_owned.as_ref()).await;
    } else {
        eprintln!("\n📝 {}", text.trim());
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_realtime_segment(
    segment: &str,
    config: &Config,
    type_deltas: bool,
    pipe_owned: Option<&Vec<String>>,
    session_buffer: &Arc<Mutex<TranscriptBuffer>>,
    dictation_mode: &str,
    beep_player: &BeepPlayer,
    preview_tail: &mut String,
) {
    let segment = segment.trim();
    if segment.is_empty() {
        return;
    }
    preview_tail.clear();
    if config.profile.uses_segment_polish() {
        if let Err(e) = emit_finalized_segment(
            config,
            pipe_owned.cloned(),
            session_buffer,
            segment,
            dictation_mode,
            beep_player,
        )
        .await
        {
            eprintln!("❌ Segment output failed: {e}");
        }
    } else if type_deltas {
        let segment = crate::text_processing::process_text(segment, &config.text_processing);
        emit_text(&segment, pipe_owned).await;
    } else {
        eprintln!("\n📝 {segment}");
    }
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
    dictation_mode: &str,
) -> Result<()> {
    run_mistral_realtime_inner(
        config,
        pipe_command,
        shutdown_rx,
        dictation_mode,
        true,
        true,
    )
    .await
}

pub async fn run_mistral_realtime_daemon(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
    dictation_mode: &str,
    active_on_start: bool,
) -> Result<()> {
    run_mistral_realtime_inner(
        config,
        pipe_command,
        control_rx,
        dictation_mode,
        active_on_start,
        false,
    )
    .await
}

async fn run_mistral_realtime_inner(
    config: &Config,
    pipe_command: Option<&Vec<String>>,
    control_rx: &mut tokio::sync::mpsc::Receiver<()>,
    dictation_mode: &str,
    active_on_start: bool,
    exit_on_signal: bool,
) -> Result<()> {
    let api_key = config
        .mistral_api_key
        .clone()
        .ok_or_else(|| anyhow!("MISTRAL_API_KEY is required for Mistral realtime STT"))?;

    let type_deltas = config.profile == crate::profile::DictateProfile::LiveTyping;

    if config.profile.uses_segment_polish() {
        eprintln!("🎙️  dictate — polished segments (Mistral realtime)");
    } else {
        eprintln!("🎙️  dictate realtime mode — Mistral WebSocket STT");
    }
    eprintln!("   Model: {}", config.mistral_realtime_model);
    if active_on_start {
        eprintln!("   {} to stop", crate::control::TOGGLE_HINT);
    } else {
        eprintln!(
            "   Warm daemon ready; {} to start or stop",
            crate::control::TOGGLE_HINT
        );
    }

    let beep_config = BeepConfig {
        enabled: config.enable_audio_feedback,
        volume: config.beep_volume,
    };
    let beep_player = BeepPlayer::new(beep_config)?;

    let ws_url = mistral_realtime_url(config);
    if std::env::var("DICTATE_REALTIME_DEBUG").is_ok() {
        eprintln!("[realtime] connecting to {ws_url}");
    }

    let mut request = ws_url.into_client_request()?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", api_key)
            .parse()
            .map_err(|e| anyhow!("Invalid auth header: {}", e))?,
    );
    request.headers_mut().insert(
        "User-Agent",
        format!("dictate/{} (Mistral realtime)", env!("CARGO_PKG_VERSION"))
            .parse()
            .map_err(|e| anyhow!("Invalid User-Agent: {}", e))?,
    );

    let (ws_stream, _) = connect_async(request).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    // Mistral sends session.created first; do not send session.update before that.
    wait_for_realtime_session(&mut ws_read).await?;

    let session_update = serde_json::json!({
        "type": "session.update",
        "session": {
            "audio_format": {
                "encoding": "pcm_s16le",
                "sample_rate": 16000
            },
            "target_streaming_delay_ms": config.mistral_realtime_delay_ms
        }
    });
    ws_write
        .send(Message::Text(session_update.to_string()))
        .await?;

    wait_for_session_updated(&mut ws_read).await?;

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
    let session_buffer = Arc::new(Mutex::new(TranscriptBuffer::new()));
    let pipe_owned = pipe_command.cloned();
    let dictation_mode = dictation_mode.to_string();
    let config = config.clone();
    let mut preview_tail = String::new();
    let mut audio_send_ready = true;
    let mut drain_flush_until: Option<Instant> = None;
    let mut first_audio_logged = false;
    // Must be re-read after each start: until the device is opened the recorder
    // reports the *requested* rate, so a warm daemon would pin 16000 while the
    // mic actually runs at 48000 and the resample below would silently no-op.
    let mut capture_rate = recorder.capture_sample_rate();

    loop {
        if let Some(until) = drain_flush_until {
            if Instant::now() >= until {
                drain_flush_until = None;
                let text = std::mem::take(&mut preview_tail);
                if !text.trim().is_empty() {
                    emit_preview_flush(
                        text,
                        &config,
                        pipe_owned.clone(),
                        Arc::clone(&session_buffer),
                        dictation_mode.clone(),
                        &beep_player,
                        type_deltas,
                    )
                    .await;
                }
            }
        }

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

                if !active_on_start && audio_rx.is_none() {
                    if drain_flush_until.take().is_some() {
                        let pending = std::mem::take(&mut preview_tail);
                        if !pending.trim().is_empty() {
                            emit_preview_flush(
                                pending,
                                &config,
                                pipe_owned.clone(),
                                Arc::clone(&session_buffer),
                                dictation_mode.clone(),
                                &beep_player,
                                type_deltas,
                            )
                            .await;
                        }
                    } else {
                        preview_tail.clear();
                    }
                    session_buffer.lock().await.clear();
                    eprintln!("\n▶️  Dictation started");
                    audio_rx = Some(recorder.start_continuous()?);
                    capture_rate = recorder.capture_sample_rate();
                    last_audio_time = Instant::now();
                    active = true;
                    audio_send_ready = true;
                    beep_player.play_async(BeepType::RecordingStart).await.ok();
                    continue;
                }

                active = !active;
                if active {
                    if drain_flush_until.take().is_some() {
                        let pending = std::mem::take(&mut preview_tail);
                        if !pending.trim().is_empty() {
                            emit_preview_flush(
                                pending,
                                &config,
                                pipe_owned.clone(),
                                Arc::clone(&session_buffer),
                                dictation_mode.clone(),
                                &beep_player,
                                type_deltas,
                            )
                            .await;
                        }
                    } else {
                        preview_tail.clear();
                    }
                    session_buffer.lock().await.clear();
                    eprintln!("\n▶️  Dictation started");
                    audio_rx = Some(recorder.start_continuous()?);
                    capture_rate = recorder.capture_sample_rate();
                    last_audio_time = Instant::now();
                    beep_player.play_async(BeepType::RecordingStart).await.ok();
                } else {
                    eprintln!("\n⏹️  Dictation stopped");
                    recorder.stop_recording().ok();
                    audio_rx = None;
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                    ws_write
                        .send(Message::Text(
                            serde_json::json!({"type":"input_audio.flush"}).to_string(),
                        ))
                        .await
                        .ok();
                    drain_flush_until =
                        Some(Instant::now() + Duration::from_millis(3500));
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                            match event.get("type").and_then(|t| t.as_str()) {
                                Some("transcription.text.delta") => {
                                    if let Some(delta) =
                                        event.get("text").and_then(|t| t.as_str())
                                    {
                                        if type_deltas {
                                            let delta = crate::text_processing::process_text(
                                                delta,
                                                &config.text_processing,
                                            );
                                            emit_text(&delta, pipe_owned.as_ref()).await;
                                        } else if config.profile.uses_segment_polish() {
                                            preview_tail.push_str(delta);
                                        }
                                    }
                                }
                                Some("transcription.segment") => {
                                    // Live typing already emitted via text.delta; segment is duplicate.
                                    if type_deltas {
                                        preview_tail.clear();
                                        continue;
                                    }
                                    if let Some(segment) =
                                        event.get("text").and_then(|t| t.as_str())
                                    {
                                        handle_realtime_segment(
                                            segment,
                                            &config,
                                            type_deltas,
                                            pipe_owned.as_ref(),
                                            &session_buffer,
                                            &dictation_mode,
                                            &beep_player,
                                            &mut preview_tail,
                                        )
                                        .await;
                                    }
                                }
                                Some("transcription.done") => {
                                    if !preview_tail.trim().is_empty() {
                                        let tail = std::mem::take(&mut preview_tail);
                                        if config.profile.uses_segment_polish() {
                                            if let Err(e) = emit_finalized_segment(
                                                &config,
                                                pipe_owned.clone(),
                                                &session_buffer,
                                                &tail,
                                                &dictation_mode,
                                                &beep_player,
                                            )
                                            .await
                                            {
                                                eprintln!("❌ Segment output failed: {e}");
                                            }
                                        } else if type_deltas {
                                            let tail = crate::text_processing::process_text(
                                                &tail,
                                                &config.text_processing,
                                            );
                                            emit_text(&tail, pipe_owned.as_ref()).await;
                                        } else {
                                            eprintln!("\n📝 {}", tail.trim());
                                        }
                                    }
                                }
                                Some("session.created") | Some("session.updated") => {}
                                Some("error") => {
                                    eprintln!("❌ Mistral realtime error: {}", event);
                                    beep_player.play_async(BeepType::Error).await.ok();
                                    audio_send_ready = false;
                                }
                                Some(other)
                                    if std::env::var("DICTATE_REALTIME_DEBUG").is_ok()
                                        || matches!(
                                            other,
                                            "transcription.language"
                                                | "input_audio_buffer.speech_started"
                                                | "input_audio_buffer.speech_stopped"
                                        ) =>
                                {
                                    eprintln!("[realtime] {other}: {text}");
                                }
                                Some(_) => {}
                                None => {}
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
                    if active && audio_send_ready {
                        let chunk = if capture_rate != 16_000 {
                            resample_linear(&chunk, capture_rate, 16_000)
                        } else {
                            chunk
                        };
                        if !first_audio_logged {
                            first_audio_logged = true;
                            let lvl = peak_audio_level(&chunk);
                            eprintln!(
                                "🎤 Audio streaming to Mistral (capture {} Hz → 16 kHz, level {:.2})",
                                capture_rate,
                                lvl
                            );
                        }
                        let pcm = f32_samples_to_pcm_s16le(&chunk);
                        let encoded = base64::engine::general_purpose::STANDARD.encode(pcm);
                        let msg = serde_json::json!({"type":"input_audio.append", "audio": encoded});
                        if let Err(e) = ws_write.send(Message::Text(msg.to_string())).await {
                            eprintln!("❌ Mistral realtime send failed: {e}");
                            audio_send_ready = false;
                            beep_player.play_async(BeepType::Error).await.ok();
                        }
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

#[allow(clippy::too_many_arguments)]
async fn process_segment(
    samples: Vec<f32>,
    config: &Config,
    provider: SharedProvider,
    pipe_command: Option<Vec<String>>,
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
            if let Err(e) = emit_finalized_segment(
                config,
                pipe_command,
                &session_buffer,
                &text,
                dictation_mode,
                beep_player,
            )
            .await
            {
                eprintln!("❌ Segment output failed: {e}");
            }
        }
        Err(e) => {
            eprintln!("❌ Transcription error: {}", e);
            beep_player.play_async(BeepType::Error).await.ok();
        }
    }

    Ok(())
}
