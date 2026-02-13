use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Output mode configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// Copy to clipboard only
    #[default]
    Clipboard,
    /// Simulate keyboard input only
    Keyboard,
    /// Both clipboard and keyboard
    Both,
}

/// Output sink trait
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Output text to the configured destination
    async fn output_text(&self, text: &str) -> Result<()>;
}
