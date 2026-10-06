//! The recording capsule's window: where it appears and when it goes away.
//!
//! The capsule lives in the `main` window, which shows nothing else. Rust alone
//! shows and hides that window, so a capsule that misbehaves in the webview can
//! never be left on screen; `ui/src/components/capsule` only draws what is in
//! it. The window ignores the pointer and never takes focus, so the text the
//! pipeline types goes to the app the person is working in. For that reason
//! nothing may call `set_focus()` on it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lt_core::{AppConfig, OutputMode};
use lt_pipeline::PipelineState;
use tauri::{Emitter, LogicalPosition, PhysicalPosition, PhysicalRect};

/// The window that holds the capsule, declared in tauri.conf.json.
pub(crate) const CAPSULE_WINDOW: &str = "main";

pub(crate) const CAPSULE_WIDTH: f64 = 440.0; // logical, tauri.conf.json
pub(crate) const CAPSULE_HEIGHT: f64 = 72.0;
/// The visible 40 pt capsule is centred in the window, so 8 pt here puts it 24 pt above the work area.
const WINDOW_BOTTOM_MARGIN: f64 = 8.0;

// How long each ending stays up. `HIDE_AFTER_MS` in
// ui/src/components/capsule/capsuleState.ts is the same schedule; the capsule
// fades out 200 ms before the window goes.
const RESULT_VISIBLE: Duration = Duration::from_millis(1500);
const ERROR_VISIBLE: Duration = Duration::from_millis(4000);
const CANCELLED_VISIBLE: Duration = Duration::from_millis(1000);

/// Where the window's top-left corner goes so that the capsule sits at the
/// bottom centre of `work_area`, in points. `work_area` is physical, in the
/// scale of its own display, and `scale_factor` is that display's.
///
/// The answer is in points, not pixels, because tao turns a physical position
/// into points with the scale of the display the window is on now, not the one
/// it is moving to. A position in points is used as it is, whichever display
/// the window starts from.
pub(crate) fn capsule_origin(
    work_area: &PhysicalRect<i32, u32>,
    scale_factor: f64,
) -> LogicalPosition<f64> {
    let x = f64::from(work_area.position.x) / scale_factor;
    let y = f64::from(work_area.position.y) / scale_factor;
    let width = f64::from(work_area.size.width) / scale_factor;
    let height = f64::from(work_area.size.height) / scale_factor;
    LogicalPosition::new(
        x + (width - CAPSULE_WIDTH) / 2.0,
        y + height - CAPSULE_HEIGHT - WINDOW_BOTTOM_MARGIN,
    )
}

/// The pointer in points, from the position tao reports. On macOS tao gives the
/// pointer in physical pixels of the primary display, but finds a display by
/// its bounds in points, so the two only agree once that scale is taken off.
fn pointer_in_points(cursor: PhysicalPosition<f64>, primary_scale: f64) -> (f64, f64) {
    (cursor.x / primary_scale, cursor.y / primary_scale)
}

/// How long the capsule stays once the pipeline reaches `state`, or `None`
/// while the session is still under way. Once the session has reported an
/// error, a result or a cancel is held as long as an error, because the capsule
/// shows that error (`Output failed`, for one, or an error partway through a
/// recording that was then cancelled).
pub(crate) fn hide_delay(state: PipelineState, session_had_error: bool) -> Option<Duration> {
    match state {
        PipelineState::Done | PipelineState::Idle if session_had_error => Some(ERROR_VISIBLE),
        PipelineState::Done => Some(RESULT_VISIBLE),
        PipelineState::Error => Some(ERROR_VISIBLE),
        PipelineState::Idle => Some(CANCELLED_VISIBLE),
        PipelineState::Recording | PipelineState::Transcribing | PipelineState::Processing => None,
    }
}

/// What the capsule needs to word its hints, sent to its window as
/// `capsule-context`. `CapsuleContext` in capsuleState.ts mirrors it.
#[derive(Clone, serde::Serialize)]
pub(crate) struct CapsuleContext {
    pub shortcut: String,
    pub output_mode: OutputMode,
    pub save_history: bool,
}

impl CapsuleContext {
    pub(crate) fn from_config(config: &AppConfig) -> Self {
        Self {
            shortcut: config.hotkey.clone(),
            output_mode: config.output_mode,
            save_history: config.save_history,
        }
    }
}

