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
            let temp_provider = CustomSttProvider::new(base_url, api_key, Some(model), language);
            let (wav_tx, mut wav_rx) = mpsc::unbounded_channel::<(Vec<u8>, u64)>();
            let transcription_task = tokio::spawn(async move {
                let mut last_timestamp_ms = 0u64;
                let mut accumulated_text = String::new();

                while let Some((wav_bytes, timestamp_ms)) = wav_rx.recv().await {
                    last_timestamp_ms = timestamp_ms;

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
                                    timestamp_ms,
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

                if !accumulated_text.trim().is_empty() {
                    let event = TranscriptionEvent::Committed {
                        text: accumulated_text,
                        timestamp_ms: last_timestamp_ms,
                    };

                    if let Err(e) = event_tx.send(event).await {
                        error!("Failed to send committed event: {}", e);
                    }
                }

                info!("Custom STT transcription task finished");
            });

            let mut last_timestamp_ms = 0u64;

            while let Some(chunk) = audio_rx.recv().await {
                last_timestamp_ms = chunk.timestamp_ms;

                let wav_bytes = {
                    let mut chunker_guard = chunker.lock().await;
                    chunker_guard.add_chunk(&chunk);

                    if chunker_guard.should_flush(chunk.timestamp_ms) {
                        debug!("Flushing audio chunk for Custom STT transcription");
                        Some(chunker_guard.flush())
                    } else {
                        None
                    }
                };

                match wav_bytes {
                    Some(Ok(wav_bytes)) if wav_bytes.is_empty() => {
                        debug!("Empty WAV bytes, skipping transcription");
                    }
                    Some(Ok(wav_bytes)) => match wav_tx.send((wav_bytes, chunk.timestamp_ms)) {
                        Ok(()) => {}
                        Err(_) => {
                            error!("Custom STT transcription task stopped unexpectedly");
                            break;
                        }
                    },
                    Some(Err(e)) => {
                        error!("Failed to flush audio buffer: {}", e);
                    }
                    None => {}
                }
            }

            debug!("Audio stream ended, flushing remaining audio");
            let final_wav_bytes = {
                let mut chunker_guard = chunker.lock().await;
                chunker_guard.flush()
            };

            match final_wav_bytes {
                Ok(wav_bytes) if wav_bytes.is_empty() => {
                    debug!("Empty final WAV bytes, skipping transcription");
                }
                Ok(wav_bytes) => {
                    if wav_tx.send((wav_bytes, last_timestamp_ms)).is_err() {
                        error!("Custom STT transcription task stopped before final audio");
                    }
                }
                Err(e) => {
                    error!("Failed to flush final audio buffer: {}", e);
                }
            }

            drop(wav_tx);

            if let Err(e) = transcription_task.await {
                error!("Custom STT transcription task join failed: {}", e);
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;
    use tokio::time::{timeout, Duration};

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

    #[tokio::test]
    async fn send_audio_does_not_block_while_transcription_request_is_in_flight() {
        let server = HangingTranscriptionServer::start().await;
        let mut provider = CustomSttProvider::new(server.base_url(), None, None, None);

        provider.start_session().await.unwrap();
        let _events = provider.subscribe_events().await;

        provider.send_audio(test_chunk(1)).await.unwrap();
        provider.send_audio(test_chunk(4001)).await.unwrap();
        server.wait_for_request().await;

        let mut blocked_at = None;
        for i in 0..33 {
            let send = provider.send_audio(test_chunk(4010 + i));
            match timeout(Duration::from_millis(100), send).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => panic!("send_audio returned error: {}", e),
                Err(_) => {
                    blocked_at = Some(i);
                    break;
                }
            }
        }

        server.release_response();
        let _ = provider.stop_session().await;

        assert!(
            blocked_at.is_none(),
            "send_audio blocked at queued chunk {:?} while a transcription request was in flight",
            blocked_at
        );
    }

    fn test_chunk(timestamp_ms: u64) -> AudioChunk {
        AudioChunk {
            data: vec![0; 160],
            timestamp_ms,
        }
    }

    struct HangingTranscriptionServer {
        base_url: String,
        request_seen_rx: Mutex<Option<oneshot::Receiver<()>>>,
        release_tx: Mutex<Option<oneshot::Sender<()>>>,
    }

    impl HangingTranscriptionServer {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (request_seen_tx, request_seen_rx) = oneshot::channel();
            let (release_tx, release_rx) = oneshot::channel();

            tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                read_http_request(&mut stream).await;
                let _ = request_seen_tx.send(());
                let _ = release_rx.await;

                let body = r#"{"text":"ok"}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });

            Self {
                base_url: format!("http://{}/v1", addr),
                request_seen_rx: Mutex::new(Some(request_seen_rx)),
                release_tx: Mutex::new(Some(release_tx)),
            }
        }

        fn base_url(&self) -> String {
            self.base_url.clone()
        }

        async fn wait_for_request(&self) {
            let rx = self.request_seen_rx.lock().await.take().unwrap();
            rx.await.unwrap();
        }

        fn release_response(&self) {
            if let Ok(mut guard) = self.release_tx.try_lock() {
                if let Some(tx) = guard.take() {
                    let _ = tx.send(());
                }
            }
        }
    }

    async fn read_http_request(stream: &mut tokio::net::TcpStream) {
        let mut buf = Vec::new();
        let mut header_end = None;
        let mut tmp = [0u8; 4096];

        while header_end.is_none() {
            let n = stream.read(&mut tmp).await.unwrap();
            assert!(n > 0, "connection closed before HTTP headers");
            buf.extend_from_slice(&tmp[..n]);
            header_end = buf.windows(4).position(|window| window == b"\r\n\r\n");
        }

        let header_end = header_end.unwrap() + 4;
        let headers = String::from_utf8_lossy(&buf[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .or_else(|| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length:"))
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);

        let mut body_read = buf.len().saturating_sub(header_end);
        while body_read < content_length {
            let n = stream.read(&mut tmp).await.unwrap();
            assert!(n > 0, "connection closed before HTTP body");
            body_read += n;
        }
    }
}
