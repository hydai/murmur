pub mod copilot;
pub mod executor;
pub mod gemini;
pub mod prompts;

pub use copilot::CopilotProcessor;
pub use executor::CliExecutor;
pub use gemini::GeminiProcessor;
pub use prompts::PromptManager;
