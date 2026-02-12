use async_trait::async_trait;
use lt_core::error::{LocaltypeError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::time::Instant;

use crate::executor::CliExecutor;
use crate::prompts::PromptManager;

/// Copilot CLI adapter implementing LlmProcessor trait (stub implementation)
pub struct CopilotProcessor {
    executor: CliExecutor,
    prompt_manager: PromptManager,
}

impl CopilotProcessor {
    /// Create a new Copilot processor with default settings
    pub fn new() -> Self {
        Self {
            executor: CliExecutor::with_timeout(30),
            prompt_manager: PromptManager::new(),
        }
    }

    /// Create a new Copilot processor with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            executor: CliExecutor::with_timeout(timeout_secs),
            prompt_manager: PromptManager::new(),
        }
    }
}

impl Default for CopilotProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProcessor for CopilotProcessor {
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
        let start_time = Instant::now();

        // Build prompt from template
        let prompt = self.prompt_manager.build_prompt(&task).map_err(|e| {
            LocaltypeError::Llm(format!("Failed to build prompt template: {}", e))
        })?;

        tracing::debug!("Executing copilot CLI with prompt (length: {} chars)", prompt.len());

        // Execute copilot CLI
        // Format: copilot --prompt "prompt"
        let output = self
            .executor
            .execute("copilot", &["--prompt", &prompt])
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    LocaltypeError::Llm("Copilot CLI timed out".to_string())
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    LocaltypeError::Llm(
                        "Copilot CLI not found. Please install copilot-cli.".to_string()
                    )
                } else {
                    LocaltypeError::Llm(format!("Failed to execute copilot CLI: {}", e))
                }
            })?;

        // Check exit code
        if output.exit_code != 0 {
            tracing::error!("Copilot CLI failed with exit code {}: {}", output.exit_code, output.stderr);
            return Err(LocaltypeError::Llm(format!(
                "Copilot CLI failed: {}",
                output.stderr
            )));
        }

        let processed_text = output.stdout.trim().to_string();
        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            "LLM processing completed in {}ms (output length: {} chars)",
            processing_time_ms,
            processed_text.len()
        );

        Ok(ProcessingOutput {
            text: processed_text,
            processing_time_ms,
            metadata: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        tracing::debug!("Performing copilot CLI health check");

        let is_available = self.executor.is_available("copilot").await;

        if is_available {
            tracing::info!("Copilot CLI is available");
            Ok(true)
        } else {
            tracing::warn!("Copilot CLI is not available in PATH");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_copilot_health_check() {
        let processor = CopilotProcessor::new();
        // This will return false if copilot is not installed, which is expected
        let _ = processor.health_check().await;
    }
}
