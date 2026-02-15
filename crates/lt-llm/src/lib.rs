pub mod copilot;
pub mod executor;
pub mod gemini;
pub mod http_api;
pub mod prompts;

#[cfg(target_os = "macos")]
pub mod apple;

pub use copilot::CopilotProcessor;
pub use executor::CliExecutor;
pub use gemini::GeminiProcessor;
pub use http_api::HttpLlmProcessor;
pub use prompts::PromptManager;

#[cfg(target_os = "macos")]
pub use apple::AppleLlmProcessor;
