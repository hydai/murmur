pub mod commands;
pub mod orchestrator;
pub mod state;
mod text_normalization;

pub use commands::{detect_command, CommandDetection};
pub use orchestrator::PipelineOrchestrator;
pub use state::{PipelineEvent, PipelineState};