/// What the capsule needs to show a failed toggle, and whether it should, from
/// the config as it could be read. A config that cannot be read (a hand edit
/// left config.toml unparseable, say) makes every attempt to start a recording
/// fail, so the failure is shown all the same: worded for the defaults, and as
/// if the indicator were on, since that setting cannot be looked up. The error
/// on screen is then the person's only hint at what is wrong.
pub(crate) fn failure_context(config: Result<&AppConfig, &str>) -> (CapsuleContext, bool) {
    match config {
        Ok(config) => (
            CapsuleContext::from_config(config),
            config.show_recording_indicator,
        ),
        Err(_) => (CapsuleContext::from_config(&AppConfig::default()), true),
    }
}

/// The capsule's window, behind a trait so that the timing can be tested
/// without a display.
pub(crate) trait CapsuleWindow: Clone + Send + Sync + 'static {
    /// Place at the pointer's display, send `capsule-context`, show. Never focuses.
    fn present(&self, context: &CapsuleContext) -> Result<(), String>;
    fn hide(&self) -> Result<(), String>;
}

impl CapsuleWindow for tauri::WebviewWindow {
    fn present(&self, context: &CapsuleContext) -> Result<(), String> {
        // The display the pointer is on, or the primary one when it cannot be found.
        let primary = self.primary_monitor().ok().flatten();
        let under_pointer = primary.as_ref().and_then(|primary| {
            let pointer = self.cursor_position().ok()?;
            let (x, y) = pointer_in_points(pointer, primary.scale_factor());
            self.monitor_from_point(x, y).ok().flatten()
        });
        if let Some(monitor) = under_pointer.or(primary) {
            self.set_position(capsule_origin(monitor.work_area(), monitor.scale_factor()))
                .map_err(|error| error.to_string())?;
        }
        // The words go first so that the capsule has them when it is seen.
        self.emit_to(CAPSULE_WINDOW, "capsule-context", context.clone())
            .map_err(|error| error.to_string())?;
        self.show().map_err(|error| error.to_string())
    }

    fn hide(&self) -> Result<(), String> {
        tauri::WebviewWindow::hide(self).map_err(|error| error.to_string())
    }
}

/// Decides when the capsule's window is up. It is managed app state, called
/// from the event forwarder, the global shortcut and the menu bar.
#[derive(Default)]
pub(crate) struct Capsule {
    /// Counts the recordings that have begun and the failed toggles that put
    /// the capsule up. A hide scheduled under an earlier count does nothing,
    /// so a new recording cancels the hide that the one before it left
    /// pending.
    generation: Arc<AtomicU64>,
    /// An error was reported since the recording began.
    had_error: AtomicBool,
    /// The window is up, or has been told to come up and not yet to go away.
    visible: Arc<AtomicBool>,
    /// A hide has been scheduled under the current generation. While the window
    /// is up, that means what is on it has ended and is only waiting to go, and
    /// false means a recording is under way. It says nothing once the window
    /// is hidden.
    hide_pending: AtomicBool,
    /// What the next recording's capsule says, and whether it shows, from the
    /// config snapshot `start_pipeline` configures that recording with.
    prepared: Mutex<Option<(CapsuleContext, bool)>>,
}

impl Capsule {
    /// Holds the capsule's context and the indicator switch for the recording
    /// about to start, so the capsule agrees with the session on the output
    /// mode and the switch. `start_pipeline` calls it before the pipeline
    /// starts; the recording's first state takes it.
    pub(crate) fn prepare(&self, context: CapsuleContext, enabled: bool) {
        *self
            .prepared
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some((context, enabled));
    }

    /// What `prepare` left for the recording that is beginning, once.
    pub(crate) fn take_prepared(&self) -> Option<(CapsuleContext, bool)> {
        self.prepared
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    /// A recording begins, and the last one's pending hide and error are
    /// forgotten. With the indicator on, the window comes up; callers do that
    /// before they announce the recording, since the announcement is what
    /// starts the capsule's fade-in. With it off, nothing shows, and an ending
    /// that is still up from the last recording goes now. The setting is read
    /// once per recording, so a change takes effect from the next one.
    pub(crate) fn recording_started<W: CapsuleWindow>(
        &self,
        window: &W,
        context: CapsuleContext,
        enabled: bool,
    ) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.hide_pending.store(false, Ordering::SeqCst);
        self.had_error.store(false, Ordering::SeqCst);
        if enabled {
            self.show(window, &context);
        } else if self.visible.swap(false, Ordering::SeqCst) {
            hide_window(window);
        }
    }

