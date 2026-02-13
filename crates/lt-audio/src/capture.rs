use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lt_core::AudioChunk;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::error::{AudioError, Result};
use crate::resampler::AudioResampler;
use crate::vad::{AudioLevel, VadProcessor};

/// Audio capture with pipeline architecture:
/// cpal callback → bounded channel (64) → processing task (resample + VAD) → bounded channel (32)
pub struct AudioCapture {
    // Stream handle (kept alive while capturing)
    stream: Option<cpal::Stream>,

    // Channels
    chunk_rx: Option<mpsc::Receiver<AudioChunk>>,
    level_rx: Option<mpsc::Receiver<AudioLevel>>,

    // State
    is_running: Arc<AtomicBool>,
    session_start_ms: Arc<AtomicU64>,

    // Processing task handle
    processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl AudioCapture {
    /// Create a new AudioCapture instance
    pub fn new() -> Self {
        Self {
            stream: None,
            chunk_rx: None,
            level_rx: None,
            is_running: Arc::new(AtomicBool::new(false)),
            session_start_ms: Arc::new(AtomicU64::new(0)),
            processing_task: None,
        }
    }

    /// Start audio capture
    pub fn start(&mut self) -> Result<()> {
        if self.is_running.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyRunning);
        }

        info!("Starting audio capture");

        // Get default input device
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(AudioError::NoInputDevice)?;

