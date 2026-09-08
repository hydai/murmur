use async_trait::async_trait;
use lt_core::error::Result;
use lt_core::output::{OutputMode, OutputSink};

use crate::clipboard::ClipboardOutput;
use crate::keyboard::KeyboardOutput;

/// Combined output sink that routes to clipboard, keyboard, or both
pub struct CombinedOutput {
    mode: OutputMode,
    clipboard: Option<Box<dyn OutputSink>>,
    keyboard: Option<Box<dyn OutputSink>>,
}

impl CombinedOutput {
    /// Create a new combined output sink with the specified mode
    pub fn new(mode: OutputMode) -> Result<Self> {
        let clipboard: Option<Box<dyn OutputSink>> = match mode {
            OutputMode::Clipboard | OutputMode::Both => Some(Box::new(ClipboardOutput::new()?)),
            OutputMode::Keyboard => None,
        };

        let keyboard: Option<Box<dyn OutputSink>> = match mode {
            OutputMode::Keyboard | OutputMode::Both => Some(Box::new(KeyboardOutput::new()?)),
            OutputMode::Clipboard => None,
        };

        Ok(Self {
            mode,
            clipboard,
            keyboard,
        })
    }

    #[cfg(test)]
    fn from_sinks(
        mode: OutputMode,
        clipboard: Option<Box<dyn OutputSink>>,
        keyboard: Option<Box<dyn OutputSink>>,
    ) -> Self {
        Self {
            mode,
            clipboard,
            keyboard,
        }
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

        // Each destination is attempted independently: in Both mode a failed
        // clipboard write must not swallow the typed delivery, and vice versa.
        let mut failures = Vec::new();
        if let Some(clipboard) = &self.clipboard {
            if let Err(error) = clipboard.output_text(text).await {
                tracing::error!("Clipboard output failed: {error}");
                failures.push(format!("clipboard: {error}"));
            }
        }
        if let Some(keyboard) = &self.keyboard {
            if let Err(error) = keyboard.output_text(text).await {
                tracing::error!("Keyboard output failed: {error}");
                failures.push(format!("keyboard: {error}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(lt_core::error::MurmurError::Output(failures.join("; ")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_combined_output_clipboard_only() {
        let output =
            CombinedOutput::new(OutputMode::Clipboard).expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Clipboard);
        assert!(output.clipboard.is_some());
        assert!(output.keyboard.is_none());
    }

    #[tokio::test]
    async fn test_combined_output_keyboard_only() {
        let output =
            CombinedOutput::new(OutputMode::Keyboard).expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Keyboard);
        assert!(output.clipboard.is_none());
        assert!(output.keyboard.is_some());
    }

    #[tokio::test]
    async fn test_combined_output_both() {
        let output =
            CombinedOutput::new(OutputMode::Both).expect("Failed to create combined output");

        assert_eq!(output.mode(), OutputMode::Both);
        assert!(output.clipboard.is_some());
        assert!(output.keyboard.is_some());
    }

    struct RecordingSink(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
    #[async_trait]
    impl OutputSink for RecordingSink {
        async fn output_text(&self, text: &str) -> Result<()> {
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    struct FailingSink;
    #[async_trait]
    impl OutputSink for FailingSink {
        async fn output_text(&self, _: &str) -> Result<()> {
            Err(lt_core::error::MurmurError::Output(
                "pasteboard unavailable".into(),
            ))
        }
    }

    #[tokio::test]
    async fn both_mode_still_types_when_the_clipboard_fails() {
        let typed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let output = CombinedOutput::from_sinks(
            OutputMode::Both,
            Some(Box::new(FailingSink)),
            Some(Box::new(RecordingSink(typed.clone()))),
        );
        let error = output.output_text("hello").await.unwrap_err().to_string();
        assert!(error.contains("clipboard"), "{error}");
        assert!(error.contains("pasteboard unavailable"), "{error}");
        assert_eq!(*typed.lock().unwrap(), ["hello"]);
    }

    #[tokio::test]
    #[ignore = "Types into the focused application; run only in an isolated desktop session"]
    async fn test_combined_output_text() {
        // Test keyboard mode to avoid interfering with clipboard tests
        // Note: This may fail without accessibility permissions on macOS
        let output = CombinedOutput::new(OutputMode::Keyboard);

        if output.is_err() {
            // Skip test if keyboard output requires permissions not available in test environment
            return;
        }

        let output = output.unwrap();
        let test_text = "Hello from combined output!";
        let result = output.output_text(test_text).await;

        // Allow permission errors in test environment
        if let Err(e) = result {
            let err_str = e.to_string();
            if err_str.contains("permission") || err_str.contains("accessibility") {
                // Expected in test environment without accessibility permissions
                return;
            }
            panic!("Failed to output text: {:?}", e);
        }
    }
}
