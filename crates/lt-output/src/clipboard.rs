use arboard::Clipboard;
use async_trait::async_trait;
use lt_core::error::Result;
use lt_core::output::OutputSink;

/// Clipboard output sink using arboard
pub struct ClipboardOutput;

impl ClipboardOutput {
    /// Create a new clipboard output sink
    pub fn new() -> Result<Self> {
        // Verify clipboard access works at construction time
        Clipboard::new()
            .map_err(|e| lt_core::error::LocaltypeError::Output(e.to_string()))?;

        Ok(Self)
    }
}

impl Default for ClipboardOutput {
    fn default() -> Self {
        Self::new().expect("Failed to initialize clipboard")
    }
}

#[async_trait]
impl OutputSink for ClipboardOutput {
    async fn output_text(&self, text: &str) -> Result<()> {
        // arboard's set_text is not thread-safe, so we need to create a new clipboard instance
        // for each operation to avoid issues
        let mut clipboard = Clipboard::new()
            .map_err(|e| lt_core::error::LocaltypeError::Output(e.to_string()))?;

        clipboard
            .set_text(text.to_string())
            .map_err(|e| lt_core::error::LocaltypeError::Output(e.to_string()))?;

        tracing::info!("Text copied to clipboard ({} chars)", text.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_output() {
        let output = ClipboardOutput::new().expect("Failed to create clipboard output");
        let test_text = "Hello, clipboard!";

        let result = output.output_text(test_text).await;
        assert!(result.is_ok(), "Failed to write to clipboard: {:?}", result.err());

        // Verify by reading back from clipboard
        let mut clipboard = Clipboard::new().expect("Failed to create clipboard");
        let read_text = clipboard.get_text().expect("Failed to read from clipboard");
        assert_eq!(read_text, test_text);
    }
}
