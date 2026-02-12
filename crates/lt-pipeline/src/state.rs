use serde::{Deserialize, Serialize};

/// Pipeline state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PipelineState {
    /// Pipeline is idle, ready to start
    Idle,
    /// Recording audio from microphone
    Recording,
    /// Transcribing audio to text via STT
    Transcribing,
    /// Processing text via LLM
    Processing,
    /// Pipeline completed successfully
    Done,
    /// Pipeline encountered an error
    Error,
}

impl Default for PipelineState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Pipeline events emitted during state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PipelineEvent {
    /// State changed
    StateChanged {
        state: PipelineState,
        timestamp_ms: u64,
    },
    /// Audio level update (for waveform)
    AudioLevel {
        rms: f32,
        voice_active: bool,
        timestamp_ms: u64,
    },
    /// Partial transcription
    PartialTranscription {
        text: String,
        timestamp_ms: u64,
    },
    /// Committed transcription
    CommittedTranscription {
        text: String,
        timestamp_ms: u64,
    },
    /// Final result after LLM processing
    FinalResult {
        text: String,
        processing_time_ms: u64,
    },
    /// Error occurred
    Error {
        message: String,
        recoverable: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_state_default() {
        assert_eq!(PipelineState::default(), PipelineState::Idle);
    }

    #[test]
    fn test_pipeline_state_transitions() {
        let states = vec![
            PipelineState::Idle,
            PipelineState::Recording,
            PipelineState::Transcribing,
            PipelineState::Processing,
            PipelineState::Done,
        ];

        for state in states {
            let serialized = serde_json::to_string(&state).unwrap();
            let deserialized: PipelineState = serde_json::from_str(&serialized).unwrap();
            assert_eq!(state, deserialized);
        }
    }

    #[test]
    fn test_pipeline_event_serialization() {
        let event = PipelineEvent::StateChanged {
            state: PipelineState::Recording,
            timestamp_ms: 1000,
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: PipelineEvent = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            PipelineEvent::StateChanged { state, timestamp_ms } => {
                assert_eq!(state, PipelineState::Recording);
                assert_eq!(timestamp_ms, 1000);
            }
            _ => panic!("Unexpected event type"),
        }
    }
}