        let device_name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "Unknown".to_string());
        info!("Using audio input device: {}", device_name);

        // Get default input config
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate();
        let channels = config.channels() as usize;

        info!(
            "Audio config: {} Hz, {} channels, format: {:?}",
            sample_rate,
            channels,
            config.sample_format()
        );

        // Create channels for pipeline
        // Stage 1: cpal callback → raw_tx (capacity 64)
        let (raw_tx, raw_rx) = mpsc::channel::<Vec<i16>>(64);

        // Stage 2: processing task → chunk_tx (capacity 32) and level_tx (capacity 32)
        let (chunk_tx, chunk_rx) = mpsc::channel::<AudioChunk>(32);
        let (level_tx, level_rx) = mpsc::channel::<AudioLevel>(32);

        // Store receivers
        self.chunk_rx = Some(chunk_rx);
        self.level_rx = Some(level_rx);

        // Set session start time
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.session_start_ms.store(start_time, Ordering::SeqCst);

        // Build audio stream based on sample format
        let is_running = Arc::clone(&self.is_running);
        is_running.store(true, Ordering::SeqCst);

        let stream = match config.sample_format() {
            cpal::SampleFormat::I16 => self.build_stream_i16(&device, &config, raw_tx)?,
            cpal::SampleFormat::U16 => self.build_stream_u16(&device, &config, raw_tx)?,
            cpal::SampleFormat::F32 => self.build_stream_f32(&device, &config, raw_tx)?,
            format => {
                return Err(AudioError::UnsupportedFormat(format!("{:?}", format)));
            }
        };

        // Start the stream
        stream.play()?;
        self.stream = Some(stream);

        // Spawn processing task
        let is_running_clone = Arc::clone(&is_running);
        let session_start = Arc::clone(&self.session_start_ms);

        let processing_task = tokio::spawn(async move {
            Self::processing_loop(
                raw_rx,
                chunk_tx,
                level_tx,
                sample_rate,
                channels,
                is_running_clone,
                session_start,
            )
            .await;
        });

        self.processing_task = Some(processing_task);

        info!("Audio capture started successfully");
        Ok(())
    }

    /// Stop audio capture
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running.load(Ordering::SeqCst) {
            return Err(AudioError::NotStarted);
        }

        info!("Stopping audio capture");

        self.is_running.store(false, Ordering::SeqCst);

        // Drop stream to stop audio callbacks
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }

        // Wait for processing task to finish
        if let Some(task) = self.processing_task.take() {
            // We can't block here in sync context, so we just drop it
            // The task will finish when the raw_rx channel closes
            drop(task);
        }

        info!("Audio capture stopped");
        Ok(())
    }

    /// Subscribe to audio chunks (resampled 16kHz mono with VAD state)
    pub fn subscribe_chunks(&mut self) -> Option<mpsc::Receiver<AudioChunk>> {
        self.chunk_rx.take()
    }

    /// Subscribe to audio levels (for waveform visualization)
    pub fn subscribe_levels(&mut self) -> Option<mpsc::Receiver<AudioLevel>> {
        self.level_rx.take()
    }

    /// Check if capture is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Build audio input stream for i16 samples
    fn build_stream_i16(
        &self,
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        raw_tx: mpsc::Sender<Vec<i16>>,
    ) -> Result<cpal::Stream> {
        let config = config.config();
        let err_fn = |err| error!("Audio stream error: {}", err);

        let data_callback = move |data: &[i16], _: &cpal::InputCallbackInfo| {
            let samples = data.to_vec();
            if let Err(_) = raw_tx.try_send(samples) {
                warn!("Audio buffer full, dropping frame");
            }
        };

        let stream = device.build_input_stream(&config, data_callback, err_fn, None)?;
        Ok(stream)
    }

    /// Build audio input stream for u16 samples
    fn build_stream_u16(
        &self,
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        raw_tx: mpsc::Sender<Vec<i16>>,
    ) -> Result<cpal::Stream> {
        let config = config.config();
        let err_fn = |err| error!("Audio stream error: {}", err);

        let data_callback = move |data: &[u16], _: &cpal::InputCallbackInfo| {
            let samples: Vec<i16> = data
                .iter()
                .map(|&sample| {
                    // Convert u16 to i16 (shift range)
                    (sample as i32 - 32768) as i16
                })
                .collect();
            if let Err(_) = raw_tx.try_send(samples) {
                warn!("Audio buffer full, dropping frame");
            }
        };

        let stream = device.build_input_stream(&config, data_callback, err_fn, None)?;
        Ok(stream)
    }

    /// Build audio input stream for f32 samples
    fn build_stream_f32(
        &self,
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        raw_tx: mpsc::Sender<Vec<i16>>,
    ) -> Result<cpal::Stream> {
        let config = config.config();
        let err_fn = |err| error!("Audio stream error: {}", err);

        let data_callback = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let samples: Vec<i16> = data
                .iter()
                .map(|&sample| {
                    let clamped = sample.clamp(-1.0, 1.0);
                    (clamped * i16::MAX as f32) as i16
                })
                .collect();
            if let Err(_) = raw_tx.try_send(samples) {
                warn!("Audio buffer full, dropping frame");
            }
        };

        let stream = device.build_input_stream(&config, data_callback, err_fn, None)?;
        Ok(stream)
    }

    /// Processing loop: resample + VAD
    async fn processing_loop(
        mut raw_rx: mpsc::Receiver<Vec<i16>>,
        chunk_tx: mpsc::Sender<AudioChunk>,
        level_tx: mpsc::Sender<AudioLevel>,
        sample_rate: u32,
        channels: usize,
        is_running: Arc<AtomicBool>,
        session_start: Arc<AtomicU64>,
    ) {
        debug!(
            "Processing loop started: {} Hz, {} channels",
            sample_rate, channels
        );

        // Create resampler (target: 16kHz mono)
        let mut resampler = match AudioResampler::new(sample_rate, 16000, channels) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to create resampler: {}", e);
                return;
            }
        };

        // Create VAD processor (threshold: 0.02 for normalized audio)
        let vad = VadProcessor::new(0.02);

        let start_ms = session_start.load(Ordering::SeqCst);

        while is_running.load(Ordering::SeqCst) {
            // Receive raw audio from cpal callback
            let raw_samples = match raw_rx.recv().await {
                Some(samples) => samples,
                None => {
                    debug!("Raw audio channel closed");
                    break;
                }
            };

            // Calculate timestamp
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let timestamp_ms = now_ms - start_ms;

            // Resample to 16kHz mono
            let resampled = match resampler.resample(&raw_samples) {
                Ok(samples) => samples,
                Err(e) => {
                    error!("Resampling error: {}", e);
                    continue;
                }
            };

            // Calculate audio level and VAD
            let audio_level = vad.process(&resampled, timestamp_ms);

            // Send audio level (non-blocking)
            if let Err(_) = level_tx.try_send(audio_level) {
                // Level channel full - skip this update
                // UI updates can be dropped without issue
            }

            // Send audio chunk (non-blocking)
            let chunk = AudioChunk {
                data: resampled,
                timestamp_ms,
            };

            if let Err(_) = chunk_tx.try_send(chunk) {
                // Chunk channel full - this is more critical but we still don't want to block
                warn!("Audio chunk channel full, dropping chunk");
            }
        }

        debug!("Processing loop finished");
    }
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        if self.is_running.load(Ordering::SeqCst) {
            let _ = self.stop();
        }
    }
}
