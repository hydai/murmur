use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::time::Instant;

use crate::executor::CliExecutor;
use crate::prompts::PromptManager;

/// Gemini CLI adapter implementing LlmProcessor trait
pub struct GeminiProcessor {
    executor: CliExecutor,
    prompt_manager: PromptManager,
}

impl GeminiProcessor {
    /// Create a new Gemini processor with default settings
    pub fn new() -> Self {
        Self {
            executor: CliExecutor::with_timeout(30),
            prompt_manager: PromptManager::new(),
        }
    }

    /// Create a new Gemini processor with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            executor: CliExecutor::with_timeout(timeout_secs),
            prompt_manager: PromptManager::new(),
        }
    }

    /// Parse JSON output from gemini CLI
    fn parse_json_output(&self, output: &str) -> Result<String> {
        // Try to parse as JSON first
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
            // Check for various possible JSON response formats
            if let Some(text) = json.get("text").and_then(|v| v.as_str()) {
                return Ok(text.to_string());
            }
            if let Some(text) = json.get("content").and_then(|v| v.as_str()) {
                return Ok(text.to_string());
            }
            if let Some(text) = json.get("response").and_then(|v| v.as_str()) {
                return Ok(text.to_string());
            }
            // If it's a string value directly
            if let Some(text) = json.as_str() {
                return Ok(text.to_string());
            }
        }

        // If JSON parsing fails or no recognized fields, return output as-is
        // (gemini might return plain text even with --output-format json)
        Ok(output.trim().to_string())
    }
}

impl Default for GeminiProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmProcessor for GeminiProcessor {
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
        let start_time = Instant::now();

        // Build prompt from template
        let prompt = self.prompt_manager.build_prompt(&task).map_err(|e| {
            MurmurError::Llm(format!("Failed to build prompt template: {}", e))
        })?;

        tracing::debug!("Executing gemini CLI with prompt (length: {} chars)", prompt.len());

        // Execute gemini CLI
        // Format: gemini -p "prompt" --output-format json -m gemini-2.5-flash
        let output = self
            .executor
            .execute(
                "gemini",
                &[
                    "-p",
                    &prompt,
                    "--output-format",
                    "json",
                    "-m",
                    "gemini-2.5-flash",
                ],
            )
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    MurmurError::Llm("Gemini CLI timed out".to_string())
                } else if e.kind() == std::io::ErrorKind::NotFound {
                    MurmurError::Llm(
                        "Gemini CLI not found. Please install gemini-cli: https://github.com/google/generative-ai-cli".to_string()
                    )
                } else {
                    MurmurError::Llm(format!("Failed to execute gemini CLI: {}", e))
                }
            })?;

        // Check exit code
        if output.exit_code != 0 {
            tracing::error!("Gemini CLI failed with exit code {}: {}", output.exit_code, output.stderr);
            return Err(MurmurError::Llm(format!(
                "Gemini CLI failed: {}",
                output.stderr
            )));
        }

        // Parse output
        let processed_text = self.parse_json_output(&output.stdout)?;

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
        tracing::debug!("Performing gemini CLI health check");

        let is_available = self.executor.is_available("gemini").await;

        if is_available {
            tracing::info!("Gemini CLI is available");
            Ok(true)
        } else {
            tracing::warn!("Gemini CLI is not available in PATH");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gemini_health_check() {
        let processor = GeminiProcessor::new();
        // This will return false if gemini is not installed, which is expected
        let _ = processor.health_check().await;
    }

    #[test]
    fn test_parse_json_output() {
        let processor = GeminiProcessor::new();

        // Test with "text" field
        let json1 = r#"{"text": "Hello world"}"#;
        assert_eq!(
            processor.parse_json_output(json1).unwrap(),
            "Hello world"
        );

        // Test with "content" field
        let json2 = r#"{"content": "Hello world"}"#;
        assert_eq!(
            processor.parse_json_output(json2).unwrap(),
            "Hello world"
        );

        // Test with plain text fallback
        let plain = "Hello world";
        assert_eq!(
            processor.parse_json_output(plain).unwrap(),
            "Hello world"
        );
    }
}
