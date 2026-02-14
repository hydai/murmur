use lt_audio::AudioCapture;
use lt_core::error::{MurmurError, Result};
use lt_core::llm::LlmProcessor;
use lt_core::output::OutputSink;
use lt_core::stt::{SttProvider, TranscriptionEvent};
use lt_core::PersonalDictionary;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use crate::commands::detect_command;
use crate::state::{PipelineEvent, PipelineState};

/// Pipeline orchestrator coordinating the full flow
pub struct PipelineOrchestrator {
    audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    stt_provider: Arc<Mutex<Option<Box<dyn SttProvider>>>>,
    llm_processor: Arc<dyn LlmProcessor>,
    output_sink: Arc<dyn OutputSink>,
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
            stt_provider: Arc::new(Mutex::new(None)),
            llm_processor,
            output_sink,
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

    /// Get reference to the dictionary for updates
    pub fn get_dictionary(&self) -> Arc<Mutex<PersonalDictionary>> {
        self.dictionary.clone()
    }

    /// Start the pipeline with the provided STT provider
    pub async fn start(&self, stt_provider: Box<dyn SttProvider>) -> Result<()> {
        let mut state = self.state.lock().await;

        match *state {
            PipelineState::Recording | PipelineState::Transcribing | PipelineState::Processing => {
                return Err(MurmurError::InvalidState(format!(
                    "Cannot start pipeline in {:?} state",
                    *state
                )));
            }
            PipelineState::Done | PipelineState::Error => {
                // Reset from completed/error state to allow new recording
                *state = PipelineState::Idle;
            }
            PipelineState::Idle => {
                // Already idle, ready to start
            }
        }

        tracing::info!("Starting pipeline");

        // Transition to Recording state
        *state = PipelineState::Recording;
        self.emit_state_change(PipelineState::Recording);
        drop(state);

        // Store STT provider
        let mut stt_guard = self.stt_provider.lock().await;
        *stt_guard = Some(stt_provider);
        let mut stt = stt_guard.take().unwrap();
        drop(stt_guard);

        // Start STT session
        stt.start_session().await.map_err(|e| {
            tracing::error!("Failed to start STT session: {}", e);
            e
        })?;

        // Subscribe to transcription events
        let mut event_rx = stt.subscribe_events().await;
        let event_tx = self.event_tx.clone();
        let llm_processor = self.llm_processor.clone();
        let output_sink = self.output_sink.clone();
        let dictionary = self.dictionary.clone();
        let state_arc = self.state.clone();

        // Spawn transcription event handler
        let transcription_task = tokio::spawn(async move {
            let mut full_transcription = String::new();
            let mut last_partial_text = String::new();
            let mut last_timestamp = 0u64;

            while let Some(event) = event_rx.recv().await {
                match &event {
                    TranscriptionEvent::Partial { text, timestamp_ms } => {
                        tracing::debug!("Partial transcript: {}", text);
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
                        tracing::info!("Committed transcript: {}", text);
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
                    }
                    TranscriptionEvent::Error { message } => {
                        tracing::error!("STT error: {}", message);
                        let _ = event_tx.send(PipelineEvent::Error {
                            message: message.clone(),
                            recoverable: false,
                        });
                        break; // Exit loop — let post-processing run or transition to Idle
                    }
                }
            }

            // Fallback: use last partial when no Committed events were received
            // (Apple STT only sends cumulative Partial events, never Committed)
            if full_transcription.is_empty() && !last_partial_text.is_empty() {
                tracing::info!(
                    "No committed transcription received, using last partial text ({} chars)",
                    last_partial_text.len()
                );
                full_transcription = last_partial_text;
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

                tracing::info!(
                    "Starting LLM post-processing: input_len={} chars",
                    full_transcription.len()
                );
                tracing::debug!("LLM input text: {:?}", &full_transcription);

                let start_time = std::time::Instant::now();

                match llm_processor.process(task).await {
                    Ok(output) => {
                        tracing::info!(
                            "LLM processing successful (took {}ms, output_len={} chars)",
                            output.processing_time_ms,
                            output.text.len()
                        );
                        tracing::debug!("LLM output text: {:?}", &output.text);

                        // Output to clipboard/keyboard
                        if let Err(e) = output_sink.output_text(&output.text).await {
                            tracing::error!("Failed to output text: {}", e);
                            let _ = event_tx.send(PipelineEvent::Error {
                                message: format!("Output failed: {}", e),
                                recoverable: true,
                            });
                        }

                        // Emit final result
                        let _ = event_tx.send(PipelineEvent::FinalResult {
                            text: output.text,
                            processing_time_ms: start_time.elapsed().as_millis() as u64,
                        });

                        // Transition to Done state
                        {
                            let mut state = state_arc.lock().await;
                            *state = PipelineState::Done;
                        }
                        let _ = event_tx.send(PipelineEvent::StateChanged {
                            state: PipelineState::Done,
                            timestamp_ms: last_timestamp,
                        });
                    }
                    Err(e) => {
                        tracing::error!("LLM processing failed: {}", e);

                        // Emit error but try to output raw transcription
                        let _ = event_tx.send(PipelineEvent::Error {
                            message: format!(
                                "LLM processing failed: {}. Using raw transcription.",
                                e
                            ),
                            recoverable: true,
                        });

                        // Output raw transcription as fallback
                        if let Err(e) = output_sink.output_text(&full_transcription).await {
                            tracing::error!("Failed to output raw transcription: {}", e);
                        }

                        // Emit raw transcription as final result
                        let _ = event_tx.send(PipelineEvent::FinalResult {
                            text: full_transcription,
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
                    *state = PipelineState::Idle;
                }
                let _ = event_tx.send(PipelineEvent::StateChanged {
                    state: PipelineState::Idle,
                    timestamp_ms: last_timestamp,
                });
            }

            tracing::debug!("Transcription task finished");
        });

        *self.transcription_task.lock().await = Some(transcription_task);

        // Create audio capture
        let mut capture = AudioCapture::new();
        capture.start().map_err(|e| {
            tracing::error!("Failed to start audio capture: {}", e);
            MurmurError::Audio(e.to_string())
        })?;

        // Subscribe to audio levels for waveform
        if let Some(mut level_rx) = capture.subscribe_levels() {
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

        // Subscribe to audio chunks and forward to STT
        if let Some(mut chunk_rx) = capture.subscribe_chunks() {
            let audio_task = tokio::spawn(async move {
                while let Some(chunk) = chunk_rx.recv().await {
                    if let Err(e) = stt.send_audio(chunk).await {
                        tracing::error!("Failed to send audio to STT: {}", e);
                        break;
                    }
                }
                tracing::debug!("Audio forwarding task finished");

                // Stop STT session when audio ends
                let _ = stt.stop_session().await;
            });

            *self.audio_task.lock().await = Some(audio_task);
        }

        // Store capture instance
        *self.audio_capture.lock().await = Some(capture);

        tracing::info!("Pipeline started successfully");
        Ok(())
    }

    /// Stop the pipeline
    pub async fn stop(&self) -> Result<()> {
        // Read state for logging only (don't hold lock across async operations)
        {
            let state = self.state.lock().await;
            tracing::info!("Stopping pipeline (current state: {:?})", *state);
        }

        // Stop audio capture
        if let Some(mut capture) = self.audio_capture.lock().await.take() {
            capture
                .stop()
                .map_err(|e| MurmurError::Audio(e.to_string()))?;
        }

        // Cancel level task (just UI, safe to abort)
        if let Some(task) = self.level_task.lock().await.take() {
            task.abort();
        }

        // DON'T abort audio_task — let it finish naturally.
        // Stopping audio capture (above) closes chunk_tx, causing chunk_rx.recv()
        // to return None, which triggers stt.stop_session() for clean shutdown.
        // This is important for Apple STT's destroyAndWait() synchronization.

        // DON'T abort transcription_task — let it finish naturally.
        // The flow: audio capture stops → chunk channel closes → audio_task
        // calls stt.stop_session() → STT processes remaining audio → event
        // channel closes → transcription task exits loop → post-processing
        // runs (LLM, clipboard copy, FinalResult, Done state transition).

        tracing::info!("Pipeline stopped (post-processing will continue)");
        Ok(())
    }

    /// Reset the pipeline to idle state
    pub async fn reset(&self) -> Result<()> {
        let state = *self.state.lock().await;
        if state != PipelineState::Idle {
            self.stop().await?;
        }
        tracing::info!("Pipeline reset to idle");
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lt_core::llm::{ProcessingOutput, ProcessingTask};
    use lt_core::stt::AudioChunk;
    use lt_output::ClipboardOutput;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    // Mock LLM processor for testing
    struct MockLlmProcessor;

    #[async_trait]
    impl LlmProcessor for MockLlmProcessor {
        async fn process(&self, task: ProcessingTask) -> Result<ProcessingOutput> {
            match task {
                ProcessingTask::PostProcess { text, .. } => Ok(ProcessingOutput {
                    text: format!("Processed: {}", text),
                    processing_time_ms: 10,
                    metadata: None,
                }),
                _ => unimplemented!(),
            }
        }

        async fn health_check(&self) -> Result<bool> {
            Ok(true)
        }
    }

    // Mock STT provider for testing
    struct MockSttProvider {
        event_tx: mpsc::Sender<TranscriptionEvent>,
    }

    #[async_trait]
    impl SttProvider for MockSttProvider {
        async fn start_session(&mut self) -> Result<()> {
            Ok(())
        }

        async fn send_audio(&mut self, _chunk: AudioChunk) -> Result<()> {
            Ok(())
        }

        async fn stop_session(&mut self) -> Result<()> {
            Ok(())
        }

        async fn subscribe_events(&self) -> tokio::sync::mpsc::Receiver<TranscriptionEvent> {
            let (_tx, rx) = mpsc::channel(10);
            rx
        }
    }

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let llm = Arc::new(MockLlmProcessor);
        let output = Arc::new(ClipboardOutput::new().unwrap());
        let dict = Arc::new(Mutex::new(PersonalDictionary::new()));

        let orchestrator = PipelineOrchestrator::new(llm, output, dict);

        assert_eq!(orchestrator.get_state().await, PipelineState::Idle);
    }

    #[tokio::test]
    async fn test_orchestrator_state_transitions() {
        let llm = Arc::new(MockLlmProcessor);
        let output = Arc::new(ClipboardOutput::new().unwrap());
        let dict = Arc::new(Mutex::new(PersonalDictionary::new()));

        let orchestrator = PipelineOrchestrator::new(llm, output, dict);

        // Initial state should be Idle
        assert_eq!(orchestrator.get_state().await, PipelineState::Idle);
    }
}
