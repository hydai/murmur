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
        let startup = tokio::time::timeout(Duration::from_secs(30), stt.start_session()).await;
        let startup =
            startup.unwrap_or_else(|_| Err(MurmurError::Stt("STT startup timed out".into())));
        if let Err(error) = startup {
            self.fail_start(&error).await;
            return Err(error);
        }
        let AudioInput {
            capture,
            chunks: mut chunk_rx,
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
        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        *self.cancel_tx.lock().await = Some(cancel_tx.clone());
        let failed = Arc::new(AtomicBool::new(false));

        // Subscribe to transcription events
        let mut event_rx = stt.subscribe_events().await;
        let event_tx = self.event_tx.clone();
        // Clone the processor under a read lock so the current recording
        // uses a snapshot; hot-swaps take effect on the next recording.
        let llm_processor = self.llm_processor.read().await.clone();
        let output_sink = self.output_sink.read().await.clone();
        let chinese_conversion = *self.chinese_conversion.read().await;
        let dictionary = self.dictionary.clone();
        let state_arc = self.state.clone();
        let capture_arc = self.audio_capture.clone();
        let transcription_failed = failed.clone();

        // Spawn transcription event handler
        let transcription_task = tokio::spawn(async move {
            let mut full_transcription = String::new();
            let mut last_partial_text = String::new();
            let mut last_timestamp = 0u64;

            while let Some(event) = event_rx.recv().await {
                match &event {
                    TranscriptionEvent::Partial { text, timestamp_ms } => {
                        tracing::debug!(chars = text.chars().count(), "Partial transcript");
                        let _ = event_tx.send(PipelineEvent::PartialTranscription {
                            text: text.clone(),
                            timestamp_ms: *timestamp_ms,
                        });
                        last_timestamp = *timestamp_ms;

                        // Track latest partial for fallback (Apple STT only sends partials)
                        if !text.is_empty() {
                            last_partial_text = text.clone();
                        }

                        // Transition to Transcribing if we have text
                        if !text.is_empty() {
                            let mut state = state_arc.lock().await;
                            if *state == PipelineState::Recording {
                                *state = PipelineState::Transcribing;
                                let _ = event_tx.send(PipelineEvent::StateChanged {
                                    state: PipelineState::Transcribing,
                                    timestamp_ms: last_timestamp,
                                });
                            }
                        }
                    }
                    TranscriptionEvent::Committed { text, timestamp_ms } => {
                        tracing::info!(chars = text.chars().count(), "Committed transcript");
                        let _ = event_tx.send(PipelineEvent::CommittedTranscription {
                            text: text.clone(),
                            timestamp_ms: *timestamp_ms,
                        });

                        // Accumulate transcription
                        if !full_transcription.is_empty() {
                            full_transcription.push(' ');
                        }
                        full_transcription.push_str(text);
                        last_timestamp = *timestamp_ms;

                        // Reset partial tracker so it only holds text
                        // from partials AFTER this commit (the uncommitted tail)
                        last_partial_text.clear();
                    }
                    TranscriptionEvent::Error { message } => {
                        transcription_failed.store(true, Ordering::SeqCst);
                        tracing::error!("STT error: {}", message);
                        let _ = event_tx.send(PipelineEvent::Error {
                            message: message.clone(),
                            recoverable: false,
                        });
                        break; // Exit loop — let post-processing run or transition to Idle
                    }
                }
            }

            // A failed provider may still be finalizing. Closing its receiver
            // makes callbacks fail promptly instead of blocking during the LLM.
            drop(event_rx);

            // Ending transcription (including provider failure) ends capture.
            // Do this before emitting any terminal state or writing output.
            let _ = stop_capture(&capture_arc).await;
            let _ = cancel_tx.send(true);

            // Append any uncommitted trailing partial text.
            // Covers two cases:
            // 1. No commits at all (e.g. Apple STT only sent partials) — partial becomes the full text
            // 2. Commits + trailing partials — appends the uncommitted tail after the last commit
            if !last_partial_text.is_empty() {
                tracing::info!(
                    "Appending trailing partial text ({} chars, had_commits={})",
                    last_partial_text.len(),
                    !full_transcription.is_empty()
                );
                if !full_transcription.is_empty() {
                    full_transcription.push(' ');
                }
                full_transcription.push_str(&last_partial_text);
            }

            // When transcription finishes (channel closed), trigger LLM processing
            if !full_transcription.is_empty() {
                tracing::info!("Transcription complete, detecting voice commands");

                // Get dictionary terms
                let dictionary_terms = {
                    let dict = dictionary.lock().await;
                    dict.get_terms()
                };

                // Detect voice commands in the transcription
                let detection = detect_command(&full_transcription, dictionary_terms);

                // Emit command detection event
                let _ = event_tx.send(PipelineEvent::CommandDetected {
                    command_name: detection.command_name.clone(),
                    timestamp_ms: last_timestamp,
                });

                if let Some(ref cmd) = detection.command_name {
                    tracing::info!("Voice command detected: {}", cmd);
                } else {
                    tracing::info!("No voice command detected, using default post-processing");
                }

                // Transition to Processing state
                {
                    let mut state = state_arc.lock().await;
                    *state = PipelineState::Processing;
                }
                let _ = event_tx.send(PipelineEvent::StateChanged {
                    state: PipelineState::Processing,
                    timestamp_ms: last_timestamp,
                });

                let task = detection.task;
                let translate_target = match &task {
                    ProcessingTask::Translate {
                        target_language, ..
                    } => Some(target_language.clone()),
                    _ => None,
                };

                tracing::info!(
                    "Starting LLM post-processing: input_len={} chars",
                    full_transcription.chars().count()
                );

                let start_time = std::time::Instant::now();

                match llm_processor.process(task).await {
                    Ok(output) => {
                        let final_text = finalize_output(
                            &output.text,
                            chinese_conversion,
                            translate_target.as_deref(),
                        );
                        tracing::info!(
                            "LLM processing successful (took {}ms, output_len={} chars)",
                            output.processing_time_ms,
                            final_text.chars().count()
                        );

                        // Output to clipboard/keyboard
                        if let Err(e) = output_sink.output_text(&final_text).await {
                            tracing::error!("Failed to output text: {}", e);
                            let _ = event_tx.send(PipelineEvent::Error {
                                message: format!("Output failed: {}", e),
                                recoverable: true,
                            });
                        }

                        // Emit final result
                        let _ = event_tx.send(PipelineEvent::FinalResult {
                            text: final_text,
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                        });

                        // Transition to Done state
                        {
                            let mut state = state_arc.lock().await;
                            *state = if transcription_failed.load(Ordering::SeqCst) {
                                PipelineState::Error
                            } else {
                                PipelineState::Done
                            };
                        }
                        let _ = event_tx.send(PipelineEvent::StateChanged {
                            state: if transcription_failed.load(Ordering::SeqCst) {
                                PipelineState::Error
                            } else {
                                PipelineState::Done
                            },
                            timestamp_ms: last_timestamp,
                        });
                    }
                    Err(e) => {
                        tracing::error!("LLM processing failed: {}", e);
                        let fallback_text = finalize_output(
                            &full_transcription,
                            chinese_conversion,
                            translate_target.as_deref(),
                        );

                        // Emit error but try to output raw transcription
                        let _ = event_tx.send(PipelineEvent::Error {
                            message: format!(
                                "LLM processing failed: {}. Using raw transcription.",
                                e
                            ),
                            recoverable: true,
                        });

                        // Output raw transcription as fallback
                        if let Err(e) = output_sink.output_text(&fallback_text).await {
                            tracing::error!("Failed to output raw transcription: {}", e);
                        }

                        // Emit raw transcription as final result
                        let _ = event_tx.send(PipelineEvent::FinalResult {
                            text: fallback_text,
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                        });

                        // Transition to Error state
                        {
                            let mut state = state_arc.lock().await;
                            *state = PipelineState::Error;
                        }
                        let _ = event_tx.send(PipelineEvent::StateChanged {
                            state: PipelineState::Error,
                            timestamp_ms: last_timestamp,
                        });
                    }
                }
            } else {
                tracing::info!("No transcription to process");

                // Transition back to Idle
                {
                    let mut state = state_arc.lock().await;
                    *state = if transcription_failed.load(Ordering::SeqCst) {
                        PipelineState::Error
                    } else {
                        PipelineState::Idle
                    };
                }
                let _ = event_tx.send(PipelineEvent::StateChanged {
                    state: if transcription_failed.load(Ordering::SeqCst) {
                        PipelineState::Error
                    } else {
                        PipelineState::Idle
                    },
                    timestamp_ms: last_timestamp,
                });
            }

            tracing::debug!("Transcription task finished");
        });

        *self.transcription_task.lock().await = Some(transcription_task);

        // Subscribe to audio levels for waveform
        if let Some(mut level_rx) = levels {
            let event_tx = self.event_tx.clone();

            let level_task = tokio::spawn(async move {
                while let Some(level) = level_rx.recv().await {
                    let _ = event_tx.send(PipelineEvent::AudioLevel {
                        rms: level.rms,
                        voice_active: level.voice_active,
                        timestamp_ms: level.timestamp_ms,
                    });
                }
                tracing::debug!("Audio level task finished");
            });

            *self.level_task.lock().await = Some(level_task);
        }

        let capture_arc = self.audio_capture.clone();
        let event_tx = self.event_tx.clone();
        let audio_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_rx.changed() => break,
                    chunk = chunk_rx.recv() => {
                        let Some(chunk) = chunk else { break; };
                        let sent = tokio::select! {
                            biased;
                            _ = cancel_rx.changed() => break,
                            result = tokio::time::timeout(Duration::from_secs(30), stt.send_audio(chunk)) => result,
                        };
                        if !matches!(sent, Ok(Ok(()))) {
                            failed.store(true, Ordering::SeqCst);
                            let _ = event_tx.send(PipelineEvent::Error {
                                message: "STT audio delivery failed or timed out".into(),
                                recoverable: false,
                            });
                            let _ = stop_capture(&capture_arc).await;
                            break;
                        }
                    }
                }
            }
            // Providers own cleanup when this future is cancelled. A stalled
            // shutdown cannot retain the transcription event channel forever.
            // Allow a full HTTP backlog (4 queued + in-flight + final buffer)
            // to drain, with at most 30 seconds per request.
            if !matches!(
                tokio::time::timeout(Duration::from_secs(180), stt.stop_session()).await,
                Ok(Ok(()))
            ) {
                failed.store(true, Ordering::SeqCst);
                let _ = event_tx.send(PipelineEvent::Error {
                    message: "STT shutdown failed or timed out".into(),
                    recoverable: false,
                });
            }
        });
        *self.audio_task.lock().await = Some(audio_task);

        tracing::info!("Pipeline started successfully");
        Ok(())
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
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

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

    fn pipeline(
        output: Arc<TestOutput>,
        llm: Arc<TestLlm>,
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
