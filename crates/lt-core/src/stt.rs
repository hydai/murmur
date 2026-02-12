use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Audio chunk for STT processing
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// PCM samples (16-bit mono, 16kHz)
    pub data: Vec<i16>,
    /// Timestamp (milliseconds from session start)
    pub timestamp_ms: u64,
}

/// Transcription events from STT provider
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptionEvent {
    /// Partial (interim) transcription
    Partial {
        text: String,
        timestamp_ms: u64,
    },
    /// Committed (final) transcription
    Committed {
        text: String,
        timestamp_ms: u64,
    },
    /// Error during transcription
    Error {
        message: String,
    },
}

/// Unified STT provider trait
#[async_trait]
pub trait SttProvider: Send + Sync {
    /// Start a new transcription session
    async fn start_session(&mut self) -> Result<()>;

    /// Send audio chunk for transcription
    async fn send_audio(&mut self, chunk: AudioChunk) -> Result<()>;

    /// Stop the current session
    async fn stop_session(&mut self) -> Result<()>;

    /// Subscribe to transcription events
    /// Returns a channel receiver for events
    async fn subscribe_events(&self) -> tokio::sync::mpsc::Receiver<TranscriptionEvent>;
}
