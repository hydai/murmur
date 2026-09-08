use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures_util::{SinkExt, StreamExt};
use lt_core::error::{MurmurError, Result};
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, tungstenite::Message,
};
use tracing::{debug, error, info, warn};
use url::Url;

/// ElevenLabs WebSocket message types
#[derive(Debug, Serialize)]
struct ElevenLabsMessage {
    message_type: String,
    audio_base_64: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<bool>,
}

/// ElevenLabs WebSocket response types
#[derive(Debug, Deserialize)]
#[serde(tag = "message_type")]
enum ElevenLabsResponse {
    #[serde(rename = "session_started")]
    SessionStarted {},

    #[serde(rename = "partial_transcript")]
    PartialTranscript {
        #[serde(default)]
        text: String,
    },

    #[serde(rename = "committed_transcript")]
    CommittedTranscript {
        #[serde(default)]
        text: String,
    },

    #[serde(rename = "error")]
    Error {
        #[serde(default)]
        error: String,
    },

    #[serde(rename = "invalid_request")]
    InvalidRequest {
        #[serde(default)]
        error: String,
    },
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
    #[cfg(test)]
    test_url: Option<Url>,
}

impl ElevenLabsProvider {
    /// Create a new ElevenLabs provider
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model_id: "scribe_v2_realtime".to_string(),
            language_code: "en".to_string(),
            ws_tx: Arc::new(Mutex::new(None)),
            event_tx: Arc::new(Mutex::new(None)),
            event_rx: Arc::new(Mutex::new(None)),
            ws_task: Arc::new(Mutex::new(None)),
            reconnect_config: ReconnectConfig::default(),
            should_reconnect: Arc::new(Mutex::new(true)),
            #[cfg(test)]
            test_url: None,
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
            #[cfg(test)]
            test_url: None,
        }
    }

    /// Build WebSocket URL
    fn build_ws_url(&self) -> Result<Url> {
        #[cfg(test)]
        if let Some(url) = &self.test_url {
            return Ok(url.clone());
        }
        let url = if self.language_code == "auto" {
            format!(
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id={}&audio_format=pcm_16000",
                self.model_id
            )
        } else {
            format!(
                "wss://api.elevenlabs.io/v1/speech-to-text/realtime?model_id={}&language_code={}&audio_format=pcm_16000",
                self.model_id, self.language_code
            )
        };
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
            let mut request = ws_url
                .as_str()
                .into_client_request()
                .map_err(|e| MurmurError::Stt(format!("Failed to build request: {}", e)))?;
            request.headers_mut().insert(
                "xi-api-key",
                self.api_key
                    .parse()
                    .map_err(|_| MurmurError::Stt("Invalid API key header value".to_string()))?,
            );

            match tokio::time::timeout(Duration::from_secs(10), connect_async(request)).await {
                Ok(Ok((ws_stream, _))) => {
                    info!("WebSocket connected to ElevenLabs");
                    return Ok(ws_stream);
                }
                failure => {
                    let e = match failure {
                        Ok(Err(e)) => e.to_string(),
                        Err(_) => "WebSocket connection timed out".to_string(),
                        Ok(Ok(_)) => unreachable!(),
                    };
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
            let mut receiver_task = tokio::spawn(async move {
                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            // Payloads carry transcripts; log only their size.
                            debug!(bytes = text.len(), "Received message");

                            match serde_json::from_str::<ElevenLabsResponse>(&text) {
                                Ok(response) => match response {
                                    ElevenLabsResponse::SessionStarted {} => {
                                        info!("ElevenLabs session started");
                                    }
                                    ElevenLabsResponse::PartialTranscript { text } => {
                                        if !text.is_empty() {
                                            let event = TranscriptionEvent::Partial {
                                                text,
                                                timestamp_ms: 0,
                                            };
                                            if let Err(e) = event_tx_clone.send(event).await {
                                                error!("Failed to send partial event: {}", e);
                                            }
                                        }
                                    }
                                    ElevenLabsResponse::CommittedTranscript { text } => {
                                        if !text.is_empty() {
                                            let event = TranscriptionEvent::Committed {
                                                text,
                                                timestamp_ms: 0,
                                            };
                                            if let Err(e) = event_tx_clone.send(event).await {
                                                error!("Failed to send committed event: {}", e);
                                            }
                                        }
                                    }
                                    ElevenLabsResponse::Error { error }
                                    | ElevenLabsResponse::InvalidRequest { error } => {
                                        error!("ElevenLabs error: {}", error);
                                        let event = TranscriptionEvent::Error { message: error };
                                        if let Err(e) = event_tx_clone.send(event).await {
                                            error!("Failed to send error event: {}", e);
                                        }
                                    }
                                },
                                Err(e) => {
                                    warn!(bytes = text.len(), "Failed to parse message: {}", e);
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

            // Cancelling the outer task must cancel its child as well.
            let _receiver_guard = AbortTaskOnDrop(receiver_task.abort_handle());

            let upload = async {
                // Send audio chunks
                while let Some(chunk) = audio_rx.recv().await {
                    // Convert i16 PCM to raw bytes
                    let pcm_bytes: Vec<u8> =
                        chunk.data.iter().flat_map(|s| s.to_le_bytes()).collect();

                    let audio_base_64 = BASE64.encode(&pcm_bytes);

                    // Create JSON message
                    let msg = ElevenLabsMessage {
                        message_type: "input_audio_chunk".to_string(),
                        audio_base_64,
                        sample_rate: Some(16000),
                        commit: None,
                    };
                    let json = serde_json::to_string(&msg).unwrap();

                    // Send to WebSocket
                    match tokio::time::timeout(
                        Duration::from_secs(10),
                        ws_write.send(Message::Text(json.into())),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        result => {
                            error!("Failed to send audio chunk: {:?}", result);
                            let _ = event_tx
                                .send(TranscriptionEvent::Error {
                                    message: "ElevenLabs audio upload failed or timed out"
                                        .to_string(),
                                })
                                .await;
                            break;
                        }
                    }
                }

                debug!("Audio sender finished, sending commit signal");

                // Send a final commit message to flush the server's transcription buffer.
                // With vad_commit_strategy disabled, the server won't auto-commit;
                // we must explicitly request it.
                let commit_msg = ElevenLabsMessage {
                    message_type: "input_audio_chunk".to_string(),
                    audio_base_64: String::new(),
                    sample_rate: Some(16000),
                    commit: Some(true),
                };
                let json = serde_json::to_string(&commit_msg).unwrap();
                if !matches!(
                    tokio::time::timeout(
                        Duration::from_secs(10),
                        ws_write.send(Message::Text(json.into()))
                    )
                    .await,
                    Ok(Ok(()))
                ) {
                    warn!("Failed to send commit message before deadline");
                }

                // Give the server time to process the commit and send back
                // a committed_transcript before we tear down the connection.
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                debug!("Closing WebSocket");
                let _ = tokio::time::timeout(Duration::from_secs(2), ws_write.close()).await;
            };
            let cancelled = tokio::select! {
                biased;
                _ = event_tx.closed() => true,
                _ = upload => false,
            };
            if cancelled {
                // Terminal pipeline failure cancels buffered and in-flight
                // uploads instead of treating them as a graceful drain.
                receiver_task.abort();
                let _ = receiver_task.await;
            } else {
                finish_receiver(&mut receiver_task, Duration::from_secs(2)).await;
            }

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
            let _guard = AbortTaskOnDrop(task.abort_handle());
            let _ = task.await;
        }

        self.event_tx.lock().await.take();
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

struct AbortTaskOnDrop(tokio::task::AbortHandle);

impl Drop for AbortTaskOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn finish_receiver(task: &mut tokio::task::JoinHandle<()>, deadline: Duration) {
    if tokio::time::timeout(deadline, &mut *task).await.is_err() {
        warn!("Receiver task timed out during shutdown");
        task.abort();
        let _ = task.await;
    }
}

impl Drop for ElevenLabsProvider {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.ws_task.try_lock() {
            if let Some(task) = guard.take() {
                task.abort();
            }
        }
    }
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;

    #[tokio::test]
    async fn timeout_joins_receiver_and_closes_its_event_channel() {
        let (tx, mut rx) = mpsc::channel::<TranscriptionEvent>(1);
        let mut task = tokio::spawn(async move {
            let _keep_sender_alive = tx;
            std::future::pending::<()>().await;
        });
        finish_receiver(&mut task, Duration::from_millis(10)).await;
        assert!(task.is_finished());
        assert!(rx.recv().await.is_none());
    }
    use crate::test_support::{captured_logs, logs_text};

    #[tokio::test]
    async fn websocket_payloads_never_reach_the_logs() {
        let logs = captured_logs();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            websocket
                .send(Message::Text(
                    r#"{"unexpected":"garbage-zebra-quartz"}"#.into(),
                ))
                .await
                .unwrap();
            websocket
                .send(Message::Text(
                    r#"{"message_type":"committed_transcript","text":"spoken-zebra-quartz"}"#
                        .into(),
                ))
                .await
                .unwrap();
            // Keep the socket open until the client closes it.
            while let Some(Ok(message)) = websocket.next().await {
                if message.is_close() {
                    break;
                }
            }
        });
        let mut provider = ElevenLabsProvider::new("test".into());
        provider.test_url = Some(format!("ws://{address}").parse().unwrap());
        provider.start_session().await.unwrap();
        let mut events = provider.subscribe_events().await;
        let committed = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match events.recv().await {
                    Some(TranscriptionEvent::Committed { text, .. }) => break text,
                    Some(_) => continue,
                    None => panic!("event channel closed before the transcript arrived"),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(committed, "spoken-zebra-quartz");
        drop(events);
        tokio::time::timeout(Duration::from_secs(2), provider.stop_session())
            .await
            .unwrap()
            .unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(2), server).await;

        let captured = logs_text(&logs);
        assert!(
            captured.contains("Failed to parse message"),
            "logs were not captured:\n{captured}"
        );
        assert!(
            !captured.contains("zebra-quartz"),
            "payload leaked into logs:\n{captured}"
        );
    }

    #[tokio::test]
    async fn closing_consumer_discards_buffered_audio_and_joins_websocket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut audio_messages = 0;
            while let Some(Ok(message)) = websocket.next().await {
                if message.is_text() {
                    audio_messages += 1;
                }
            }
            audio_messages
        });
        let mut provider = ElevenLabsProvider::new("test".into());
        provider.test_url = Some(format!("ws://{address}").parse().unwrap());
        provider.start_session().await.unwrap();
        let events = provider.subscribe_events().await;
        let audio_tx = provider.ws_tx.lock().await.as_ref().unwrap().clone();
        for timestamp in 0..8 {
            audio_tx
                .try_send(AudioChunk {
                    data: vec![0; 160],
                    timestamp_ms: timestamp,
                })
                .unwrap();
        }
        drop(audio_tx);
        // This current-thread test queues audio without yielding to the worker.
        // Cancellation must win over every buffered upload and the final commit.
        drop(events);
        tokio::time::timeout(Duration::from_millis(200), provider.stop_session())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), server)
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }
}
