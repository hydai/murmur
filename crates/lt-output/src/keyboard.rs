use async_trait::async_trait;
use enigo::{Enigo, Keyboard, Settings};
use lt_core::error::Result;
use lt_core::output::OutputSink;

/// Keyboard simulation output sink using enigo
/// Note: Keyboard simulation is not thread-safe due to enigo limitations
pub struct KeyboardOutput;

impl KeyboardOutput {
    /// Create a new keyboard output sink
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Default for KeyboardOutput {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl OutputSink for KeyboardOutput {
    async fn output_text(&self, text: &str) -> Result<()> {
        // Note: enigo is not Send/Sync, so we create a new instance for each operation
        // This is a limitation of the underlying CGEvent API on macOS
        let settings = Settings::default();
        let mut enigo = Enigo::new(&settings)
            .map_err(|e| lt_core::error::MurmurError::Output(format!("Failed to initialize enigo: {}", e)))?;

        // Type the text character by character
        enigo.text(text)
            .map_err(|e| lt_core::error::MurmurError::Output(format!("Failed to type text: {}", e)))?;

        tracing::info!("Text typed via keyboard simulation ({} chars)", text.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_keyboard_output_creation() {
        let output = KeyboardOutput::new();
        assert!(output.is_ok(), "Failed to create keyboard output");
    }

    // Note: We cannot easily test actual keyboard typing in unit tests
    // as it requires GUI interaction. Manual testing is required.
}
