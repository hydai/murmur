use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// LLM processing task types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProcessingTask {
    /// Post-process transcription (remove filler words, fix grammar, format)
    PostProcess {
        text: String,
        dictionary_terms: Vec<String>,
    },
    /// Shorten text
    Shorten { text: String },
    /// Change tone
    ChangeTone { text: String, target_tone: String },
    /// Generate reply
    GenerateReply { context: String },
    /// Translate
    Translate {
        text: String,
        target_language: String,
    },
}

/// LLM processing output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingOutput {
    /// Processed text
    pub text: String,
    /// Processing time in milliseconds
    pub processing_time_ms: u64,
    /// Metadata (optional)
    pub metadata: Option<serde_json::Value>,
}

/// LLM processor trait (via local CLI)
#[async_trait]
pub trait LlmProcessor: Send + Sync {
    /// Process a task
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput>;

    /// Health check (verify CLI is installed and working)
    async fn health_check(&self) -> Result<bool>;
}
