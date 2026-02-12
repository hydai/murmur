use thiserror::Error;

pub type Result<T> = std::result::Result<T, AudioError>;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("No audio input device available")]
    NoInputDevice,

    #[error("Audio device error: {0}")]
    DeviceError(String),

    #[error("Audio format not supported: {0}")]
    UnsupportedFormat(String),

    #[error("Audio stream error: {0}")]
    StreamError(String),

    #[error("Resampling error: {0}")]
    ResamplingError(String),

    #[error("Channel send error: audio pipeline full")]
    ChannelFull,

    #[error("Channel receive error: {0}")]
    ChannelClosed(String),

    #[error("Microphone permission denied")]
    PermissionDenied,

    #[error("Audio capture not started")]
    NotStarted,

    #[error("Audio capture already running")]
    AlreadyRunning,
}

impl From<cpal::DevicesError> for AudioError {
    fn from(err: cpal::DevicesError) -> Self {
        AudioError::DeviceError(err.to_string())
    }
}

impl From<cpal::DefaultStreamConfigError> for AudioError {
    fn from(err: cpal::DefaultStreamConfigError) -> Self {
        AudioError::DeviceError(err.to_string())
    }
}

impl From<cpal::BuildStreamError> for AudioError {
    fn from(err: cpal::BuildStreamError) -> Self {
        AudioError::StreamError(err.to_string())
    }
}

impl From<cpal::PlayStreamError> for AudioError {
    fn from(err: cpal::PlayStreamError) -> Self {
        AudioError::StreamError(err.to_string())
    }
}
