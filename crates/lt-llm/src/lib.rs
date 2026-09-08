pub mod cli;
pub mod copilot;
pub mod executor;
pub mod gemini;
pub mod http_api;
pub mod prompt_store;
pub mod prompts;

#[cfg(target_os = "macos")]
pub mod apple;

pub use copilot::CopilotProcessor;
pub use executor::CliExecutor;
pub use gemini::GeminiProcessor;
pub use http_api::HttpLlmProcessor;
pub use prompt_store::PromptStore;
pub use prompts::{PromptManager, PromptName, PromptSet};

#[cfg(target_os = "macos")]
pub use apple::AppleLlmProcessor;

/// The model a processor uses when the config carries no override.
///
/// Lives here rather than in the Tauri layer so adding a processor cannot
/// forget it: the match is exhaustive.
pub fn default_model(processor: lt_core::config::LlmProcessorType) -> &'static str {
    use lt_core::config::LlmProcessorType as P;
    match processor {
        P::Gemini => gemini::DEFAULT_MODEL,
        P::Copilot => copilot::DEFAULT_MODEL,
        #[cfg(target_os = "macos")]
        P::AppleLlm => apple::DEFAULT_MODEL,
        #[cfg(not(target_os = "macos"))]
        P::AppleLlm => "",
        P::OpenAiApi | P::CustomApi => http_api::OPENAI_DEFAULT_MODEL,
        P::ClaudeApi => http_api::CLAUDE_DEFAULT_MODEL,
        P::GeminiApi => http_api::GEMINI_API_DEFAULT_MODEL,
    }
}
