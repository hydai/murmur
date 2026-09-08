//! Shared implementation for LLM processors backed by a command-line tool.
//!
//! Mirrors what `HttpSttProvider` does for the REST speech providers: the
//! differences between one CLI and the next collapse into a [`CliSpec`], so
//! timeout handling, error mapping, exit-code checking and health probing exist
//! once instead of being copied per tool.

use async_trait::async_trait;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
use std::time::Instant;

use crate::executor::CliExecutor;
use crate::prompts::PromptManager;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Everything that differs between one CLI-backed processor and the next.
pub struct CliSpec {
    /// Executable resolved on PATH.
    pub binary: &'static str,
    /// Name used in log lines and user-facing errors, e.g. "Gemini CLI".
    pub display_name: &'static str,
    /// Model used when the config carries no override.
    pub default_model: &'static str,
    /// Appended to the "not found" error.
    pub install_hint: &'static str,
    /// Arguments for one invocation.
    pub args: fn(prompt: &str, model: &str) -> Vec<String>,
    /// Extracts the reply from the tool's stdout.
    pub parse: fn(stdout: &str) -> String,
}

/// An `LlmProcessor` that shells out to the tool described by its spec.
pub struct CliLlmProcessor {
    spec: &'static CliSpec,
    executor: CliExecutor,
    prompt_manager: PromptManager,
    model: String,
}

impl CliLlmProcessor {
    pub fn with_model_and_prompts(
        spec: &'static CliSpec,
        model: Option<String>,
        prompts: PromptManager,
    ) -> Self {
        Self {
            spec,
            executor: CliExecutor::with_timeout(DEFAULT_TIMEOUT_SECS),
            prompt_manager: prompts,
            model: model
                .filter(|model| !model.is_empty())
                .unwrap_or_else(|| spec.default_model.to_string()),
        }
    }

    /// The model this processor will pass to the tool.
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProcessor for CliLlmProcessor {
    async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
        let start_time = Instant::now();
        let name = self.spec.display_name;

        // Built from the shared prompt set, so user overrides are respected.
        let prompt = self.prompt_manager.build_prompt(&task).await;
        tracing::debug!(
            "Executing {name} with prompt (length: {} chars)",
            prompt.len()
        );

        let args = (self.spec.args)(&prompt, &self.model);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = self
            .executor
            .execute(self.spec.binary, &args)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::TimedOut => MurmurError::Llm(format!("{name} timed out")),
                std::io::ErrorKind::NotFound => {
                    MurmurError::Llm(format!("{name} not found. {}", self.spec.install_hint))
                }
                _ => MurmurError::Llm(format!("Failed to execute {name}: {error}")),
            })?;

        if output.exit_code != 0 {
            tracing::error!(
                "{name} failed with exit code {}: {}",
                output.exit_code,
                output.stderr
            );
            return Err(MurmurError::Llm(format!(
                "{name} failed: {}",
                output.stderr
            )));
        }

        let processed_text = (self.spec.parse)(&output.stdout);
        let processing_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::info!(
            "LLM processing completed in {processing_time_ms}ms (output length: {} chars)",
            processed_text.len()
        );

        Ok(ProcessingOutput {
            text: processed_text,
            processing_time_ms,
            metadata: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let name = self.spec.display_name;
        tracing::debug!("Performing {name} health check");

        if self.executor.is_available(self.spec.binary).await {
            tracing::info!("{name} is available");
            Ok(true)
        } else {
            tracing::warn!("{name} is not available in PATH");
            Ok(false)
        }
    }
}

/// Wrap [`CliLlmProcessor`] in a named type with the project's constructor set.
///
/// The processors differ only by spec, so generating the wrapper keeps the four
/// call sites in `create_llm_processor` unchanged while the behaviour lives in
/// one place.
macro_rules! cli_processor {
    ($name:ident, $spec:expr) => {
        pub struct $name {
            inner: $crate::cli::CliLlmProcessor,
        }

        impl $name {
            pub fn new() -> Self {
                Self::with_model_and_prompts(None, $crate::prompts::PromptManager::new())
            }

            /// Create a processor with an optional model override.
            pub fn with_model(model: Option<String>) -> Self {
                Self::with_model_and_prompts(model, $crate::prompts::PromptManager::new())
            }

            /// Create a processor with a model override and a shared PromptManager.
            pub fn with_model_and_prompts(
                model: Option<String>,
                prompts: $crate::prompts::PromptManager,
            ) -> Self {
                Self {
                    inner: $crate::cli::CliLlmProcessor::with_model_and_prompts(
                        &$spec, model, prompts,
                    ),
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        #[async_trait::async_trait]
        impl lt_core::llm::LlmProcessor for $name {
            async fn process(
                &self,
                task: lt_core::llm::ProcessingTask,
            ) -> lt_core::error::Result<lt_core::llm::ProcessingOutput> {
                self.inner.process(task).await
            }
            async fn health_check(&self) -> lt_core::error::Result<bool> {
                self.inner.health_check().await
            }
        }
    };
}
pub(crate) use cli_processor;
