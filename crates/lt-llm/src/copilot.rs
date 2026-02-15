use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::time::Instant;

use crate::executor::CliExecutor;
use crate::prompts::PromptManager;

/// Copilot CLI adapter implementing LlmProcessor trait (stub implementation)
pub struct CopilotProcessor {
    executor: CliExecutor,
    prompt_manager: PromptManager,
    model: Option<String>,
}

pub const DEFAULT_MODEL: &str = "gpt-5-mini";

impl CopilotProcessor {
    /// Create a new Copilot processor with default settings
    pub fn new() -> Self {
        Self {
            executor: CliExecutor::with_timeout(30),
            prompt_manager: PromptManager::new(),
            model: Some(DEFAULT_MODEL.to_string()),
        }
    }

    /// Create a new Copilot processor with an optional model override
    pub fn with_model(model: Option<String>) -> Self {
        let model = Some(
            model
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        );
        Self {
            executor: CliExecutor::with_timeout(30),
            prompt_manager: PromptManager::new(),
            model,
        }
    }

    /// Create a new Copilot processor with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            executor: CliExecutor::with_timeout(timeout_secs),
            prompt_manager: PromptManager::new(),
            model: Some(DEFAULT_MODEL.to_string()),
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

        // Build prompt from embedded template
        let prompt = self.prompt_manager.build_prompt(&task);

        tracing::debug!(
            "Executing copilot CLI with prompt (length: {} chars)",
            prompt.len()
        );

        // Execute copilot CLI
        // Format: copilot --prompt "prompt" [--model <model>]
        let mut args = vec!["--prompt", &prompt];
        let model_str;
        if let Some(ref model) = self.model {
            model_str = model.clone();
            args.push("--model");
            args.push(&model_str);
        }
        let output = self.executor.execute("copilot", &args).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                MurmurError::Llm("Copilot CLI timed out".to_string())
            } else if e.kind() == std::io::ErrorKind::NotFound {
                MurmurError::Llm("Copilot CLI not found. Please install copilot-cli.".to_string())
            } else {
                MurmurError::Llm(format!("Failed to execute copilot CLI: {}", e))
            }
        })?;

        // Check exit code
        if output.exit_code != 0 {
            tracing::error!(
                "Copilot CLI failed with exit code {}: {}",
                output.exit_code,
                output.stderr
            );
            return Err(MurmurError::Llm(format!(
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
