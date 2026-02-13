use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use url::Url;

/// ElevenLabs WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ElevenLabsMessage {
    #[serde(rename = "audio")]
    Audio { audio_base64: String },
}

/// ElevenLabs WebSocket response types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ElevenLabsResponse {
    #[serde(rename = "partial_transcript")]
    PartialTranscript {
        text: String,
        #[serde(default)]
        timestamp: Option<u64>,
    },
    #[serde(rename = "final_transcript")]
    FinalTranscript {
        text: String,
        #[serde(default)]
        timestamp: Option<u64>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Reconnection configuration
#[derive(Clone)]
struct ReconnectConfig {
    max_retries: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_retries: 10,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

/// ElevenLabs Scribe v2 WebSocket client
pub struct ElevenLabsProvider {
    api_key: String,
    model_id: String,
    language_code: String,
    ws_tx: Arc<Mutex<Option<mpsc::Sender<AudioChunk>>>>,
    event_tx: Arc<Mutex<Option<mpsc::Sender<TranscriptionEvent>>>>,
    event_rx: Arc<Mutex<Option<mpsc::Receiver<TranscriptionEvent>>>>,
    ws_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    reconnect_config: ReconnectConfig,
    should_reconnect: Arc<Mutex<bool>>,
}

impl ElevenLabsProvider {
    /// Create a new ElevenLabs provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model_id: "scribe_v2".to_string(),
            language_code: "en".to_string(),
            ws_tx: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            event_rx: Arc::new(Mutex::new(None)),
            ws_task: Arc::new(Mutex::new(None)),
            reconnect_config: ReconnectConfig::default(),
            should_reconnect: Arc::new(Mutex::new(true)),
        }
    }

    /// Create provider with custom model and language
    pub fn with_config(api_key: String, model_id: String, language_code: String) -> Self {
        Self {
            api_key,
            model_id,
            language_code,
            ws_tx: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            event_rx: Arc::new(Mutex::new(None)),
            ws_task: Arc::new(Mutex::new(None)),
            reconnect_config: ReconnectConfig::default(),
            should_reconnect: Arc::new(Mutex::new(true)),
        }
    }

    /// Build WebSocket URL
    fn build_ws_url(&self) -> Result<Url> {
        let url = format!(
            "wss://api.elevenlabs.io/v1/speech-to-text/ws?model_id={}&language_code={}",
            self.model_id, self.language_code
        );
        Url::parse(&url).map_err(|e| MurmurError::Stt(format!("Invalid URL: {}", e)))
    }

    /// Connect to WebSocket with retry logic
    async fn connect_with_retry(
        &self,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        let ws_url = self.build_ws_url()?;
        let mut retry_count = 0;

        loop {
            let request = http::Request::builder()
                .uri(ws_url.as_str())
                .header("xi-api-key", &self.api_key)
                .body(())
                .map_err(|e| MurmurError::Stt(format!("Failed to build request: {}", e)))?;

            match connect_async(request).await {
                Ok((ws_stream, _)) => {
                    info!("WebSocket connected to ElevenLabs");
                    return Ok(ws_stream);
                }
                Err(e) => {
                    if retry_count >= self.reconnect_config.max_retries {
                        error!("Failed to connect after {} retries", retry_count);
                        return Err(MurmurError::Stt(format!(
                            "WebSocket connection failed after {} retries: {}",
                            retry_count, e
                        )));
                    }

                    let delay = std::cmp::min(
                        self.reconnect_config.base_delay_ms * 2u64.pow(retry_count),
                        self.reconnect_config.max_delay_ms,
                    );

                    warn!(
                        "WebSocket connection failed (attempt {}/{}), retrying in {}ms: {}",
                        retry_count + 1,
                        self.reconnect_config.max_retries,
                        delay,
                        e
                    );

                    tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    retry_count += 1;
                }
            }
        }
    }
}

