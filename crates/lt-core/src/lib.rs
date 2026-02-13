pub mod config;
pub mod dictionary;
pub mod error;
pub mod llm;
pub mod output;
pub mod stt;

pub use config::{AppConfig, LlmProcessorType, SttProviderType, UiPreferences};
pub use dictionary::{DictionaryEntry, PersonalDictionary};
pub use error::MurmurError;
pub use llm::{LlmProcessor, ProcessingOutput, ProcessingTask};
pub use output::{OutputMode, OutputSink};
pub use stt::{AudioChunk, SttProvider, TranscriptionEvent};
