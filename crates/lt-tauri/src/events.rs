use lt_core::HistoryEntry;
use lt_pipeline::{PipelineEvent, PipelineState};
use tauri::Emitter;
use tokio::sync::{broadcast, mpsc};

use crate::{rebuild_tray_menu, sound, storage::HistoryStore};

#[derive(Clone, serde::Serialize)]
struct PipelineStateEvent {
    state: String,
    timestamp_ms: u64,
}

#[derive(Clone, serde::Serialize)]
struct AudioLevelEvent {
    rms: f32,
    voice_active: bool,
    timestamp_ms: u64,
}

#[derive(Clone, serde::Serialize)]
struct TranscriptionEvent {
    text: String,
    timestamp_ms: u64,
}

#[derive(Clone, serde::Serialize)]
struct FinalResultEvent {
    text: String,
    processing_time_ms: u64,
}

#[derive(Clone, serde::Serialize)]
struct ErrorEvent {
    message: String,
    recoverable: bool,
}

/// Exactly one forwarder is created during app setup, before recording can start.
/// Its owner aborts it on application teardown.
pub(crate) struct EventForwarder(tauri::async_runtime::JoinHandle<()>);

impl Drop for EventForwarder {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn spawn(
    app_clone: tauri::AppHandle,
    mut event_rx: broadcast::Receiver<PipelineEvent>,
    history: HistoryStore,
) -> EventForwarder {
    EventForwarder(tauri::async_runtime::spawn(async move {
        // The writer drains and exits once this task drops the queue.
        let (history_queue, _writer) = spawn_history_writer(history.clone());
        // Track raw transcription and command for history
        let mut raw_transcription = String::new();
        let mut detected_command: Option<String> = None;

        loop {
            let event = match event_rx.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    tracing::warn!("Pipeline event receiver lagged by {count} events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            match event {
                PipelineEvent::StateChanged {
                    state,
                    timestamp_ms,
                } => {
                    if state == PipelineState::Recording {
                        raw_transcription.clear();
                        detected_command = None;
                    }
                    match state {
                        PipelineState::Recording => sound::play_start_sound(),
                        PipelineState::Done | PipelineState::Error => sound::play_stop_sound(),
                        _ => {}
                    }
                    tracing::info!("Pipeline state changed: {:?}", state);
                    let state_str = match state {
                        PipelineState::Idle => "idle",
                        PipelineState::Recording => "recording",
                        PipelineState::Transcribing => "transcribing",
                        PipelineState::Processing => "processing",
                        PipelineState::Done => "done",
                        PipelineState::Error => "error",
                    };

                    let _ = app_clone.emit(
                        "pipeline-state",
                        PipelineStateEvent {
                            state: state_str.to_string(),
                            timestamp_ms,
                        },
                    );

                    // Also emit recording-state for compatibility
                    let is_recording = matches!(
                        state,
                        PipelineState::Recording | PipelineState::Transcribing
                    );
                    let _ = app_clone.emit(
                        "recording-state",
                        serde_json::json!({
                            "is_recording": is_recording
                        }),
                    );

                    // Update tray menu to reflect recording state
                    if let Err(e) = rebuild_tray_menu(&app_clone, is_recording) {
                        tracing::warn!("Failed to update tray menu: {}", e);
                    }
                }
                PipelineEvent::AudioLevel {
                    rms,
                    voice_active,
                    timestamp_ms,
                } => {
                    let _ = app_clone.emit(
                        "audio-level",
                        AudioLevelEvent {
                            rms,
                            voice_active,
                            timestamp_ms,
                        },
                    );
                }
                PipelineEvent::PartialTranscription { text, timestamp_ms } => {
                    let _ = app_clone.emit(
                        "transcription-partial",
                        TranscriptionEvent { text, timestamp_ms },
                    );
                }
                PipelineEvent::CommittedTranscription { text, timestamp_ms } => {
                    // Accumulate raw transcription for history
                    if !raw_transcription.is_empty() {
                        raw_transcription.push(' ');
                    }
                    raw_transcription.push_str(&text);

                    let _ = app_clone.emit(
                        "transcription-committed",
                        TranscriptionEvent { text, timestamp_ms },
                    );
                }
                PipelineEvent::CommandDetected {
                    command_name,
                    timestamp_ms,
                } => {
                    // Capture command for history
                    detected_command = command_name.clone();

                    let _ = app_clone.emit(
                        "command-detected",
                        serde_json::json!({
                            "command_name": command_name,
                            "timestamp_ms": timestamp_ms
                        }),
                    );
                }
                PipelineEvent::FinalResult {
                    text,
                    processing_time_ms,
                } => {
                    tracing::info!(
                        "Pipeline completed: {} chars in {}ms",
                        text.len(),
                        processing_time_ms
                    );

                    let _ = app_clone.emit(
                        "pipeline-result",
                        FinalResultEvent {
                            text: text.clone(),
                            processing_time_ms,
                        },
                    );

                    let raw = std::mem::take(&mut raw_transcription);
                    let entry = lt_core::HistoryEntry::new(
                        text,
                        if raw.is_empty() { None } else { Some(raw) },
                        processing_time_ms,
                        detected_command.take(),
                    );
                    // Tag with the generation now: a clear issued after this
                    // point must win over the queued append.
                    if history_queue.send((entry, history.generation())).is_err() {
                        tracing::warn!("History writer stopped; entry not saved");
                    }
                }
                PipelineEvent::Error {
                    message,
                    recoverable,
                } => {
                    tracing::error!("Pipeline error: {} (recoverable: {})", message, recoverable);

                    let _ = app_clone.emit(
                        "pipeline-error",
                        ErrorEvent {
                            message: message.clone(),
                            recoverable,
                        },
                    );

                    // Emit as audio-error for compatibility
                    let _ = app_clone.emit(
                        "audio-error",
                        serde_json::json!({
                            "message": message
                        }),
                    );
                }
            }
        }
        tracing::debug!("Pipeline event forwarding task finished");
    }))
}

/// History writes run on their own ordered queue so a slow disk cannot stall
/// event forwarding and lag the broadcast receiver behind audio levels.
fn spawn_history_writer(
    history: HistoryStore,
) -> (
    mpsc::UnboundedSender<(HistoryEntry, u64)>,
    tokio::task::JoinHandle<()>,
) {
    let (queue, mut entries) = mpsc::unbounded_channel();
    let writer = tokio::spawn(async move {
        while let Some((entry, generation)) = entries.recv().await {
            if let Err(error) = history.append(entry, generation).await {
                tracing::warn!("Failed to save history: {error}");
            }
        }
    });
    (queue, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn history_writer_persists_entries_in_send_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path().join("history.json"));
        let (queue, writer) = spawn_history_writer(store.clone());
        for text in ["one", "two", "three"] {
            queue
                .send((
                    HistoryEntry::new(text.into(), None, 0, None),
                    store.generation(),
                ))
                .unwrap();
        }
        drop(queue);
        writer.await.unwrap();

        let texts: Vec<String> = store
            .read()
            .await
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.final_text.clone())
            .collect();
        assert_eq!(texts, ["three", "two", "one"]);
    }
}