#[async_trait]
impl SttProvider for ElevenLabsProvider {
    async fn start_session(&mut self) -> Result<()> {
        info!("Starting ElevenLabs STT session");

        // Enable reconnection
        *self.should_reconnect.lock().await = true;

        // Create channel for audio chunks
        let (audio_tx, mut audio_rx) = mpsc::channel::<AudioChunk>(32);
        *self.ws_tx.lock().await = Some(audio_tx);

        // Create channel for transcription events
        let (event_tx, event_rx) = mpsc::channel::<TranscriptionEvent>(32);
        *self.event_tx.lock().await = Some(event_tx.clone());
        *self.event_rx.lock().await = Some(event_rx);

        // Connect to WebSocket with retry
        let ws_stream = self.connect_with_retry().await?;

        let (mut ws_write, mut ws_read) = ws_stream.split();

        // Spawn task to send audio and receive transcription
        let task = tokio::spawn(async move {
            // Spawn receiver task
            let event_tx_clone = event_tx.clone();
            let receiver_task = tokio::spawn(async move {
                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            debug!("Received message: {}", text);

                            match serde_json::from_str::<ElevenLabsResponse>(&text) {
                                Ok(response) => match response {
                                    ElevenLabsResponse::PartialTranscript { text, timestamp } => {
                                        if !text.is_empty() {
                                            let event = TranscriptionEvent::Partial {
                                                text,
                                                timestamp_ms: timestamp.unwrap_or(0),
                                            };
                                            if let Err(e) = event_tx_clone.send(event).await {
                                                error!("Failed to send partial event: {}", e);
                                            }
                                        }
                                    }
                                    ElevenLabsResponse::FinalTranscript { text, timestamp } => {
                                        if !text.is_empty() {
                                            let event = TranscriptionEvent::Committed {
                                                text,
                                                timestamp_ms: timestamp.unwrap_or(0),
                                            };
                                            if let Err(e) = event_tx_clone.send(event).await {
                                                error!("Failed to send committed event: {}", e);
                                            }
                                        }
                                    }
                                    ElevenLabsResponse::Error { message } => {
                                        error!("ElevenLabs error: {}", message);
                                        let event = TranscriptionEvent::Error { message };
                                        if let Err(e) = event_tx_clone.send(event).await {
                                            error!("Failed to send error event: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    warn!("Failed to parse message: {} - {}", e, text);
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            info!("WebSocket closed by server");
                            break;
                        }
                        Ok(_) => {
                            debug!("Received non-text message");
                        }
                        Err(e) => {
                            error!("WebSocket error: {}", e);
                            let event = TranscriptionEvent::Error {
                                message: format!("WebSocket error: {}", e),
                            };
                            let _ = event_tx_clone.send(event).await;
                            break;
                        }
                    }
                }
                debug!("WebSocket receiver task finished");
            });

            // Send audio chunks
            while let Some(chunk) = audio_rx.recv().await {
                // Convert i16 PCM to WAV
                let wav_bytes = {
                    let sample_rate = 16000u32;
                    let num_channels = 1u16;
                    let bits_per_sample = 16u16;
                    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
                    let block_align = num_channels * bits_per_sample / 8;
                    let data_size = (chunk.data.len() * 2) as u32;
                    let file_size = 36 + data_size;

                    let mut wav = Vec::with_capacity((44 + data_size) as usize);

                    wav.extend_from_slice(b"RIFF");
                    wav.extend_from_slice(&file_size.to_le_bytes());
                    wav.extend_from_slice(b"WAVE");
                    wav.extend_from_slice(b"fmt ");
                    wav.extend_from_slice(&16u32.to_le_bytes());
                    wav.extend_from_slice(&1u16.to_le_bytes());
                    wav.extend_from_slice(&num_channels.to_le_bytes());
                    wav.extend_from_slice(&sample_rate.to_le_bytes());
                    wav.extend_from_slice(&byte_rate.to_le_bytes());
                    wav.extend_from_slice(&block_align.to_le_bytes());
                    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
                    wav.extend_from_slice(b"data");
                    wav.extend_from_slice(&data_size.to_le_bytes());

                    for sample in &chunk.data {
                        wav.extend_from_slice(&sample.to_le_bytes());
                    }

                    wav
                };

                // Encode as base64
                let audio_base64 = BASE64.encode(&wav_bytes);

                // Create JSON message
                let msg = ElevenLabsMessage::Audio { audio_base64 };
                let json = serde_json::to_string(&msg).unwrap();

                // Send to WebSocket
                if let Err(e) = ws_write.send(Message::Text(json.into())).await {
                    error!("Failed to send audio chunk: {}", e);
                    break;
                }
            }

            debug!("Audio sender finished, closing WebSocket");

            // Close WebSocket
            let _ = ws_write.close().await;

            // Wait for receiver to finish
            let _ = receiver_task.await;

            info!("WebSocket task finished");
        });

        *self.ws_task.lock().await = Some(task);

        Ok(())
    }

    async fn send_audio(&mut self, chunk: AudioChunk) -> Result<()> {
        let tx_lock = self.ws_tx.lock().await;
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
        info!("Stopping ElevenLabs STT session");

        // Disable reconnection
        *self.should_reconnect.lock().await = false;

        // Close audio sender channel
        *self.ws_tx.lock().await = None;

        // Wait for WebSocket task to finish
        if let Some(task) = self.ws_task.lock().await.take() {
            let _ = task.await;
        }

        info!("ElevenLabs STT session stopped");
        Ok(())
    }

    async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
        let mut rx_lock = self.event_rx.lock().await;
        rx_lock
            .take()
            .expect("subscribe_events called multiple times")
    }
}
