use crate::audio::AudioRecorder;
use crate::audio_processing::AudioProcessor;
use crate::beep::{BeepConfig, BeepPlayer, BeepType};
use crate::config::Config;
use crate::editing::{apply_edit, plan_edit, EditPolicy, TextEdit};
use crate::intent::{detect_intent, DictationIntent};
use crate::transcript::TranscriptBuffer;
use crate::transcription::{TranscriptionFactory, TranscriptionProvider};
use crate::typing::{OutputBackend, TypingBackend};
use crate::wav::WavEncoder;
use anyhow::{anyhow, Result};
use base64::Engine;
use futures::{SinkExt, StreamExt};
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
                    if lookback > 0 {
                        let start_idx = self.ring_buffer.len() - lookback;
                        for f in &self.ring_buffer[start_idx..] {
                            self.speech_buffer.extend_from_slice(f);
                        }
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
            if self.ring_buffer.len() >= RING_FRAMES {
                self.ring_buffer.remove(0);
            }
            self.ring_buffer.push(frame);
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
    let transcript = Arc::new(Mutex::new(TranscriptBuffer::new()));

    loop {
        tokio::select! {
            // Check for shutdown signal
            _ = shutdown_rx.recv() => {
                eprintln!("\n🛑 Stream mode shutting down...");
                if let Some(segment) = segmenter.flush() {
                    let _ = process_segment(segment, config, Arc::clone(&provider), pipe_command, &beep_player, Arc::clone(&transcript)).await;
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
                            let transcript_clone = Arc::clone(&transcript);

                            tokio::spawn(async move {
                                if let Err(e) = process_segment(
                                    segment,
                                    &config_clone,
                                    provider_clone,
                                    pipe_clone.as_ref(),
                                    &beep_clone,
                                    transcript_clone,
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

    recorder.stop_recording()?;
    recorder.clear_buffer()?;

    eprintln!("✅ Stream mode exited");
    Ok(())
}

fn should_use_mistral_realtime(config: &Config) -> bool {
    config
        .transcription_provider
        .eq_ignore_ascii_case("mistral")
        && !config.batch_mode
        && !config.transcription_mode.eq_ignore_ascii_case("batch")
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
        "{}/v1/audio/transcriptions/realtime?model={}",
        base, config.mistral_realtime_model
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

fn normalize_insert_spacing(previous: &str, next: &str) -> String {
    if previous.is_empty() || next.is_empty() {
        return next.to_string();
    }
    let prev_needs_space = !previous.ends_with(char::is_whitespace);
    let next_allows_space =
        !next.starts_with(char::is_whitespace) && !next.starts_with(['.', ',', '!', '?', ':', ';']);
    if prev_needs_space && next_allows_space {
        format!(" {}", next)
    } else {
        next.to_string()
    }
}

fn edit_policy(config: &Config) -> EditPolicy {
    EditPolicy {
        max_delete_chars: config.context_editing_max_delete_chars,
        max_delete_words: config.context_editing_max_delete_words,
        type_rejected_commands: config.context_editing_type_rejected_commands,
    }
}

async fn handle_final_text<B: TypingBackend + ?Sized>(
    backend: &B,
    buffer: &mut TranscriptBuffer,
    config: &Config,
    text: &str,
) -> Result<()> {
    let mut intent = detect_intent(text);
    if let DictationIntent::InsertText(insert_text) = intent {
        intent = DictationIntent::InsertText(normalize_insert_spacing(buffer.text(), &insert_text));
    }
    let policy = edit_policy(config);
    let edit = plan_edit(buffer, intent, policy);
    apply_edit(backend, buffer, edit, policy).await
}

fn live_correction_edit(buffer: &TranscriptBuffer) -> Option<TextEdit> {
    let text = buffer.text();
    let lower = text.to_lowercase();
    let markers = [
        "actually scratch that",
        "scratch that",
        "actually no",
        "no i mean",
        "i mean",
    ];

    for marker in markers {
        let Some(marker_start) = lower.rfind(marker) else {
            continue;
        };
        let after_marker = marker_start + marker.len();
        let replacement = text[after_marker..]
            .trim_start_matches(|ch: char| {
                ch.is_whitespace() || matches!(ch, '.' | ',' | '!' | '?' | ':' | ';')
            })
            .trim();
        let replacement = normalize_replacement_phrase(replacement);
        if replacement.is_empty() {
            continue;
        }

        let replacement_words = replacement.split_whitespace().count().max(1);
        let target_words = replacement_words.min(4);
        let before_command = &text[..marker_start];
        let target_range = last_words_range_in_text(before_command, target_words)?;
        let replacement = normalize_insert_spacing(&text[..target_range.start], &replacement);
        return Some(TextEdit::ReplaceSuffix {
            range: target_range.start..text.len(),
            replacement,
        });
    }

    None
}

fn normalize_replacement_phrase(replacement: &str) -> String {
    let trimmed = replacement.trim();
    let normalized = trimmed
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_punctuation() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    for prefix in [
        "let s make it ",
        "lets make it ",
        "let us make it ",
        "make it ",
        "change it to ",
        "change that to ",
        "to ",
        "by ",
    ] {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }

    normalized
}

fn last_words_range_in_text(text: &str, count: usize) -> Option<std::ops::Range<usize>> {
    if count == 0 || text.trim_end().is_empty() {
        return None;
    }

    let end = text.trim_end_matches(char::is_whitespace).len();
    let prefix = &text[..end];
    let mut ranges = Vec::new();
    let mut in_word = false;
    let mut word_end = end;

    for (idx, ch) in prefix.char_indices().rev() {
        if ch.is_whitespace() {
            if in_word {
                ranges.push(idx + ch.len_utf8()..word_end);
                in_word = false;
                if ranges.len() == count {
                    break;
                }
            }
        } else if !in_word {
            in_word = true;
            word_end = idx + ch.len_utf8();
        }
    }

    if in_word && ranges.len() < count {
        ranges.push(0..word_end);
    }

    if ranges.len() < count {
        return None;
    }

    let start = ranges.last().unwrap().start;
    Some(start..end)
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

    // The raw Mistral realtime WebSocket first emits session.created. Only then
    // update the session with audio format and latency settings.
    loop {
        match ws_read.next().await {
            Some(Ok(Message::Text(text))) => {
                let event: serde_json::Value = serde_json::from_str(&text)?;
                match event.get("type").and_then(|t| t.as_str()) {
                    Some("session.created") => {
                        eprintln!("✅ Mistral realtime session created");
                        break;
                    }
                    Some("error") => {
                        return Err(anyhow!("Mistral realtime handshake error: {}", event));
                    }
                    _ => {}
                }
            }
            Some(Ok(Message::Close(frame))) => {
                return Err(anyhow!(
                    "Mistral realtime WebSocket closed during handshake: {:?}",
                    frame
                ));
            }
            Some(Ok(_)) => {}
            Some(Err(e)) => return Err(e.into()),
            None => {
                return Err(anyhow!(
                    "Mistral realtime WebSocket closed during handshake"
                ))
            }
        }
    }

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

    let mut recorder = AudioRecorder::new()?;
    let mut audio_rx: Option<std::sync::mpsc::Receiver<Vec<f32>>> = None;
    if active_on_start {
        audio_rx = Some(recorder.start_continuous()?);
        beep_player.play_async(BeepType::RecordingStart).await.ok();
    }

    let mut last_audio_time = Instant::now();
    let mut active = active_on_start;
    let backend = OutputBackend::new(pipe_command.cloned());
    let mut transcript = TranscriptBuffer::new();

    loop {
        tokio::select! {
            _ = control_rx.recv() => {
                if exit_on_signal {
                    eprintln!("\n🛑 Realtime mode shutting down...");

                    // Release the microphone immediately when the user stops dictation.
                    // Network flush/end and feedback sounds can take extra time and must
                    // not keep the CPAL input stream alive during shutdown.
                    recorder.stop_recording()?;
                    drop(audio_rx.take());

                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.flush"}).to_string())).await.ok();
                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.end"}).to_string())).await.ok();
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                    break;
                }

                active = !active;
                if active {
                    eprintln!("\n▶️  Realtime typing started");
                    beep_player.play_async(BeepType::RecordingStart).await.ok();
                    if audio_rx.is_none() {
                        audio_rx = Some(recorder.start_continuous()?);
                    }
                } else {
                    eprintln!("\n⏹️  Realtime typing stopped");

                    // Stop CPAL before websocket cleanup so desktop mic indicators clear
                    // as soon as the shortcut toggles recording off.
                    recorder.stop_recording()?;
                    audio_rx = None;

                    ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.flush"}).to_string())).await.ok();
                    if !config.realtime_output_mode.eq_ignore_ascii_case("delta") {
                        ws_write.send(Message::Text(serde_json::json!({"type":"input_audio.end"}).to_string())).await.ok();
                    }
                    beep_player.play_async(BeepType::RecordingStop).await.ok();
                }
            }
            maybe_msg = ws_read.next() => {
                match maybe_msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&text) {
                            match event.get("type").and_then(|t| t.as_str()) {
                                Some("transcription.text.delta")
                                    if config.realtime_output_mode.eq_ignore_ascii_case("delta") =>
                                {
                                    if let Some(delta) = event.get("text").and_then(|t| t.as_str()) {
                                        if let Err(e) = backend.type_text(delta).await {
                                            eprintln!("❌ Output failed: {}", e);
                                        } else {
                                            transcript.append_typed(delta);
                                            if let Some(edit) = live_correction_edit(&transcript) {
                                                if let Err(e) = apply_edit(&backend, &mut transcript, edit, edit_policy(config)).await {
                                                    eprintln!("❌ Live correction failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                                Some("transcription.done") | Some("transcription.segment") => {
                                    if let Some(segment) = event.get("text").and_then(|t| t.as_str()) {
                                        let segment = segment.trim();
                                        if !segment.is_empty() {
                                            eprintln!("\n📝 {}", segment);
                                            if config.realtime_output_mode.eq_ignore_ascii_case("stable") {
                                                if let Err(e) = handle_final_text(&backend, &mut transcript, config, segment).await {
                                                    eprintln!("❌ Output failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                                Some("error") => {
                                    eprintln!("❌ Mistral realtime error: {}", event);
                                    beep_player.play_async(BeepType::Error).await.ok();
                                }
                                Some(other) => {
                                    if std::env::var("DICTATE_DEBUG_REALTIME").is_ok() {
                                        eprintln!("🔎 Mistral realtime event: {} {}", other, event);
                                    }
                                }
                                None => {
                                    if std::env::var("DICTATE_DEBUG_REALTIME").is_ok() {
                                        eprintln!("🔎 Mistral realtime event without type: {}", event);
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | Option::None => break,
                    Some(Err(e)) => return Err(e.into()),
                    _ => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                if let Some(ref rx) = audio_rx {
                    match rx.recv_timeout(Duration::from_millis(30)) {
                        Ok(chunk) => {
                            last_audio_time = Instant::now();
                            let pcm = f32_samples_to_pcm_s16le(&chunk);
                            let encoded = base64::engine::general_purpose::STANDARD.encode(pcm);
                            let msg = serde_json::json!({"type":"input_audio.append", "audio": encoded});
                            ws_write.send(Message::Text(msg.to_string())).await?;
                        }
                        Err(_) => {
                            if last_audio_time.elapsed() > Duration::from_secs(30) {
                                eprintln!("⚠️  No audio detected for 30s — mic may be muted or disconnected");
                                last_audio_time = Instant::now();
                            }
                        }
                    }
                }
            }
        }
    }

    recorder.stop_recording()?;
    recorder.clear_buffer()?;

    eprintln!("✅ Realtime mode exited");
    Ok(())
}

async fn process_segment(
    samples: Vec<f32>,
    config: &Config,
    provider: Arc<Mutex<Box<dyn TranscriptionProvider>>>,
    pipe_command: Option<&Vec<String>>,
    beep_player: &BeepPlayer,
    transcript: Arc<Mutex<TranscriptBuffer>>,
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

                let backend = OutputBackend::new(pipe_command.cloned());
                let mut buffer = transcript.lock().await;
                if let Err(e) = handle_final_text(&backend, &mut buffer, config, text).await {
                    eprintln!("❌ Output failed: {}", e);
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
