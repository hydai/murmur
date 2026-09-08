use lt_audio::AudioCapture;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::LlmProcessor;
use lt_core::output::OutputSink;
use lt_core::stt::{AudioChunk, SttProvider, TranscriptionEvent};
use lt_core::PersonalDictionary;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, watch, Mutex, RwLock};
use tokio::task::JoinHandle;

use crate::commands::detect_command;
use crate::state::{PipelineEvent, PipelineState};
use crate::text_normalization::finalize_output;
use lt_core::config::ChineseConversion;
use lt_core::llm::ProcessingTask;

/// Provider handshake budget before a recording is abandoned.
const STT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Per-chunk delivery budget.
const STT_SEND_TIMEOUT: Duration = Duration::from_secs(30);
/// Long enough for a full HTTP backlog (4 queued + in-flight + final buffer)
/// to drain at 30 seconds per request.
const STT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(180);

/// Pipeline orchestrator coordinating the full flow
pub struct PipelineOrchestrator {
    audio_capture: Arc<Mutex<Option<Box<dyn CaptureControl>>>>,
    capture_factory: Arc<dyn Fn() -> lt_audio::Result<AudioInput> + Send + Sync>,
    lifecycle: Mutex<()>,
    cancel_tx: Mutex<Option<watch::Sender<bool>>>,
    llm_processor: Arc<RwLock<Arc<dyn LlmProcessor>>>,
    output_sink: Arc<RwLock<Arc<dyn OutputSink>>>,
    chinese_conversion: Arc<RwLock<ChineseConversion>>,
    dictionary: Arc<Mutex<PersonalDictionary>>,
    state: Arc<Mutex<PipelineState>>,
    event_tx: broadcast::Sender<PipelineEvent>,
    // Task handles
    level_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    audio_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    transcription_task: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl PipelineOrchestrator {
    /// Create a new pipeline orchestrator
    pub fn new(
        llm_processor: Arc<dyn LlmProcessor>,
        output_sink: Arc<dyn OutputSink>,
        dictionary: Arc<Mutex<PersonalDictionary>>,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(100);

        Self {
            audio_capture: Arc::new(Mutex::new(None)),
            capture_factory: Arc::new(AudioInput::open),
            lifecycle: Mutex::new(()),
            cancel_tx: Mutex::new(None),
            llm_processor: Arc::new(RwLock::new(llm_processor)),
            output_sink: Arc::new(RwLock::new(output_sink)),
            chinese_conversion: Arc::new(RwLock::new(ChineseConversion::default())),
            dictionary,
            state: Arc::new(Mutex::new(PipelineState::Idle)),
            event_tx,
            level_task: Arc::new(Mutex::new(None)),
            audio_task: Arc::new(Mutex::new(None)),
            transcription_task: Arc::new(Mutex::new(None)),
        }
    }

    /// Subscribe to pipeline events
    /// Creates a new receiver that will receive all pipeline events
    pub fn subscribe_events(&self) -> broadcast::Receiver<PipelineEvent> {
        self.event_tx.subscribe()
    }

    /// Get current pipeline state
    pub async fn get_state(&self) -> PipelineState {
        *self.state.lock().await
    }

    /// Whether the microphone is still open. The state alone cannot tell a
    /// live recording from a session that is finishing after `stop()`.
    pub async fn is_capturing(&self) -> bool {
        self.audio_capture.lock().await.is_some()
    }

    /// Get reference to the dictionary for updates
    pub fn get_dictionary(&self) -> Arc<Mutex<PersonalDictionary>> {
        self.dictionary.clone()
    }

    /// Hot-swap the LLM processor. Takes effect on the next recording.
    pub async fn set_llm_processor(&self, processor: Arc<dyn LlmProcessor>) {
        let mut guard = self.llm_processor.write().await;
        *guard = processor;
        tracing::info!("LLM processor hot-swapped (takes effect on next recording)");
    }

    /// Hot-swap the output destination for the next recording.
    pub async fn set_output_sink(&self, sink: Arc<dyn OutputSink>) {
        *self.output_sink.write().await = sink;
    }

    /// Hot-swap the Chinese conversion for the next recording.
    pub async fn set_chinese_conversion(&self, conversion: ChineseConversion) {
        *self.chinese_conversion.write().await = conversion;
    }

    /// Start the pipeline with the provided STT provider.
    ///
    /// Drive this future to completion (callers spawn it on detached tasks):
    /// a startup failure rolls the state back, but dropping the future while
    /// provider startup is pending would leave it in `Recording`.
    pub async fn start(&self, stt_provider: Box<dyn SttProvider>) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        let state = self.state.lock().await;

        match *state {
            PipelineState::Recording | PipelineState::Transcribing | PipelineState::Processing => {
                return Err(MurmurError::InvalidState(format!(
                    "Cannot start pipeline in {:?} state",
                    *state
                )));
            }
            _ => {}
        }
        drop(state);
        self.cancel_session().await;
        *self.state.lock().await = PipelineState::Recording;
        self.emit_state_change(PipelineState::Recording);

        let mut stt = stt_provider;
        let startup = tokio::time::timeout(STT_STARTUP_TIMEOUT, stt.start_session()).await;
        let startup =
            startup.unwrap_or_else(|_| Err(MurmurError::Stt("STT startup timed out".into())));
        if let Err(error) = startup {
            self.fail_start(&error).await;
            return Err(error);
        }

        let AudioInput {
            capture,
            chunks,
            levels,
        } = match (self.capture_factory)() {
            Ok(input) => input,
            Err(error) => {
                // Drop owns provider cleanup; do not leave an active session on
                // a device/permission failure after the network connected.
                let error = MurmurError::Audio(error.to_string());
                self.fail_start(&error).await;
                return Err(error);
            }
        };
        *self.audio_capture.lock().await = Some(capture);

        let (cancel_tx, cancel_rx) = watch::channel(false);
        *self.cancel_tx.lock().await = Some(cancel_tx.clone());
        let stt_events = stt.subscribe_events().await;
        let session = self.session_snapshot().await;

        *self.transcription_task.lock().await = Some(tokio::spawn(run_transcription(
            session.clone(),
            stt_events,
            cancel_tx,
        )));
        if let Some(levels) = levels {
            *self.level_task.lock().await =
                Some(tokio::spawn(forward_levels(self.event_tx.clone(), levels)));
        }
        *self.audio_task.lock().await =
            Some(tokio::spawn(pump_audio(session, stt, chunks, cancel_rx)));

        tracing::info!("Pipeline started successfully");
        Ok(())
    }

