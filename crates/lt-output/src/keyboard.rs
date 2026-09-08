use std::time::Duration;

use async_trait::async_trait;
use enigo::{Enigo, Keyboard, Settings};
use lt_core::error::{MurmurError, Result};
use lt_core::output::OutputSink;

/// Typing a full dictation takes well under this; anything longer means the
/// accessibility API is stuck.
const TYPING_TIMEOUT: Duration = Duration::from_secs(30);

/// Keyboard simulation output sink using enigo
/// Note: Keyboard simulation is not thread-safe due to enigo limitations
pub struct KeyboardOutput;

/// Run a blocking typing job off the async runtime and bound it. A job that
/// outlives the timeout keeps running on its blocking thread (enigo cannot be
/// interrupted), but the pipeline moves on and reports the failure.
pub(crate) async fn type_with_timeout<F>(job: F, timeout: Duration) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    match tokio::time::timeout(timeout, tokio::task::spawn_blocking(job)).await {
        Ok(Ok(result)) => result,
        Ok(Err(join)) => Err(MurmurError::Output(format!(
            "Keyboard output task failed: {join}"
        ))),
        Err(_) => Err(MurmurError::Output(format!(
            "Keyboard output timed out after {timeout:?}"
        ))),
    }
}

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
        let text = text.to_owned();
        let chars = text.chars().count();
        // enigo is neither Send nor Sync and blocks on CGEvent calls, so each
        // delivery builds its own instance on a blocking thread.
        type_with_timeout(
            move || {
                let settings = Settings::default();
                let mut enigo = Enigo::new(&settings)
                    .map_err(|e| MurmurError::Output(format!("Failed to initialize enigo: {e}")))?;
                enigo
                    .text(&text)
                    .map_err(|e| MurmurError::Output(format!("Failed to type text: {e}")))
            },
            TYPING_TIMEOUT,
        )
        .await?;

        tracing::info!("Text typed via keyboard simulation ({chars} chars)");
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

    #[tokio::test]
    async fn typing_runs_off_the_runtime_and_is_bounded() {
        // Blocking-pool threads carry a runtime handle, so the observable
        // property is that the job leaves the current-thread runtime's thread.
        let runtime_thread = std::thread::current().id();
        let ok = type_with_timeout(
            move || {
                assert_ne!(
                    std::thread::current().id(),
                    runtime_thread,
                    "must not run on the runtime thread"
                );
                Ok(())
            },
            std::time::Duration::from_secs(1),
        )
        .await;
        assert!(ok.is_ok());

        let stalled = type_with_timeout(
            || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(())
            },
            std::time::Duration::from_millis(20),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(stalled.contains("timed out"), "{stalled}");
    }
}
