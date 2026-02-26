pub mod chunker;
pub mod custom;
pub mod elevenlabs;
pub mod groq;
pub mod openai;

#[cfg(target_os = "macos")]
pub mod apple;

pub use custom::CustomSttProvider;
pub use elevenlabs::ElevenLabsProvider;
pub use groq::GroqProvider;
pub use openai::OpenAIProvider;

#[cfg(target_os = "macos")]
pub use apple::AppleSttProvider;