    /// Freeze the configuration this recording runs against.
    ///
    /// Everything is read under its lock once, here, so a hot-swap of the LLM
    /// processor, the output sink or the conversion setting takes effect on the
    /// next recording rather than partway through this one.
    async fn session_snapshot(&self) -> Session {
        Session {
            events: self.event_tx.clone(),
            state: self.state.clone(),
            capture: self.audio_capture.clone(),
            dictionary: self.dictionary.clone(),
            llm: self.llm_processor.read().await.clone(),
            output: self.output_sink.read().await.clone(),
            chinese_conversion: *self.chinese_conversion.read().await,
            failed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Stop capture and let queued transcription and output finish normally.
    pub async fn stop(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        let result = stop_capture(&self.audio_capture).await;
        if let Some(task) = self.level_task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
        result
    }

    /// Cancel the session and return to Idle without later output from old tasks.
    pub async fn reset(&self) -> Result<()> {
        let _lifecycle = self.lifecycle.lock().await;
        self.cancel_session().await;
        *self.state.lock().await = PipelineState::Idle;
        self.emit_state_change(PipelineState::Idle);
        Ok(())
    }

    async fn fail_start(&self, error: &MurmurError) {
        *self.state.lock().await = PipelineState::Error;
        let _ = self.event_tx.send(PipelineEvent::Error {
            message: error.to_string(),
            recoverable: false,
        });
        self.emit_state_change(PipelineState::Error);
    }

    async fn cancel_session(&self) {
        if let Some(cancel) = self.cancel_tx.lock().await.take() {
            let _ = cancel.send(true);
        }
        let _ = stop_capture(&self.audio_capture).await;
        for slot in [&self.level_task, &self.transcription_task, &self.audio_task] {
            if let Some(task) = slot.lock().await.take() {
                task.abort();
                let _ = task.await;
            }
        }
    }

    fn emit_state_change(&self, state: PipelineState) {
        let timestamp_ms = lt_core::now_ms();

        let _ = self.event_tx.send(PipelineEvent::StateChanged {
            state,
            timestamp_ms,
        });
    }
}

trait CaptureControl: Send {
    fn stop(&mut self) -> lt_audio::Result<()>;
}

impl CaptureControl for AudioCapture {
    fn stop(&mut self) -> lt_audio::Result<()> {
        AudioCapture::stop(self)
    }
}

struct AudioInput {
    capture: Box<dyn CaptureControl>,
    chunks: mpsc::Receiver<AudioChunk>,
    levels: Option<mpsc::Receiver<lt_audio::AudioLevel>>,
}

impl AudioInput {
    fn open() -> lt_audio::Result<Self> {
        let mut capture = AudioCapture::new();
        capture.start()?;
        let chunks = capture
            .subscribe_chunks()
            .expect("new audio capture has a chunk receiver");
        let levels = capture.subscribe_levels();
        Ok(Self {
            capture: Box::new(capture),
            chunks,
            levels,
        })
    }
}

/// One recording's shared handles and frozen configuration.
///
/// Session tasks run detached and have no `&self`, so everything they touch is
/// gathered here instead of being cloned field by field into each spawn.
#[derive(Clone)]
struct Session {
    events: broadcast::Sender<PipelineEvent>,
    state: Arc<Mutex<PipelineState>>,
    capture: Arc<Mutex<Option<Box<dyn CaptureControl>>>>,
    dictionary: Arc<Mutex<PersonalDictionary>>,
    llm: Arc<dyn LlmProcessor>,
    output: Arc<dyn OutputSink>,
    chinese_conversion: ChineseConversion,
    /// Set by whichever task hits an unrecoverable problem; read once when
    /// choosing the terminal state.
    failed: Arc<AtomicBool>,
}

impl Session {
    fn emit(&self, event: PipelineEvent) {
        // No receiver is a closed UI, not a pipeline error.
        let _ = self.events.send(event);
    }

    async fn publish(&self, next: PipelineState, timestamp_ms: u64) {
        publish_state(&self.state, &self.events, next, timestamp_ms).await;
    }

    /// Record an unrecoverable failure and tell the UI.
    fn fail(&self, message: impl Into<String>) {
        self.failed.store(true, Ordering::SeqCst);
        self.emit(PipelineEvent::Error {
            message: message.into(),
            recoverable: false,
        });
    }

    /// The terminal state to settle on, downgraded to `Error` when any task
    /// failed. Read once: two reads either side of an await could store one
    /// state and announce another.
    fn terminal(&self, success: PipelineState) -> PipelineState {
        if self.failed.load(Ordering::SeqCst) {
            PipelineState::Error
        } else {
            success
        }
    }

    /// Write the final text out and settle the pipeline.
    ///
    /// A sink failure is reported but never withholds the result: the text
    /// stays available in the overlay even when delivery fails.
    async fn deliver(
        &self,
        text: String,
        elapsed_ms: u64,
        timestamp_ms: u64,
        terminal: PipelineState,
    ) {
        if let Err(error) = self.output.output_text(&text).await {
            tracing::error!("Failed to output text: {error}");
            self.emit(PipelineEvent::Error {
                message: format!("Output failed: {error}"),
                recoverable: true,
            });
        }
        self.emit(PipelineEvent::FinalResult {
            text,
            processing_time_ms: elapsed_ms,
        });
        self.publish(terminal, timestamp_ms).await;
    }
}

/// What the transcription event loop accumulated.
struct Transcript {
    text: String,
    last_timestamp_ms: u64,
}

/// Mirror STT events to the UI and accumulate the transcript, returning when
/// the provider stops sending or reports an error.
async fn collect_transcript(
    session: &Session,
    events: &mut mpsc::Receiver<TranscriptionEvent>,
) -> Transcript {
    let mut text = String::new();
    // Apple STT only sends partials, and a stream can end with an uncommitted
    // tail, so the latest partial is kept as a fallback until the next commit.
    let mut trailing_partial = String::new();
    let mut last_timestamp_ms = 0u64;

    while let Some(event) = events.recv().await {
        match &event {
            TranscriptionEvent::Partial {
                text: partial,
                timestamp_ms,
            } => {
                tracing::debug!(chars = partial.chars().count(), "Partial transcript");
                session.emit(PipelineEvent::PartialTranscription {
                    text: partial.clone(),
                    timestamp_ms: *timestamp_ms,
                });
                last_timestamp_ms = *timestamp_ms;

                if !partial.is_empty() {
                    trailing_partial = partial.clone();
                    // Compare-and-set: only the first non-empty partial moves
                    // the pipeline out of Recording.
                    let mut state = session.state.lock().await;
                    if *state == PipelineState::Recording {
                        *state = PipelineState::Transcribing;
                        session.emit(PipelineEvent::StateChanged {
                            state: PipelineState::Transcribing,
                            timestamp_ms: last_timestamp_ms,
                        });
                    }
                }
            }
            TranscriptionEvent::Committed {
                text: committed,
                timestamp_ms,
            } => {
                tracing::info!(chars = committed.chars().count(), "Committed transcript");
                session.emit(PipelineEvent::CommittedTranscription {
                    text: committed.clone(),
                    timestamp_ms: *timestamp_ms,
                });
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(committed);
                last_timestamp_ms = *timestamp_ms;
                // Anything kept so far is now committed; only partials after
                // this point are an uncommitted tail.
                trailing_partial.clear();
            }
            TranscriptionEvent::Error { message } => {
                tracing::error!("STT error: {message}");
                session.fail(message.clone());
                // Let post-processing run on whatever was transcribed.
                break;
            }
        }
    }

    if !trailing_partial.is_empty() {
        tracing::info!(
            "Appending trailing partial text ({} chars, had_commits={})",
            trailing_partial.len(),
            !text.is_empty()
        );
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&trailing_partial);
    }

    Transcript {
        text,
        last_timestamp_ms,
    }
}

/// Run the transcript through command detection and the LLM, then deliver it.
async fn post_process(session: &Session, transcript: Transcript) {
    tracing::info!("Transcription complete, detecting voice commands");
    let dictionary_terms = session.dictionary.lock().await.get_terms();
    let detection = detect_command(&transcript.text, dictionary_terms);

    session.emit(PipelineEvent::CommandDetected {
        command_name: detection.command_name.clone(),
        timestamp_ms: transcript.last_timestamp_ms,
    });
    match &detection.command_name {
        Some(command) => tracing::info!("Voice command detected: {command}"),
        None => tracing::info!("No voice command detected, using default post-processing"),
    }

    session
        .publish(PipelineState::Processing, transcript.last_timestamp_ms)
        .await;

    // On LLM failure the user gets the spoken content back, not the command
    // prefix that addressed the model.
    let fallback_content = detection.content;
    let task = detection.task;
    let translate_target = match &task {
        ProcessingTask::Translate {
            target_language, ..
        } => Some(target_language.clone()),
        _ => None,
    };

    tracing::info!(
        "Starting LLM post-processing: input_len={} chars",
        transcript.text.chars().count()
    );
    let started = std::time::Instant::now();

    match session.llm.process(task).await {
        Ok(output) => {
            let final_text = finalize_output(
                &output.text,
                session.chinese_conversion,
                translate_target.as_deref(),
            );
            tracing::info!(
                "LLM processing successful (took {}ms, output_len={} chars)",
                output.processing_time_ms,
                final_text.chars().count()
            );
            let terminal = session.terminal(PipelineState::Done);
            session
                .deliver(
                    final_text,
                    started.elapsed().as_millis() as u64,
                    transcript.last_timestamp_ms,
                    terminal,
                )
                .await;
        }
        Err(error) => {
            tracing::error!("LLM processing failed: {error}");
            session.emit(PipelineEvent::Error {
                message: format!("LLM processing failed: {error}. Using raw transcription."),
                recoverable: true,
            });
            let fallback = finalize_output(
                &fallback_content,
                session.chinese_conversion,
                translate_target.as_deref(),
            );
            session
                .deliver(
                    fallback,
                    started.elapsed().as_millis() as u64,
                    transcript.last_timestamp_ms,
                    PipelineState::Error,
                )
                .await;
        }
    }
}

/// Own the recording from the first STT event to the terminal state.
async fn run_transcription(
    session: Session,
    mut stt_events: mpsc::Receiver<TranscriptionEvent>,
    cancel_tx: watch::Sender<bool>,
) {
    let transcript = collect_transcript(&session, &mut stt_events).await;

    // A failed provider may still be finalizing. Closing its receiver makes
    // callbacks fail promptly instead of blocking during the LLM call.
    drop(stt_events);

    // Ending transcription (including provider failure) ends capture. Do this
    // before emitting any terminal state or writing output.
    let _ = stop_capture(&session.capture).await;
    let _ = cancel_tx.send(true);

    if transcript.text.is_empty() {
        tracing::info!("No transcription to process");
        let terminal = session.terminal(PipelineState::Idle);
        session
            .publish(terminal, transcript.last_timestamp_ms)
            .await;
    } else {
        post_process(&session, transcript).await;
    }

    tracing::debug!("Transcription task finished");
}

/// Mirror capture levels to the waveform until capture ends.
async fn forward_levels(
    events: broadcast::Sender<PipelineEvent>,
    mut levels: mpsc::Receiver<lt_audio::AudioLevel>,
) {
    while let Some(level) = levels.recv().await {
        let _ = events.send(PipelineEvent::AudioLevel {
            rms: level.rms,
            voice_active: level.voice_active,
            timestamp_ms: level.timestamp_ms,
        });
    }
    tracing::debug!("Audio level task finished");
}

/// Feed captured audio to the provider, then close the session.
async fn pump_audio(
    session: Session,
    mut stt: Box<dyn SttProvider>,
    mut chunks: mpsc::Receiver<AudioChunk>,
    mut cancel: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            _ = cancel.changed() => break,
            chunk = chunks.recv() => {
                let Some(chunk) = chunk else { break; };
                let sent = tokio::select! {
                    biased;
                    _ = cancel.changed() => break,
                    result = tokio::time::timeout(STT_SEND_TIMEOUT, stt.send_audio(chunk)) => result,
                };
                if !matches!(sent, Ok(Ok(()))) {
                    session.fail("STT audio delivery failed or timed out");
                    let _ = stop_capture(&session.capture).await;
                    break;
                }
            }
        }
    }

    // Providers own cleanup when this future is cancelled. A stalled shutdown
    // cannot retain the transcription event channel forever.
    if !matches!(
        tokio::time::timeout(STT_SHUTDOWN_TIMEOUT, stt.stop_session()).await,
        Ok(Ok(()))
    ) {
        session.fail("STT shutdown failed or timed out");
    }
}

