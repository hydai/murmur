use lt_pipeline::PipelineState;

/// What a "toggle recording" gesture (the hotkey or the menu bar) should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToggleAction {
    Start,
    Stop,
    Cancel,
}

/// Pressing again while the microphone is open stops the recording; pressing
/// again while a session is only finishing or processing cancels it, which is
/// also the way out of a session that never reached a terminal state.
pub(crate) fn toggle_action(state: PipelineState, capturing: bool) -> ToggleAction {
    match state {
        PipelineState::Recording | PipelineState::Transcribing if capturing => ToggleAction::Stop,
        PipelineState::Recording | PipelineState::Transcribing | PipelineState::Processing => {
            ToggleAction::Cancel
        }
        PipelineState::Idle | PipelineState::Done | PipelineState::Error => ToggleAction::Start,
    }
}

/// Why toggling the recording failed, and whether the UI has heard about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToggleFailure {
    /// `Pipeline::start` failed after announcing the recording: it has sent the
    /// error and settled on Error, so the capsule shows the failure already.
    Reported(String),
    /// Nothing has told the UI: the start failed before the pipeline ran (no API
    /// key, say), or a stop or cancel failed.
    Unreported(String),
}

impl ToggleFailure {
    /// The message still to be reported, when nothing has reported it yet.
    pub(crate) fn unreported(self) -> Option<String> {
        match self {
            ToggleFailure::Reported(_) => None,
            ToggleFailure::Unreported(message) => Some(message),
        }
    }

    pub(crate) fn into_message(self) -> String {
        match self {
            ToggleFailure::Reported(message) | ToggleFailure::Unreported(message) => message,
        }
    }
}

/// Every failure on the way to starting the pipeline is one nothing has reported.
impl From<String> for ToggleFailure {
    fn from(message: String) -> Self {
        ToggleFailure::Unreported(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lt_pipeline::PipelineState;

    #[test]
    fn a_live_capture_is_stopped() {
        assert_eq!(
            toggle_action(PipelineState::Recording, true),
            ToggleAction::Stop
        );
        assert_eq!(
            toggle_action(PipelineState::Transcribing, true),
            ToggleAction::Stop
        );
    }

    #[test]
    fn a_session_that_is_finishing_or_processing_is_cancelled() {
        assert_eq!(
            toggle_action(PipelineState::Recording, false),
            ToggleAction::Cancel
        );
        assert_eq!(
            toggle_action(PipelineState::Transcribing, false),
            ToggleAction::Cancel
        );
        assert_eq!(
            toggle_action(PipelineState::Processing, false),
            ToggleAction::Cancel
        );
        assert_eq!(
            toggle_action(PipelineState::Processing, true),
            ToggleAction::Cancel
        );
    }

    #[test]
    fn only_a_failure_nothing_has_reported_is_handed_on_to_be_reported() {
        assert_eq!(
            ToggleFailure::from("No API key".to_string()).unreported(),
            Some("No API key".to_string())
        );
        assert_eq!(
            ToggleFailure::Reported("STT error: refused".into()).unreported(),
            None
        );
        assert_eq!(
            ToggleFailure::Reported("STT error: refused".into()).into_message(),
            "STT error: refused"
        );
    }

    #[test]
    fn idle_done_and_error_start_a_recording() {
        for state in [
            PipelineState::Idle,
            PipelineState::Done,
            PipelineState::Error,
        ] {
            assert_eq!(toggle_action(state, false), ToggleAction::Start);
        }
    }
}
