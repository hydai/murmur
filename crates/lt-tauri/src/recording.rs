use lt_pipeline::PipelineState;
use std::sync::{Mutex, MutexGuard};

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

/// Tells a press that waited behind a start from a fresh one. A start holds
/// the pipeline until the provider has connected or failed, so a press made
/// meanwhile waits. If that start failed, the press finds nothing to stop and
/// would start again, opening the microphone a second time; `start_recording`
/// asks `waited` under the pipeline lock, right before it begins, and such a
/// press does nothing instead. A press takes its look under the same lock a
/// start changes its flag and its count under, so it never sees half of a
/// start beginning.
#[derive(Default)]
pub(crate) struct StartTracker(Mutex<Starts>);

/// The starts as they are, or as a press saw them.
#[derive(Default, Clone, Copy)]
struct Starts {
    running: bool,
    begun: u64,
}

/// What a press saw of the starts when it came.
pub(crate) struct Arrival(Starts);

impl StartTracker {
    fn starts(&self) -> MutexGuard<'_, Starts> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn arrive(&self) -> Arrival {
        Arrival(*self.starts())
    }

    /// Whether a start ran while the press waited: one was running when it
    /// came, or one has begun since, while the press was choosing its action.
    pub(crate) fn waited(&self, arrival: &Arrival) -> bool {
        arrival.0.running || self.starts().begun != arrival.0.begun
    }

    /// Marks a start as running until the guard drops, whichever way the
    /// start returns.
    pub(crate) fn begin(&self) -> StartInProgress<'_> {
        let mut starts = self.starts();
        starts.begun += 1;
        starts.running = true;
        StartInProgress(self)
    }
}

/// A start that is running, for as long as it lives.
pub(crate) struct StartInProgress<'a>(&'a StartTracker);

impl Drop for StartInProgress<'_> {
    fn drop(&mut self) {
        self.0.starts().running = false;
    }
}

/// What the menu bar's recording item says: what choosing it would do, so it
/// never says Stop while choosing it would cancel.
pub(crate) fn menu_label(action: ToggleAction) -> &'static str {
    match action {
        ToggleAction::Start => "Start Recording",
        ToggleAction::Stop => "Stop Recording",
        ToggleAction::Cancel => "Cancel Dictation",
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
    fn a_press_made_during_a_start_waited_behind_it() {
        let starts = StartTracker::default();
        let starting = starts.begin();
        let press = starts.arrive();
        drop(starting);
        assert!(starts.waited(&press));
    }

    #[test]
    fn a_press_made_just_before_a_start_began_waited_behind_it() {
        // It chose Start while the other press's start was still about to
        // take the pipeline.
        let starts = StartTracker::default();
        let press = starts.arrive();
        drop(starts.begin());
        assert!(starts.waited(&press));
    }

    #[test]
    fn a_press_never_sees_half_of_a_start_beginning() {
        // A start is halfway through beginning: its count has moved and its
        // flag not yet. A press that looks now waits for the whole change.
        let starts = std::sync::Arc::new(StartTracker::default());
        let mut beginning = starts.starts();
        beginning.begun += 1;
        let looking = {
            let starts = starts.clone();
            std::thread::spawn(move || starts.arrive())
        };
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(!looking.is_finished());
        beginning.running = true;
        drop(beginning);

        let press = looking.join().unwrap();
        starts.starts().running = false;
        assert!(starts.waited(&press));
    }

    #[test]
    fn a_press_made_after_a_start_waited_for_nothing() {
        let starts = StartTracker::default();
        drop(starts.begin());
        let press = starts.arrive();
        assert!(!starts.waited(&press));
    }

    #[test]
    fn the_menu_bar_item_says_what_choosing_it_would_do() {
        assert_eq!(
            menu_label(toggle_action(PipelineState::Idle, false)),
            "Start Recording"
        );
        assert_eq!(
            menu_label(toggle_action(PipelineState::Recording, true)),
            "Stop Recording"
        );
        // After Stop, a batch provider stays in Recording until it has the
        // audio: choosing the item then cancels, so it must not say Stop.
        assert_eq!(
            menu_label(toggle_action(PipelineState::Recording, false)),
            "Cancel Dictation"
        );
        assert_eq!(
            menu_label(toggle_action(PipelineState::Processing, false)),
            "Cancel Dictation"
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