/// Store a state and announce the same value. Session tasks have no `&self`,
/// so without this they re-implement `emit_state_change` inline; taking the
/// state by value keeps the stored state and the emitted event from being
/// computed separately and drifting apart.
async fn publish_state(
    state: &Mutex<PipelineState>,
    event_tx: &broadcast::Sender<PipelineEvent>,
    next: PipelineState,
    timestamp_ms: u64,
) {
    *state.lock().await = next;
    let _ = event_tx.send(PipelineEvent::StateChanged {
        state: next,
        timestamp_ms,
    });
}

async fn stop_capture(capture: &Mutex<Option<Box<dyn CaptureControl>>>) -> Result<()> {
    if let Some(mut capture) = capture.lock().await.take() {
        capture
            .stop()
            .map_err(|error| MurmurError::Audio(error.to_string()))?;
    }
    Ok(())
}

impl Drop for PipelineOrchestrator {
    fn drop(&mut self) {
        if let Ok(mut capture) = self.audio_capture.try_lock() {
            if let Some(mut capture) = capture.take() {
                let _ = capture.stop();
            }
        }
        for slot in [&self.level_task, &self.transcription_task, &self.audio_task] {
            if let Ok(mut slot) = slot.try_lock() {
                if let Some(task) = slot.take() {
                    task.abort();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lt_core::llm::{ProcessingOutput, ProcessingTask};
    use std::sync::Mutex as SyncMutex;
    use tokio::sync::Notify;

    struct TestLlm(Option<Arc<Notify>>);
    #[async_trait]
    impl LlmProcessor for TestLlm {
        async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
            if let Some(release) = &self.0 {
                release.notified().await;
            }
            let ProcessingTask::PostProcess { text, .. } = task else {
                panic!("unexpected task")
            };
            Ok(ProcessingOutput {
                text,
                processing_time_ms: 0,
                metadata: None,
            })
        }
        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }
    }

    #[derive(Default)]
    struct TestOutput(SyncMutex<Vec<String>>);
    #[async_trait]
    impl OutputSink for TestOutput {
        async fn output_text(&self, text: &str) -> Result<()> {
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    struct TestCapture {
        chunks: Option<mpsc::Sender<AudioChunk>>,
        running: Arc<AtomicBool>,
    }
    impl CaptureControl for TestCapture {
        fn stop(&mut self) -> lt_audio::Result<()> {
            self.running.store(false, Ordering::SeqCst);
            self.chunks.take();
            Ok(())
        }
    }

    struct TestStt {
        tx: Option<mpsc::Sender<TranscriptionEvent>>,
        rx: SyncMutex<Option<mpsc::Receiver<TranscriptionEvent>>>,
        fail_start: bool,
    }
    impl TestStt {
        fn new(fail_start: bool) -> (Box<Self>, mpsc::Sender<TranscriptionEvent>) {
            let (tx, rx) = mpsc::channel(16);
            (
                Box::new(Self {
                    tx: Some(tx.clone()),
                    rx: SyncMutex::new(Some(rx)),
                    fail_start,
                }),
                tx,
            )
        }
    }
    #[async_trait]
    impl SttProvider for TestStt {
        async fn start_session(&mut self) -> Result<()> {
            if self.fail_start {
                Err(MurmurError::Stt("startup failed".into()))
            } else {
                Ok(())
            }
        }
        async fn send_audio(&mut self, _: AudioChunk) -> Result<()> {
            Ok(())
        }
        async fn stop_session(&mut self) -> Result<()> {
            self.tx.take();
            Ok(())
        }
        async fn subscribe_events(&self) -> mpsc::Receiver<TranscriptionEvent> {
            self.rx.lock().unwrap().take().unwrap()
        }
    }

    struct FailingOutput;
    #[async_trait]
    impl OutputSink for FailingOutput {
        async fn output_text(&self, _: &str) -> Result<()> {
            Err(MurmurError::Output("sink is unavailable".into()))
        }
    }

    fn pipeline(
        output: Arc<TestOutput>,
        llm: Arc<dyn LlmProcessor>,
    ) -> (PipelineOrchestrator, Arc<AtomicBool>) {
        pipeline_with_sink(output, llm)
    }

    fn pipeline_with_sink(
        output: Arc<dyn OutputSink>,
        llm: Arc<dyn LlmProcessor>,
    ) -> (PipelineOrchestrator, Arc<AtomicBool>) {
        let running = Arc::new(AtomicBool::new(false));
        let status = running.clone();
        let mut pipeline =
            PipelineOrchestrator::new(llm, output, Arc::new(Mutex::new(PersonalDictionary::new())));
        pipeline.capture_factory = Arc::new(move || {
            let (tx, rx) = mpsc::channel(4);
            status.store(true, Ordering::SeqCst);
            Ok(AudioInput {
                capture: Box::new(TestCapture {
                    chunks: Some(tx),
                    running: status.clone(),
                }),
                chunks: rx,
                levels: None,
            })
        });
        (pipeline, running)
    }

    async fn wait_state(events: &mut broadcast::Receiver<PipelineEvent>, expected: PipelineState) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if matches!(events.recv().await.unwrap(), PipelineEvent::StateChanged { state, .. } if state == expected) { break; }
            }
        }).await.expect("expected pipeline state");
    }

