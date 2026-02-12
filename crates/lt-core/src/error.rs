use thiserror::Error;

#[derive(Error, Debug)]
pub enum LocaltypeError {
    #[error("Audio error: {0}")]
    Audio(String),

    #[error("STT error: {0}")]
    Stt(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Output error: {0}")]
    Output(String),

    #[error("Permission error: {0}")]
    Permission(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, LocaltypeError>;
