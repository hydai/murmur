use lt_pipeline::PipelineState;

/// What a "toggle recording" gesture (hotkey, tray, overlay button) should do.
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
