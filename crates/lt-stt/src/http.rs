use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::redact::display_origin;
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};

use crate::chunker::AudioChunker;

const DEFAULT_MODEL: &str = "whisper-1";
pub(crate) const MAX_PENDING_TRANSCRIPTION_CHUNKS: usize = 4;
#[cfg(not(test))]
const TRANSCRIPTION_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const TRANSCRIPTION_REQUEST_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
}

/// Shared bounded worker for OpenAI-compatible transcription endpoints.
pub struct HttpSttProvider {
    base_url: String,
    client: reqwest::Client,
    chunk_duration_ms: u64,
    pub(crate) api_key: Option<String>,
    pub(crate) model: String,
    pub(crate) language: Option<String>,
    chunker: Arc<Mutex<AudioChunker>>,
    audio_tx: Arc<Mutex<Option<mpsc::Sender<AudioChunk>>>>,
    event_tx: Arc<Mutex<Option<mpsc::Sender<TranscriptionEvent>>>>,
    event_rx: Arc<Mutex<Option<mpsc::Receiver<TranscriptionEvent>>>>,
    processing_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl HttpSttProvider {
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
            client: reqwest::Client::new(),
            chunk_duration_ms: 4000,
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

    pub(crate) fn with_chunk_duration(mut self, duration_ms: u64) -> Self {
        self.chunk_duration_ms = duration_ms;
        self
    }

    async fn transcribe_audio(&self, wav_bytes: Vec<u8>) -> Result<String> {
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

        let mut request = self.client.post(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.multipart(form).send().await.map_err(|e| {
            // Custom endpoints may carry tokens in the URL; keep it out of
            // pipeline errors and logs, as the LLM HTTP path does.
            MurmurError::Stt(format!("HTTP STT request failed: {}", e.without_url()))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(MurmurError::Stt(format!(
                "HTTP STT error ({}): {}",
                status, error_text
            )));
        }

        let whisper_response: WhisperResponse = response.json().await.map_err(|e| {
            MurmurError::Stt(format!("Failed to parse STT response: {}", e.without_url()))
        })?;

        Ok(whisper_response.text)
    }

    async fn transcribe_audio_with_timeout(&self, wav_bytes: Vec<u8>) -> Result<String> {
        tokio::time::timeout(
            TRANSCRIPTION_REQUEST_TIMEOUT,
            self.transcribe_audio(wav_bytes),
        )
        .await
        .map_err(|_| {
            MurmurError::Stt(format!(
                "HTTP STT request timed out after {:?}",
                TRANSCRIPTION_REQUEST_TIMEOUT
            ))
        })?
    }
}

#[async_trait]
impl SttProvider for HttpSttProvider {
    async fn start_session(&mut self) -> Result<()> {
        info!(
            "Starting HTTP STT session ({})",
            display_origin(&self.base_url)
        );

        if self.processing_task.lock().await.is_some() {
            return Err(MurmurError::Stt("STT session already started".into()));
        }
        *self.chunker.lock().await = AudioChunker::new(self.chunk_duration_ms);

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
        let client = self.client.clone();
        let ingest_event_tx = event_tx.clone();

        let task = tokio::spawn(async move {
            let mut temp_provider = HttpSttProvider::new(base_url, api_key, Some(model), language);
            temp_provider.client = client;
            let (wav_tx, mut wav_rx) =
                mpsc::channel::<(Vec<u8>, u64)>(MAX_PENDING_TRANSCRIPTION_CHUNKS);
            let transcription_task = tokio::spawn(async move {
                let mut last_timestamp_ms = 0u64;
                let mut accumulated_text = String::new();

                loop {
                    let next = tokio::select! {
                        biased;
                        _ = event_tx.closed() => return,
                        next = wav_rx.recv() => next,
                    };
                    let Some((wav_bytes, timestamp_ms)) = next else {
                        break;
                    };
                    last_timestamp_ms = timestamp_ms;

                    let transcription = tokio::select! {
                        biased;
                        _ = event_tx.closed() => return,
                        result = temp_provider.transcribe_audio_with_timeout(wav_bytes) => result,
                    };
                    match transcription {
                        Ok(text) => {
                            if !text.trim().is_empty() {
                                debug!("HTTP STT transcription result: {}", text);

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
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            error!("HTTP STT transcription failed: {}", e);
                            let event = TranscriptionEvent::Error {
                                message: format!("HTTP STT error: {}", e),
                            };
                            let _ = event_tx.send(event).await;
                            return;
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

                info!("HTTP STT transcription task finished");
            });

            let _transcription_guard = AbortTaskOnDrop(transcription_task.abort_handle());
            let mut last_timestamp_ms = 0u64;

            loop {
                let next = tokio::select! {
                    biased;
                    _ = ingest_event_tx.closed() => return,
                    next = audio_rx.recv() => next,
                };
                let Some(chunk) = next else {
                    break;
                };
                last_timestamp_ms = chunk.timestamp_ms;

                let wav_bytes = {
                    let mut chunker_guard = chunker.lock().await;
                    chunker_guard.add_chunk(&chunk);

                    if chunker_guard.should_flush(chunk.timestamp_ms) {
                        debug!("Flushing audio chunk for HTTP STT transcription");
                        Some(chunker_guard.flush())
                    } else {
                        None
                    }
                };

                match wav_bytes {
                    Some(Ok(wav_bytes)) if wav_bytes.is_empty() => {
                        debug!("Empty WAV bytes, skipping transcription");
                    }
                    Some(Ok(wav_bytes)) => match wav_tx.try_send((wav_bytes, chunk.timestamp_ms)) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            let message =
                                "STT endpoint cannot keep up: transcription backlog is full";
                            let _ = ingest_event_tx
                                .send(TranscriptionEvent::Error {
                                    message: message.into(),
                                })
                                .await;
                            break;
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => {
                            error!("HTTP STT transcription task stopped unexpectedly");
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
                    // Capture has ended, so backpressure here cannot interrupt
                    // live audio. Preserve the final buffer while the bounded
                    // worker queue drains (each request has a deadline).
                    let _ = wav_tx.send((wav_bytes, last_timestamp_ms)).await;
                }
                Err(e) => {
                    error!("Failed to flush final audio buffer: {}", e);
                }
            }

            drop(wav_tx);

            if let Err(e) = transcription_task.await {
                error!("HTTP STT transcription task join failed: {}", e);
            }

            info!("HTTP STT processing task finished");
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
        info!("Stopping HTTP STT session");

        *self.audio_tx.lock().await = None;

        if let Some(task) = self.processing_task.lock().await.take() {
            let _guard = AbortTaskOnDrop(task.abort_handle());
            let _ = task.await;
        }
        self.event_tx.lock().await.take();

        info!("HTTP STT session stopped");
        Ok(())
    }

    async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
        let mut rx_lock = self.event_rx.lock().await;
        rx_lock
            .take()
            .expect("subscribe_events called multiple times")
    }
}

struct AbortTaskOnDrop(tokio::task::AbortHandle);
impl Drop for AbortTaskOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Drop for HttpSttProvider {
    fn drop(&mut self) {
        if let Ok(mut task) = self.processing_task.try_lock() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }
}

macro_rules! delegate_http_provider {
    ($provider:ty) => {
        #[async_trait::async_trait]
        impl lt_core::stt::SttProvider for $provider {
            async fn start_session(&mut self) -> lt_core::error::Result<()> {
                self.inner.start_session().await
            }
            async fn send_audio(
                &mut self,
                chunk: lt_core::stt::AudioChunk,
            ) -> lt_core::error::Result<()> {
                self.inner.send_audio(chunk).await
            }
            async fn stop_session(&mut self) -> lt_core::error::Result<()> {
                self.inner.stop_session().await
            }
            async fn subscribe_events(
                &self,
            ) -> tokio::sync::mpsc::Receiver<lt_core::stt::TranscriptionEvent> {
                self.inner.subscribe_events().await
            }
        }
    };
}
pub(crate) use delegate_http_provider;

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    const SECRET: &str = "top-secret-token";

    /// Custom endpoints behind secret-URL tunnels or proxies carry the token
    /// in the path; reqwest's error Display prints the full request URL.
    fn provider_at(port: u16) -> HttpSttProvider {
        HttpSttProvider::new(
            format!("http://127.0.0.1:{port}/{SECRET}/v1"),
            None,
            None,
            None,
        )
    }

    async fn read_http_request(stream: &mut TcpStream) {
        let mut received = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let read = stream.read(&mut chunk).await.unwrap();
            if read == 0 {
                return;
            }
            received.extend_from_slice(&chunk[..read]);
            if let Some(end) = received.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if received.len() >= end + 4 + content_length {
                    return;
                }
            }
        }
    }

    async fn serve_once(body: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_http_request(&mut stream).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        port
    }

    #[tokio::test]
    async fn transport_errors_do_not_expose_endpoint_credentials() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let error = provider_at(port)
            .transcribe_audio(vec![0; 64])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("HTTP STT request failed"), "{error}");
        assert!(!error.contains(SECRET), "credentials leaked: {error}");
    }

    #[tokio::test]
    async fn malformed_responses_do_not_expose_endpoint_credentials() {
        let port = serve_once("not json").await;

        let error = provider_at(port)
            .transcribe_audio(vec![0; 64])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("Failed to parse STT response"), "{error}");
        assert!(!error.contains(SECRET), "credentials leaked: {error}");
    }
}