    /// The session reported an error, so its ending stays up as long as an
    /// error does.
    pub(crate) fn error_reported(&self) {
        self.had_error.store(true, Ordering::SeqCst);
    }

    /// The pipeline entered `state`. The ending of a session starts the clock
    /// on hiding the window.
    pub(crate) fn state_changed<W: CapsuleWindow>(&self, window: &W, state: PipelineState) {
        if !self.visible.load(Ordering::SeqCst) {
            return;
        }
        if let Some(delay) = hide_delay(state, self.had_error.load(Ordering::SeqCst)) {
            self.hide_after(window, self.generation.load(Ordering::SeqCst), delay);
        }
    }

    /// The hotkey or the menu bar could not start or stop a recording, so the
    /// pipeline has nothing to say about it. The capsule comes up to show the
    /// error for a few seconds, taking over from an ending that was only
    /// waiting to go. One that is up for a recording under way is left alone:
    /// that recording's own states schedule its hide.
    pub(crate) fn toggle_failed<W: CapsuleWindow>(
        &self,
        window: &W,
        context: CapsuleContext,
        enabled: bool,
    ) {
        self.had_error.store(true, Ordering::SeqCst);
        let recording_under_way =
            self.visible.load(Ordering::SeqCst) && !self.hide_pending.load(Ordering::SeqCst);
        if !enabled || recording_under_way {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.show(window, &context);
        self.hide_after(window, generation, ERROR_VISIBLE);
    }

    fn show<W: CapsuleWindow>(&self, window: &W, context: &CapsuleContext) {
        if let Err(error) = window.present(context) {
            tracing::warn!("Failed to show the recording capsule: {error}");
        }
        // Marked even when the window did not come up cleanly: a hide has to
        // follow either way, and hiding a window that is not up does no harm.
        self.visible.store(true, Ordering::SeqCst);
    }

    /// Hides the window after `delay`, unless a later recording or failed
    /// toggle has taken over from `generation` by then.
    fn hide_after<W: CapsuleWindow>(&self, window: &W, generation: u64, delay: Duration) {
        self.hide_pending.store(true, Ordering::SeqCst);
        // Counted from now, not from whenever the task first gets to run.
        let deadline = tokio::time::Instant::now() + delay;
        let current = Arc::clone(&self.generation);
        let visible = Arc::clone(&self.visible);
        let window = window.clone();
        tokio::spawn(async move {
            tokio::time::sleep_until(deadline).await;
            // `visible` belongs to the newer generation by now, so it is left alone.
            if current.load(Ordering::SeqCst) != generation {
                return;
            }
            // The session's last two states can each schedule a hide: the
            // first to run does it, and the other finds nothing left to do.
            if visible.swap(false, Ordering::SeqCst) {
                hide_window(&window);
            }
        });
    }
}

fn hide_window<W: CapsuleWindow>(window: &W) {
    if let Err(error) = window.hide() {
        tracing::warn!("Failed to hide the recording capsule: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> tauri::PhysicalRect<i32, u32> {
        tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(x, y),
            size: tauri::PhysicalSize::new(w, h),
        }
    }

    #[test]
    fn capsule_origin_centres_on_a_retina_work_area() {
        assert_eq!(
            capsule_origin(&rect(0, 50, 2880, 1640), 2.0),
            tauri::LogicalPosition::new(500.0, 765.0)
        );
    }

    #[test]
    fn capsule_origin_handles_a_display_left_of_the_primary() {
        assert_eq!(
            capsule_origin(&rect(-1920, 0, 1920, 1050), 1.0),
            tauri::LogicalPosition::new(-1180.0, 970.0)
        );
    }

    #[test]
    fn capsule_origin_handles_a_fractional_scale() {
        assert_eq!(
            capsule_origin(&rect(2700, -150, 2880, 1800), 1.5),
            tauri::LogicalPosition::new(2540.0, 1020.0)
        );
    }

    #[test]
    fn the_pointer_is_brought_back_to_points() {
        assert_eq!(
            pointer_in_points(tauri::PhysicalPosition::new(2000.0, 1600.0), 2.0),
            (1000.0, 800.0)
        );
    }

    #[test]
    fn hide_delay_follows_the_spec() {
        assert_eq!(
            hide_delay(PipelineState::Done, false),
            Some(Duration::from_millis(1500))
        );
        assert_eq!(
            hide_delay(PipelineState::Done, true),
            Some(Duration::from_millis(4000))
        );
        assert_eq!(
            hide_delay(PipelineState::Error, false),
            Some(Duration::from_millis(4000))
        );
        assert_eq!(
            hide_delay(PipelineState::Idle, false),
            Some(Duration::from_millis(1000))
        );
        assert_eq!(
            hide_delay(PipelineState::Idle, true),
            Some(Duration::from_millis(4000))
        );
        for state in [
            PipelineState::Recording,
            PipelineState::Transcribing,
            PipelineState::Processing,
        ] {
            assert_eq!(hide_delay(state, false), None);
        }
    }

    /// A window that only writes down what it was asked to do.
    #[derive(Clone, Default)]
    struct FakeWindow {
        calls: Arc<Mutex<Vec<&'static str>>>,
    }

    impl FakeWindow {
        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CapsuleWindow for FakeWindow {
        fn present(&self, _context: &CapsuleContext) -> Result<(), String> {
            self.calls.lock().unwrap().push("present");
            Ok(())
        }

        fn hide(&self) -> Result<(), String> {
            self.calls.lock().unwrap().push("hide");
            Ok(())
        }
    }

    fn ctx() -> CapsuleContext {
        CapsuleContext {
            shortcut: "Ctrl+`".into(),
            output_mode: OutputMode::Clipboard,
            save_history: true,
        }
    }

    /// Moves the paused clock and lets the hide task run.
    async fn advance(millis: u64) {
        tokio::time::advance(Duration::from_millis(millis)).await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn the_capsule_hides_after_a_finished_session() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        advance(1499).await;
        assert_eq!(window.calls(), ["present"]);
        advance(1).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_recording_cancels_the_pending_hide() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        advance(1000).await;
        capsule.recording_started(&window, ctx(), true);
        advance(1000).await;
        assert!(!window.calls().contains(&"hide"), "{:?}", window.calls());
    }

    #[tokio::test(start_paused = true)]
    async fn an_error_in_the_session_keeps_the_result_up_for_four_seconds() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.error_reported();
        capsule.state_changed(&window, PipelineState::Done);

        advance(3999).await;
        assert!(!window.calls().contains(&"hide"), "{:?}", window.calls());
        advance(1).await;
        assert!(window.calls().contains(&"hide"), "{:?}", window.calls());
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancel_after_an_error_keeps_the_error_up_for_four_seconds() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.error_reported();
        capsule.state_changed(&window, PipelineState::Idle);

        advance(3999).await;
        assert_eq!(window.calls(), ["present"]);
        advance(1).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    #[test]
    fn a_recording_takes_the_settings_prepared_for_it_once() {
        let capsule = Capsule::default();
        assert!(capsule.take_prepared().is_none());

        // A start that failed before its recording began leaves its settings
        // behind; the next start replaces them.
        capsule.prepare(ctx(), true);
        let both = CapsuleContext {
            output_mode: OutputMode::Both,
            ..ctx()
        };
        capsule.prepare(both.clone(), false);

        let (context, enabled) = capsule.take_prepared().unwrap();
        assert_eq!(
            serde_json::to_value(context).unwrap(),
            serde_json::to_value(both).unwrap()
        );
        assert!(!enabled);
        assert!(capsule.take_prepared().is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn an_error_does_not_follow_the_next_recording() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.error_reported();
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        advance(1500).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_indicator_switch_keeps_the_capsule_hidden() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), false);
        capsule.state_changed(&window, PipelineState::Done);

        advance(5000).await;
        assert!(window.calls().is_empty(), "{:?}", window.calls());
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_toggle_shows_the_capsule_when_hidden() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.toggle_failed(&window, ctx(), true);
        assert_eq!(window.calls(), ["present"]);

        advance(4000).await;
        assert_eq!(window.calls(), ["present", "hide"]);

        // With a recording under way the capsule is up already, and stays the one it was.
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.toggle_failed(&window, ctx(), true);
        assert_eq!(window.calls(), ["present"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_toggle_stays_silent_when_the_indicator_is_off() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.toggle_failed(&window, ctx(), false);

        advance(5000).await;
        assert!(window.calls().is_empty(), "{:?}", window.calls());
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_toggle_after_a_finished_session_stays_up_four_seconds() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        // The result is on its way out, so the failure takes the capsule over,
        // and the hide the result scheduled must not fire in its place.
        advance(1000).await;
        capsule.toggle_failed(&window, ctx(), true);
        advance(1000).await;
        assert!(!window.calls().contains(&"hide"), "{:?}", window.calls());

        advance(2999).await;
        assert!(!window.calls().contains(&"hide"), "{:?}", window.calls());
        advance(1).await;
        assert_eq!(window.calls(), ["present", "present", "hide"]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_toggle_during_the_next_recording_leaves_it_alone() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        // The next recording replaces the result, so no hide is pending any more.
        advance(500).await;
        capsule.recording_started(&window, ctx(), true);
        capsule.toggle_failed(&window, ctx(), true);
        assert_eq!(window.calls(), ["present", "present"]);
    }

    #[tokio::test(start_paused = true)]
    async fn turning_the_indicator_off_hides_a_lingering_capsule() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);

        advance(500).await;
        capsule.recording_started(&window, ctx(), false);
        assert_eq!(window.calls(), ["present", "hide"]);

        // Neither the hide the result had scheduled nor the end of this
        // recording, with nothing on screen, hides it again.
        capsule.state_changed(&window, PipelineState::Done);
        advance(5000).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    #[tokio::test(start_paused = true)]
    async fn the_last_two_states_of_a_session_hide_the_window_once() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        capsule.recording_started(&window, ctx(), true);
        capsule.state_changed(&window, PipelineState::Done);
        capsule.state_changed(&window, PipelineState::Idle);

        advance(5000).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    #[test]
    fn the_context_reaches_the_frontend_as_the_capsule_reads_it() {
        let config = AppConfig {
            hotkey: "Cmd+Shift+Space".into(),
            output_mode: OutputMode::Both,
            save_history: false,
            ..AppConfig::default()
        };
        assert_eq!(
            serde_json::to_value(CapsuleContext::from_config(&config)).unwrap(),
            serde_json::json!({
                "shortcut": "Cmd+Shift+Space",
                "output_mode": "both",
                "save_history": false,
            })
        );
    }

    #[test]
    fn a_failed_toggle_follows_the_config_that_was_read() {
        let config = AppConfig {
            hotkey: "Cmd+Shift+Space".into(),
            output_mode: OutputMode::Both,
            save_history: false,
            show_recording_indicator: false,
            ..AppConfig::default()
        };
        let (context, enabled) = failure_context(Ok(&config));
        assert!(!enabled);
        assert_eq!(
            serde_json::to_value(context).unwrap(),
            serde_json::json!({
                "shortcut": "Cmd+Shift+Space",
                "output_mode": "both",
                "save_history": false,
            })
        );

        let config = AppConfig {
            show_recording_indicator: true,
            ..config
        };
        assert!(failure_context(Ok(&config)).1);
    }

    #[test]
    fn a_failed_toggle_with_an_unreadable_config_shows_the_defaults() {
        let (context, enabled) =
            failure_context(Err("Failed to load config.toml: TOML parse error"));
        assert!(enabled);
        assert_eq!(
            serde_json::to_value(context).unwrap(),
            serde_json::to_value(CapsuleContext::from_config(&AppConfig::default())).unwrap()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_toggle_shows_the_capsule_when_the_config_cannot_be_read() {
        let (capsule, window) = (Capsule::default(), FakeWindow::default());
        let (context, enabled) =
            failure_context(Err("Failed to load config.toml: TOML parse error"));
        capsule.toggle_failed(&window, context, enabled);
        assert_eq!(window.calls(), ["present"]);

        advance(4000).await;
        assert_eq!(window.calls(), ["present", "hide"]);
    }

    /// These keys are what keep the capsule from taking focus from the app
    /// being typed into or getting in the way of a click, so an edit to any of
    /// them has to be a deliberate one.
    #[test]
    fn the_capsule_window_is_declared_to_never_take_focus() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let window = config["app"]["windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|window| window["label"] == CAPSULE_WINDOW)
            .expect("the capsule window must be declared");

        assert_eq!(window["width"], CAPSULE_WIDTH);
        assert_eq!(window["height"], CAPSULE_HEIGHT);
        for (key, expected) in [
            ("focusable", false),
            ("focus", false),
            ("visible", false),
            ("decorations", false),
            ("resizable", false),
            ("shadow", false),
            ("transparent", true),
            ("alwaysOnTop", true),
            ("visibleOnAllWorkspaces", true),
            ("skipTaskbar", true),
        ] {
            assert_eq!(window[key], expected, "{key}");
        }
        // `center` would put the window mid-screen whenever it was shown.
        assert!(window.get("center").is_none());
    }
}
