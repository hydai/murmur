use async_trait::async_trait;
use lt_core::error::Result;
use lt_core::output::{OutputMode, OutputSink};

use crate::clipboard::ClipboardOutput;
use crate::keyboard::KeyboardOutput;

/// Combined output sink that routes to clipboard, keyboard, or both
pub struct CombinedOutput {
    mode: OutputMode,
    clipboard: Option<ClipboardOutput>,
    keyboard: Option<KeyboardOutput>,
}

impl CombinedOutput {
    /// Create a new combined output sink with the specified mode
    pub fn new(mode: OutputMode) -> Result<Self> {
        let clipboard = match mode {
            OutputMode::Clipboard | OutputMode::Both => Some(ClipboardOutput::new()?),
            OutputMode::Keyboard => None,
        };

        let keyboard = match mode {
            OutputMode::Keyboard | OutputMode::Both => Some(KeyboardOutput::new()?),
            OutputMode::Clipboard => None,
        };

        Ok(Self {
            mode,
            clipboard,
            keyboard,
        })
    }

    /// Get the current output mode
    pub fn mode(&self) -> OutputMode {
        self.mode
    }
}

#[async_trait]
impl OutputSink for CombinedOutput {
    async fn output_text(&self, text: &str) -> Result<()> {
        tracing::debug!("Outputting text via {:?} mode", self.mode);

        // Output to clipboard if enabled
        if let Some(clipboard) = &self.clipboard {
            clipboard.output_text(text).await?;
        }

        // Output via keyboard if enabled
        if let Some(keyboard) = &self.keyboard {
            keyboard.output_text(text).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_combined_output_clipboard_only() {
        let output = CombinedOutput::new(OutputMode::Clipboard)
            .expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Clipboard);
        assert!(output.clipboard.is_some());
        assert!(output.keyboard.is_none());
    }

    #[tokio::test]
    async fn test_combined_output_keyboard_only() {
        let output = CombinedOutput::new(OutputMode::Keyboard)
            .expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Keyboard);
        assert!(output.clipboard.is_none());
        assert!(output.keyboard.is_some());
    }

    #[tokio::test]
    async fn test_combined_output_both() {
        let output = CombinedOutput::new(OutputMode::Both)
            .expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Both);
        assert!(output.clipboard.is_some());
        assert!(output.keyboard.is_some());
    }

    #[tokio::test]
    async fn test_combined_output_text() {
        let output = CombinedOutput::new(OutputMode::Clipboard)
            .expect("Failed to create combined output");

        let test_text = "Hello from combined output!";
        let result = output.output_text(test_text).await;

        assert!(result.is_ok(), "Failed to output text: {:?}", result.err());
    }
}
