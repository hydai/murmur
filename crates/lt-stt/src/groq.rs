use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

use crate::chunker::AudioChunker;

/// Groq Whisper API response
#[derive(Debug, Deserialize)]
struct GroqResponse {
    text: String,
}

/// Groq Whisper Turbo REST API client
/// Provides 216x real-time speed transcription
pub struct GroqProvider {
    api_key: String,
    model: String,
    chunker: Arc<Mutex<AudioChunker>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioChunk>>>>,
    event_tx: Arc<Mutex<Option<mpsc::Sender<TranscriptionEvent>>>>,
    event_rx: Arc<Mutex<Option<mpsc::Receiver<TranscriptionEvent>>>>,
    processing_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl GroqProvider {
    /// Create a new Groq Whisper provider
    ///
    /// # Arguments
    /// * `api_key` - Groq API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "whisper-large-v3-turbo".to_string(),
            chunker: Arc::new(Mutex::new(AudioChunker::new(3000))), // 3 second chunks (faster than OpenAI)
            audio_tx: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            event_rx: Arc::new(Mutex::new(None)),
            processing_task: Arc::new(Mutex::new(None)),
        }
    }

    /// Transcribe audio via Groq Whisper API
    async fn transcribe_audio(&self, wav_bytes: Vec<u8>) -> Result<String> {
        let client = reqwest::Client::new();

        // Create multipart form
        let part = Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| MurmurError::Stt(format!("Failed to create multipart part: {}", e)))?;

        let form = Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        // Send request to Groq API
        let response = client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| MurmurError::Stt(format!("Groq API request failed: {}", e)))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(MurmurError::Stt(format!(
                "Groq API error ({}): {}",
                status, error_text
            )));
        }

        // Parse response
        let groq_response: GroqResponse = response
            .json()
            .await
            .map_err(|e| MurmurError::Stt(format!("Failed to parse Groq response: {}", e)))?;

        Ok(groq_response.text)
    }
}

#[async_trait]
impl SttProvider for GroqProvider {
    async fn start_session(&mut self) -> Result<()> {
        info!("Starting Groq Whisper Turbo session");

        // Reset chunker
        *self.chunker.lock().await = AudioChunker::new(3000);

        // Create channel for audio chunks
        let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(32);
        *self.audio_tx.lock().await = Some(audio_tx);

        // Create channel for transcription events
        let (event_tx, event_rx) = mpsc::channel::<TranscriptionEvent>(32);
        *self.event_tx.lock().await = Some(event_tx.clone());
        *self.event_rx.lock().await = Some(event_rx);

        // Clone necessary data for the processing task
        let chunker = self.chunker.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();

        // Spawn processing task
        let task = tokio::spawn(async move {
            let mut last_timestamp_ms = 0u64;
            let mut accumulated_text = String::new();

            // Create a temporary provider for API calls
            let temp_provider = GroqProvider {
                api_key: api_key.clone(),
                model: model.clone(),
                chunker: Arc::new(Mutex::new(AudioChunker::new(3000))),
                audio_tx: Arc::new(Mutex::new(None)),
                event_tx: Arc::new(Mutex::new(None)),
                event_rx: Arc::new(Mutex::new(None)),
                processing_task: Arc::new(Mutex::new(None)),
            };

            while let Some(chunk) = audio_rx.recv().await {
                last_timestamp_ms = chunk.timestamp_ms;

                // Add chunk to buffer
                {
                    let mut chunker_guard = chunker.lock().await;
                    chunker_guard.add_chunk(&chunk);

                    // Check if we should flush
                    if chunker_guard.should_flush(chunk.timestamp_ms) {
                        debug!("Flushing audio chunk for Groq transcription");

                        match chunker_guard.flush() {
                            Ok(wav_bytes) if !wav_bytes.is_empty() => {
                                // Send to Groq API (216x real-time speed!)
                                match temp_provider.transcribe_audio(wav_bytes).await {
                                    Ok(text) => {
                                        if !text.trim().is_empty() {
                                            debug!("Groq transcription result: {}", text);

                                            // Accumulate text
                                            if !accumulated_text.is_empty() {
                                                accumulated_text.push(' ');
                                            }
                                            accumulated_text.push_str(&text);

                                            // Send partial event
                                            let event = TranscriptionEvent::Partial {
                                                text: accumulated_text.clone(),
                                                timestamp_ms: chunk.timestamp_ms,
                                            };

                                            if let Err(e) = event_tx.send(event).await {
                                                error!("Failed to send partial event: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Groq transcription failed: {}", e);
                                        let event = TranscriptionEvent::Error {
                                            message: format!("Groq API error: {}", e),
                                        };
                                        let _ = event_tx.send(event).await;
                                    }
                                }
                            }
                            Ok(_) => {
                                debug!("Empty WAV bytes, skipping transcription");
                            }
                            Err(e) => {
                                error!("Failed to flush audio buffer: {}", e);
                            }
                        }
                    }
                }
            }

            // Flush any remaining audio
            debug!("Audio stream ended, flushing remaining audio");
            {
                let mut chunker_guard = chunker.lock().await;
                if let Ok(wav_bytes) = chunker_guard.flush() {
                    if !wav_bytes.is_empty() {
                        match temp_provider.transcribe_audio(wav_bytes).await {
                            Ok(text) => {
                                if !text.trim().is_empty() {
                                    debug!("Final Groq transcription: {}", text);

                                    if !accumulated_text.is_empty() {
                                        accumulated_text.push(' ');
                                    }
                                    accumulated_text.push_str(&text);
                                }
                            }
                            Err(e) => {
                                error!("Final Groq transcription failed: {}", e);
                            }
                        }
                    }
                }
            }

            // Send final committed transcription
            if !accumulated_text.trim().is_empty() {
                let event = TranscriptionEvent::Committed {
                    text: accumulated_text,
                    timestamp_ms: last_timestamp_ms,
                };

                if let Err(e) = event_tx.send(event).await {
                    error!("Failed to send committed event: {}", e);
                }
            }

            info!("Groq processing task finished");
        });

        *self.processing_task.lock().await = Some(task);

        Ok(())
    }

    async fn send_audio(&mut self, chunk: AudioChunk) -> Result<()> {
        let tx_lock = self.audio_tx.lock().await;
        if let Some(tx) = tx_lock.as_ref() {
            tx.send(chunk)
                .await
                .map_err(|e| MurmurError::Stt(format!("Failed to send audio chunk: {}", e)))?;
            Ok(())
        } else {
            Err(MurmurError::Stt("Session not started".to_string()))
        }
    }

    async fn stop_session(&mut self) -> Result<()> {
        info!("Stopping Groq Whisper Turbo session");

        // Close audio sender channel
        *self.audio_tx.lock().await = None;

        // Wait for processing task to finish
        if let Some(task) = self.processing_task.lock().await.take() {
            let _ = task.await;
        }

        info!("Groq Whisper Turbo session stopped");
        Ok(())
    }

    async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
        let mut rx_lock = self.event_rx.lock().await;
        rx_lock
            .take()
            .expect("subscribe_events called multiple times")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_provider_creation() {
        let provider = GroqProvider::new("test-api-key".to_string());
        assert_eq!(provider.model, "whisper-large-v3-turbo");
    }
}
