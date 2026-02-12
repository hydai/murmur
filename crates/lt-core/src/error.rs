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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LocaltypeError::Audio("device not found".to_string());
        assert_eq!(err.to_string(), "Audio error: device not found");

        let err = LocaltypeError::Stt("connection failed".to_string());
        assert_eq!(err.to_string(), "STT error: connection failed");

        let err = LocaltypeError::Llm("CLI not found".to_string());
        assert_eq!(err.to_string(), "LLM error: CLI not found");

        let err = LocaltypeError::Permission("microphone denied".to_string());
        assert_eq!(err.to_string(), "Permission error: microphone denied");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: LocaltypeError = io_err.into();
        assert!(matches!(err, LocaltypeError::Io(_)));
    }

    #[test]
    fn test_error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: LocaltypeError = json_err.into();
        assert!(matches!(err, LocaltypeError::Json(_)));
    }

    #[test]
    fn test_error_from_toml() {
        let toml_err = toml::from_str::<toml::Value>("invalid = = toml").unwrap_err();
        let err: LocaltypeError = toml_err.into();
        assert!(matches!(err, LocaltypeError::Toml(_)));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<String> = Ok("success".to_string());
        assert!(ok_result.is_ok());

        let err_result: Result<String> = Err(LocaltypeError::Audio("test".to_string()));
        assert!(err_result.is_err());
    }
}