    #[tokio::test]
    async fn publishing_a_state_stores_and_announces_the_same_value() {
        // The defect this guards against was a terminal state computed twice —
        // once for the store and once for the event — around an await, so a
        // concurrent flag change left `get_state` and the UI disagreeing.
        let state = Mutex::new(PipelineState::Recording);
        let (tx, mut rx) = broadcast::channel(4);

        for expected in [
            PipelineState::Processing,
            PipelineState::Error,
            PipelineState::Done,
            PipelineState::Idle,
        ] {
            publish_state(&state, &tx, expected, 7).await;
            assert_eq!(*state.lock().await, expected);
            let PipelineEvent::StateChanged {
                state,
                timestamp_ms,
            } = rx.recv().await.unwrap()
            else {
                panic!("expected a state change")
            };
            assert_eq!(state, expected);
            assert_eq!(timestamp_ms, 7);
        }
    }

    #[tokio::test]
    async fn failed_start_can_retry_and_reset_returns_idle() {
        let (p, running) = pipeline(Arc::default(), Arc::new(TestLlm(None)));
        let (failed, _) = TestStt::new(true);
        assert!(p.start(failed).await.is_err());
        assert_eq!(p.get_state().await, PipelineState::Error);
        assert!(!running.load(Ordering::SeqCst));
        let (working, _events) = TestStt::new(false);
        p.start(working).await.unwrap();
        assert!(running.load(Ordering::SeqCst));
        p.reset().await.unwrap();
        assert_eq!(p.get_state().await, PipelineState::Idle);
        assert!(!running.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn capture_start_failure_rolls_back_started_provider() {
        let (mut p, _) = pipeline(Arc::default(), Arc::new(TestLlm(None)));
        p.capture_factory = Arc::new(|| Err(lt_audio::AudioError::NoInputDevice));
        let (stt, events) = TestStt::new(false);
        assert!(p.start(stt).await.is_err());
        assert!(events.is_closed());
        assert_eq!(p.get_state().await, PipelineState::Error);
    }

    #[tokio::test]
    async fn terminal_stt_error_stops_capture_before_terminal_state() {
        let (p, running) = pipeline(Arc::default(), Arc::new(TestLlm(None)));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Error {
            message: "provider failed".into(),
        })
        .await
        .unwrap();
        wait_state(&mut events, PipelineState::Error).await;
        assert!(!running.load(Ordering::SeqCst));
        p.reset().await.unwrap();
    }

    #[tokio::test]
    async fn failed_transcription_closes_callbacks_before_slow_postprocessing() {
        let release = Arc::new(Notify::new());
        let (p, running) = pipeline(Arc::default(), Arc::new(TestLlm(Some(release))));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Partial {
            text: "preserved words".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        tx.send(TranscriptionEvent::Error {
            message: "provider failed".into(),
        })
        .await
        .unwrap();
        wait_state(&mut events, PipelineState::Processing).await;
        assert!(!running.load(Ordering::SeqCst));
        assert!(
            tx.is_closed(),
            "FFI callbacks must not block while LLM runs"
        );
        p.reset().await.unwrap();
    }

    #[tokio::test]
    async fn output_destination_is_snapshotted_per_recording() {
        let first = Arc::new(TestOutput::default());
        let second = Arc::new(TestOutput::default());
        let (p, _) = pipeline(first.clone(), Arc::new(TestLlm(None)));
        let mut events = p.subscribe_events();
        for (index, text) in ["first recording", "second recording"].iter().enumerate() {
            let (stt, tx) = TestStt::new(false);
            p.start(stt).await.unwrap();
            if index == 0 {
                p.set_output_sink(second.clone()).await;
            }
            tx.send(TranscriptionEvent::Committed {
                text: text.to_string(),
                timestamp_ms: 1,
            })
            .await
            .unwrap();
            drop(tx);
            p.stop().await.unwrap();
            wait_state(&mut events, PipelineState::Done).await;
        }
        assert_eq!(*first.0.lock().unwrap(), ["first recording"]);
        assert_eq!(*second.0.lock().unwrap(), ["second recording"]);
    }

    #[tokio::test]
    async fn is_capturing_tracks_the_live_capture_rather_than_the_state() {
        let release = Arc::new(Notify::new());
        let (p, _) = pipeline(Arc::default(), Arc::new(TestLlm(Some(release.clone()))));
        let mut events = p.subscribe_events();
        assert!(!p.is_capturing().await);
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        assert!(p.is_capturing().await);
        tx.send(TranscriptionEvent::Committed {
            text: "still processing".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        drop(tx);
        p.stop().await.unwrap();
        wait_state(&mut events, PipelineState::Processing).await;
        assert!(!p.is_capturing().await);
        assert_eq!(p.get_state().await, PipelineState::Processing);
        p.reset().await.unwrap();
        assert!(!p.is_capturing().await);
    }

    #[tokio::test]
    async fn chinese_conversion_setting_is_applied_to_the_final_output() {
        let output = Arc::new(TestOutput::default());
        let (p, _) = pipeline(output.clone(), Arc::new(TestLlm(None)));
        let mut events = p.subscribe_events();
        for (conversion, expected) in [
            (ChineseConversion::None, "软件会把数据复制到服务器。"),
            (ChineseConversion::Traditional, "軟體會把資料複製到伺服器。"),
        ] {
            p.set_chinese_conversion(conversion).await;
            let (stt, tx) = TestStt::new(false);
            p.start(stt).await.unwrap();
            tx.send(TranscriptionEvent::Committed {
                text: "软件会把数据复制到服务器。".into(),
                timestamp_ms: 1,
            })
            .await
            .unwrap();
            drop(tx);
            p.stop().await.unwrap();
            wait_state(&mut events, PipelineState::Done).await;
            assert_eq!(
                output.0.lock().unwrap().last().map(String::as_str),
                Some(expected)
            );
        }
    }

    struct SharedWriter(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Every test in this binary shares one capturing subscriber.
    fn captured_logs() -> Arc<std::sync::Mutex<Vec<u8>>> {
        static LOGS: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<u8>>>> =
            std::sync::OnceLock::new();
        LOGS.get_or_init(|| {
            let buffer = Arc::new(std::sync::Mutex::new(Vec::new()));
            let writer = buffer.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_ansi(false)
                .with_writer(move || SharedWriter(writer.clone()))
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("no other global subscriber in the test binary");
            buffer
        })
        .clone()
    }

    fn logs_text(logs: &Arc<std::sync::Mutex<Vec<u8>>>) -> String {
        String::from_utf8_lossy(&logs.lock().unwrap()).into_owned()
    }

    #[tokio::test]
    async fn transcript_content_never_reaches_the_logs() {
        let logs = captured_logs();
        let output = Arc::new(TestOutput::default());
        let (p, _) = pipeline(output.clone(), Arc::new(TestLlm(None)));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Partial {
            text: "partial-zebra-quartz".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        tx.send(TranscriptionEvent::Committed {
            text: "committed-zebra-quartz".into(),
            timestamp_ms: 2,
        })
        .await
        .unwrap();
        drop(tx);
        p.stop().await.unwrap();
        wait_state(&mut events, PipelineState::Done).await;
        assert_eq!(*output.0.lock().unwrap(), ["committed-zebra-quartz"]);

        let text = logs_text(&logs);
        assert!(
            text.contains("Committed transcript"),
            "logs were not captured:\n{text}"
        );
        assert!(
            !text.contains("zebra-quartz"),
            "transcript leaked into logs:\n{text}"
        );
    }

    struct FailingLlm;
    #[async_trait]
    impl LlmProcessor for FailingLlm {
        async fn process(&self, _: ProcessingTask) -> Result<ProcessingOutput> {
            Err(MurmurError::Llm("provider down".into()))
        }
        async fn health_check(&self) -> Result<bool> {
            Ok(false)
        }
    }

    #[tokio::test]
    async fn llm_failure_falls_back_to_the_content_without_the_command_prefix() {
        let output = Arc::new(TestOutput::default());
        let (p, _) = pipeline(output.clone(), Arc::new(FailingLlm));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Committed {
            text: "shorten this: hello world".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        drop(tx);
        p.stop().await.unwrap();
        wait_state(&mut events, PipelineState::Error).await;
        assert_eq!(*output.0.lock().unwrap(), ["hello world"]);
    }

    #[tokio::test]
    async fn a_failed_delivery_is_reported_on_the_fallback_path_too() {
        // The success arm emitted an Error event when the sink rejected the
        // text; the fallback arm only logged it, so a user whose clipboard
        // failed after an LLM failure saw nothing about the second problem.
        let (p, _) = pipeline_with_sink(Arc::new(FailingOutput), Arc::new(FailingLlm));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Committed {
            text: "hello world".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        drop(tx);
        p.stop().await.unwrap();

        let mut messages = Vec::new();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                match events.recv().await.unwrap() {
                    PipelineEvent::Error { message, .. } => messages.push(message),
                    PipelineEvent::StateChanged {
                        state: PipelineState::Error,
                        ..
                    } => break,
                    _ => {}
                }
            }
        })
        .await
        .expect("a terminal state");

        assert!(
            messages.iter().any(|m| m.contains("LLM processing failed")),
            "{messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("Output failed")),
            "{messages:?}"
        );
    }

    #[tokio::test]
    async fn reset_cancels_processing_without_late_output() {
        let output = Arc::new(TestOutput::default());
        let release = Arc::new(Notify::new());
        let (p, _) = pipeline(output.clone(), Arc::new(TestLlm(Some(release.clone()))));
        let mut events = p.subscribe_events();
        let (stt, tx) = TestStt::new(false);
        p.start(stt).await.unwrap();
        tx.send(TranscriptionEvent::Committed {
            text: "cancel me".into(),
            timestamp_ms: 1,
        })
        .await
        .unwrap();
        drop(tx);
        p.stop().await.unwrap();
        wait_state(&mut events, PipelineState::Processing).await;
        p.reset().await.unwrap();
        release.notify_one();
        assert_eq!(p.get_state().await, PipelineState::Idle);
        assert!(output.0.lock().unwrap().is_empty());
    }
}
