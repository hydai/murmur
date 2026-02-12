use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Output mode configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// Copy to clipboard only
    Clipboard,
    /// Simulate keyboard input only
    Keyboard,
    /// Both clipboard and keyboard
    Both,
}

impl Default for OutputMode {
    fn default() -> Self {
        Self::Clipboard
    }
}

/// Output sink trait
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Output text to the configured destination
    async fn output_text(&self, text: &str) -> Result<()>;
}
