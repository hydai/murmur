use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

use crate::chunker::AudioChunker;

pub const DEFAULT_MODEL: &str = "whisper-1";

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
}

/// Custom OpenAI-compatible STT endpoint (whisper.cpp, faster-whisper, LocalAI, etc.)
pub struct CustomSttProvider {
    base_url: String,
    api_key: Option<String>,
    model: String,
    language: Option<String>,
    chunker: Arc<Mutex<AudioChunker>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioChunk>>>>,
    event_tx: Arc<Mutex<Option<mpsc::Sender<TranscriptionEvent>>>>,
    event_rx: Arc<Mutex<Option<mpsc::Receiver<TranscriptionEvent>>>>,
    processing_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl CustomSttProvider {
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        model: Option<String>,
        language: Option<String>,
    ) -> Self {
        let api_key = api_key.filter(|k| !k.is_empty());
        let language = language.filter(|l| !l.is_empty());
        Self {
            base_url,
            api_key,
            model: model
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            language,
            chunker: Arc::new(Mutex::new(AudioChunker::new(4000))),
            audio_tx: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            event_rx: Arc::new(Mutex::new(None)),
            processing_task: Arc::new(Mutex::new(None)),
        }
    }

    async fn transcribe_audio(&self, wav_bytes: Vec<u8>) -> Result<String> {
        let client = reqwest::Client::new();

        let part = Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| MurmurError::Stt(format!("Failed to create multipart part: {}", e)))?;

        let mut form = Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        if let Some(ref lang) = self.language {
            form = form.text("language", lang.clone());
        }

        let url = format!(
            "{}/audio/transcriptions",
            self.base_url.trim_end_matches('/')
        );

        let mut request = client.post(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request
            .multipart(form)
            .send()
            .await
            .map_err(|e| MurmurError::Stt(format!("Custom STT request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(MurmurError::Stt(format!(
                "Custom STT error ({}): {}",
                status, error_text
            )));
        }

        let whisper_response: WhisperResponse = response
            .json()
            .await
            .map_err(|e| MurmurError::Stt(format!("Failed to parse STT response: {}", e)))?;

        Ok(whisper_response.text)
    }
}

#[async_trait]
impl SttProvider for CustomSttProvider {
    async fn start_session(&mut self) -> Result<()> {
        info!("Starting Custom STT session ({})", self.base_url);

        *self.chunker.lock().await = AudioChunker::new(4000);

        let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(32);
        *self.audio_tx.lock().await = Some(audio_tx);

        let (event_tx, event_rx) = mpsc::channel::<TranscriptionEvent>(32);
        *self.event_tx.lock().await = Some(event_tx.clone());
        *self.event_rx.lock().await = Some(event_rx);

        let chunker = self.chunker.clone();
        let base_url = self.base_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let language = self.language.clone();

        let task = tokio::spawn(async move {
            let mut last_timestamp_ms = 0u64;
            let mut accumulated_text = String::new();

            let temp_provider = CustomSttProvider::new(base_url, api_key, Some(model), language);

            while let Some(chunk) = audio_rx.recv().await {
                last_timestamp_ms = chunk.timestamp_ms;

                {
                    let mut chunker_guard = chunker.lock().await;
                    chunker_guard.add_chunk(&chunk);

                    if chunker_guard.should_flush(chunk.timestamp_ms) {
                        debug!("Flushing audio chunk for Custom STT transcription");

                        match chunker_guard.flush() {
                            Ok(wav_bytes) if !wav_bytes.is_empty() => {
                                match temp_provider.transcribe_audio(wav_bytes).await {
                                    Ok(text) => {
                                        if !text.trim().is_empty() {
                                            debug!("Custom STT transcription result: {}", text);

                                            if !accumulated_text.is_empty() {
                                                accumulated_text.push(' ');
                                            }
                                            accumulated_text.push_str(&text);

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
                                        error!("Custom STT transcription failed: {}", e);
                                        let event = TranscriptionEvent::Error {
                                            message: format!("Custom STT error: {}", e),
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

            debug!("Audio stream ended, flushing remaining audio");
            {
                let mut chunker_guard = chunker.lock().await;
                if let Ok(wav_bytes) = chunker_guard.flush() {
                    if !wav_bytes.is_empty() {
                        match temp_provider.transcribe_audio(wav_bytes).await {
                            Ok(text) => {
                                if !text.trim().is_empty() {
                                    debug!("Final Custom STT transcription: {}", text);

                                    if !accumulated_text.is_empty() {
                                        accumulated_text.push(' ');
                                    }
                                    accumulated_text.push_str(&text);
                                }
                            }
                            Err(e) => {
                                error!("Final Custom STT transcription failed: {}", e);
                            }
                        }
                    }
                }
            }

            if !accumulated_text.trim().is_empty() {
                let event = TranscriptionEvent::Committed {
                    text: accumulated_text,
                    timestamp_ms: last_timestamp_ms,
                };

                if let Err(e) = event_tx.send(event).await {
                    error!("Failed to send committed event: {}", e);
                }
            }

            info!("Custom STT processing task finished");
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
        info!("Stopping Custom STT session");

        *self.audio_tx.lock().await = None;

        if let Some(task) = self.processing_task.lock().await.take() {
            let _ = task.await;
        }

        info!("Custom STT session stopped");
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
    fn test_custom_provider_creation() {
        let provider =
            CustomSttProvider::new("http://localhost:8080/v1".to_string(), None, None, None);
        assert_eq!(provider.model, "whisper-1");
        assert!(provider.api_key.is_none());
        assert!(provider.language.is_none());
    }

    #[test]
    fn test_custom_provider_with_options() {
        let provider = CustomSttProvider::new(
            "http://localhost:8080/v1".to_string(),
            Some("my-key".to_string()),
            Some("large-v3".to_string()),
            Some("en".to_string()),
        );
        assert_eq!(provider.model, "large-v3");
        assert_eq!(provider.api_key.as_deref(), Some("my-key"));
        assert_eq!(provider.language.as_deref(), Some("en"));
    }

    #[test]
    fn test_empty_strings_become_none() {
        let provider = CustomSttProvider::new(
            "http://localhost:8080/v1".to_string(),
            Some("".to_string()),
            Some("".to_string()),
            Some("".to_string()),
        );
        assert_eq!(provider.model, "whisper-1");
        assert!(provider.api_key.is_none());
        assert!(provider.language.is_none());
    }
}
