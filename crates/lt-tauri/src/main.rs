// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lt_audio::{AudioCapture, AudioLevel};
use lt_core::llm::{LlmProcessor, ProcessingTask};
use lt_core::stt::{SttProvider, TranscriptionEvent};
use lt_core::{AppConfig, PersonalDictionary};
use lt_llm::GeminiProcessor;
use lt_stt::ElevenLabsProvider;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;
use tracing_subscriber;

/// Application state managing audio capture and STT
#[derive(Clone)]
struct AppState {
    audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    level_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    stt_provider: Arc<Mutex<Option<ElevenLabsProvider>>>,
    audio_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    transcription_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    llm_processor: Arc<GeminiProcessor>,
    dictionary: Arc<Mutex<PersonalDictionary>>,
}

#[derive(Clone, serde::Serialize)]
struct AudioLevelEvent {
    rms: f32,
    voice_active: bool,
    timestamp_ms: u64,
}

impl From<AudioLevel> for AudioLevelEvent {
    fn from(level: AudioLevel) -> Self {
        Self {
            rms: level.rms,
            voice_active: level.voice_active,
            timestamp_ms: level.timestamp_ms,
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct RecordingStateEvent {
    is_recording: bool,
}

#[derive(Clone, serde::Serialize)]
struct ErrorEvent {
    message: String,
}

#[tauri::command]
fn toggle_overlay(window: tauri::WebviewWindow) -> Result<bool, String> {
    let is_visible = window.is_visible().map_err(|e| e.to_string())?;

    if is_visible {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }

    Ok(!is_visible)
}

#[tauri::command]
fn get_status() -> String {
    "Ready".to_string()
}

#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("Starting recording");

    let mut capture_guard = state.audio_capture.lock().await;

    // Check if already recording
    if let Some(capture) = &*capture_guard {
        if capture.is_running() {
            return Err("Recording already in progress".to_string());
        }
    }

    // Load config and get API key
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        tracing::warn!("Config file not found, using default config");
        AppConfig::default()
    };

    let api_key = config.api_keys.get("elevenlabs")
        .ok_or_else(|| {
            "ElevenLabs API key not configured. Please add your API key to ~/.config/localtype/config.toml".to_string()
        })?
        .clone();

    // Create and start STT provider
    let mut stt = ElevenLabsProvider::new(api_key);

    if let Err(e) = stt.start_session().await {
        let error_msg = format!("Failed to start STT session: {}", e);
        tracing::error!("{}", error_msg);
        let _ = app.emit("audio-error", ErrorEvent {
            message: error_msg.clone(),
        });
        return Err(error_msg);
    }

    // Subscribe to transcription events
    let mut event_rx = stt.subscribe_events().await;
    let app_clone = app.clone();

    let app_for_llm = app.clone();

    let transcription_task = tokio::spawn(async move {
        let mut full_transcription = String::new();

        while let Some(event) = event_rx.recv().await {
            match &event {
                TranscriptionEvent::Partial { text, .. } => {
                    tracing::debug!("Partial transcript: {}", text);
                    let _ = app_clone.emit("transcription-partial", event.clone());
                }
                TranscriptionEvent::Committed { text, .. } => {
                    tracing::info!("Committed transcript: {}", text);
                    let _ = app_clone.emit("transcription-committed", event.clone());

                    // Accumulate transcription
                    if !full_transcription.is_empty() {
                        full_transcription.push(' ');
                    }
                    full_transcription.push_str(text);
                }
                TranscriptionEvent::Error { message } => {
                    tracing::error!("STT error: {}", message);
                    let _ = app_clone.emit("transcription-error", event.clone());
                }
            }
        }

        // When transcription finishes (channel closed), trigger LLM processing
        if !full_transcription.is_empty() {
            tracing::info!("Transcription complete, starting LLM post-processing");

            let state_for_llm = app_for_llm.state::<AppState>();

            let _ = app_for_llm.emit("processing-status", ProcessingStatusEvent {
                status: "processing".to_string(),
            });

            // Get dictionary terms
            let dictionary_terms = {
                let dict = state_for_llm.dictionary.lock().await;
                dict.get_terms()
            };

            let task = ProcessingTask::PostProcess {
                text: full_transcription.clone(),
                dictionary_terms,
            };

            match state_for_llm.llm_processor.process(task).await {
                Ok(output) => {
                    tracing::info!(
                        "LLM processing successful (took {}ms)",
                        output.processing_time_ms
                    );

                    // Emit processed text as a final committed transcription
                    let _ = app_for_llm.emit(
                        "transcription-processed",
                        serde_json::json!({
                            "text": output.text,
                            "processing_time_ms": output.processing_time_ms
                        })
                    );

                    let _ = app_for_llm.emit("processing-status", ProcessingStatusEvent {
                        status: "done".to_string(),
                    });
                }
                Err(e) => {
                    tracing::error!("LLM processing failed: {}", e);

                    // Emit error but keep raw transcription
                    let _ = app_for_llm.emit("audio-error", ErrorEvent {
                        message: format!("LLM processing failed: {}. Showing raw transcription.", e),
                    });

                    let _ = app_for_llm.emit("processing-status", ProcessingStatusEvent {
                        status: "error".to_string(),
                    });
                }
            }
        }

        tracing::debug!("Transcription event task finished");
    });

    *state.transcription_task.lock().await = Some(transcription_task);

    // Create new capture
    let mut capture = AudioCapture::new();

    // Start capture
    if let Err(e) = capture.start() {
        let error_msg = match e {
            lt_audio::AudioError::NoInputDevice => {
                "No microphone found. Please connect a microphone and try again.".to_string()
            }
            lt_audio::AudioError::PermissionDenied => {
                "Microphone permission denied. Please grant microphone access in System Preferences.".to_string()
            }
            _ => format!("Failed to start audio capture: {}", e),
        };

        tracing::error!("{}", error_msg);

        // Emit error event
        let _ = app.emit("audio-error", ErrorEvent {
            message: error_msg.clone(),
        });

        return Err(error_msg);
    }

    // Subscribe to audio levels for waveform
    if let Some(mut level_rx) = capture.subscribe_levels() {
        let app_clone = app.clone();

        // Spawn task to emit level events
        let task = tokio::spawn(async move {
            while let Some(level) = level_rx.recv().await {
                let event: AudioLevelEvent = level.into();
                let _ = app_clone.emit("audio-level", event);
            }
            tracing::debug!("Audio level task finished");
        });

        *state.level_task.lock().await = Some(task);
    }

    // Subscribe to audio chunks and forward to STT
    // Move stt into the audio forwarding task to avoid lock issues
    if let Some(mut chunk_rx) = capture.subscribe_chunks() {
        let audio_task = tokio::spawn(async move {
            while let Some(chunk) = chunk_rx.recv().await {
                if let Err(e) = stt.send_audio(chunk).await {
                    tracing::error!("Failed to send audio to STT: {}", e);
                    break;
                }
            }
            tracing::debug!("Audio forwarding task finished");
        });

        *state.audio_task.lock().await = Some(audio_task);
    }

    // Store the STT provider reference (just to keep track, but it's moved into the task)
    *state.stt_provider.lock().await = None;

    // Store capture instance
    *capture_guard = Some(capture);

    // Emit recording state event
    let _ = app.emit("recording-state", RecordingStateEvent {
        is_recording: true,
    });

    tracing::info!("Recording started successfully");
    Ok(())
}

#[tauri::command]
async fn stop_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("Stopping recording");

    let mut capture_guard = state.audio_capture.lock().await;

    if let Some(mut capture) = capture_guard.take() {
        if let Err(e) = capture.stop() {
            tracing::error!("Error stopping capture: {}", e);
            return Err(format!("Failed to stop recording: {}", e));
        }
    } else {
        return Err("No recording in progress".to_string());
    }

    // Cancel level task
    {
        let mut task_guard = state.level_task.lock().await;
        if let Some(task) = task_guard.take() {
            task.abort();
        }
    }

    // Cancel audio forwarding task (this will also stop STT since it owns it)
    {
        let mut audio_task_guard = state.audio_task.lock().await;
        if let Some(task) = audio_task_guard.take() {
            task.abort();
        }
    }

    // Cancel transcription task
    {
        let mut transcription_task_guard = state.transcription_task.lock().await;
        if let Some(task) = transcription_task_guard.take() {
            task.abort();
        }
    }

    // Emit recording state event
    let _ = app.emit("recording-state", RecordingStateEvent {
        is_recording: false,
    });

    tracing::info!("Recording stopped successfully");
    Ok(())
}

#[tauri::command]
async fn is_recording(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let capture_guard = state.audio_capture.lock().await;
    if let Some(capture) = &*capture_guard {
        Ok(capture.is_running())
    } else {
        Ok(false)
    }
}

#[derive(Clone, serde::Serialize)]
struct ProcessingStatusEvent {
    status: String,
}

#[tauri::command]
async fn process_text(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
) -> Result<String, String> {
    tracing::info!("Starting LLM post-processing (text length: {} chars)", text.len());

    // Emit processing status
    let _ = app.emit("processing-status", ProcessingStatusEvent {
        status: "processing".to_string(),
    });

    // Get dictionary terms
    let dictionary_terms = {
        let dict = state.dictionary.lock().await;
        dict.get_terms()
    };

    // Create processing task
    let task = ProcessingTask::PostProcess {
        text: text.clone(),
        dictionary_terms,
    };

    // Process with LLM
    match state.llm_processor.process(task).await {
        Ok(output) => {
            tracing::info!(
                "LLM processing successful (took {}ms, output length: {} chars)",
                output.processing_time_ms,
                output.text.len()
            );

            // Emit done status
            let _ = app.emit("processing-status", ProcessingStatusEvent {
                status: "done".to_string(),
            });

            Ok(output.text)
        }
        Err(e) => {
            tracing::error!("LLM processing failed: {}", e);

            // Emit error and return raw text as fallback
            let _ = app.emit("audio-error", ErrorEvent {
                message: format!("LLM processing failed: {}. Showing raw transcription.", e),
            });

            let _ = app.emit("processing-status", ProcessingStatusEvent {
                status: "error".to_string(),
            });

            // Return raw text as fallback
            Ok(text)
        }
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lt_tauri=debug,lt_audio=debug,lt_stt=debug,lt_llm=debug,info")
        .init();

    // Initialize LLM processor
    let llm_processor = Arc::new(GeminiProcessor::new());

    // Load dictionary (or create empty if not exists)
    let dictionary = {
        let dict_path = AppConfig::default_config_dir()
            .ok()
            .map(|dir| dir.join("dictionary.json"));

        if let Some(path) = dict_path.as_ref() {
            if path.exists() {
                match PersonalDictionary::load_from_file(path) {
                    Ok(dict) => {
                        tracing::info!("Loaded personal dictionary with {} entries", dict.entries.len());
                        dict
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load dictionary: {}, using empty dictionary", e);
                        PersonalDictionary::new()
                    }
                }
            } else {
                tracing::info!("No dictionary file found, using empty dictionary");
                PersonalDictionary::new()
            }
        } else {
            tracing::warn!("Could not determine dictionary path, using empty dictionary");
            PersonalDictionary::new()
        }
    };

    // Create app state
    let app_state = AppState {
        audio_capture: Arc::new(Mutex::new(None)),
        level_task: Arc::new(Mutex::new(None)),
        stt_provider: Arc::new(Mutex::new(None)),
        audio_task: Arc::new(Mutex::new(None)),
        transcription_task: Arc::new(Mutex::new(None)),
        llm_processor,
        dictionary: Arc::new(Mutex::new(dictionary)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            toggle_overlay,
            get_status,
            start_recording,
            stop_recording,
            is_recording,
            process_text
        ])
        .setup(|app| {
            // Perform LLM health check
            let app_handle_clone = app.handle().clone();
            tokio::spawn(async move {
                let state = app_handle_clone.state::<AppState>();
                match state.llm_processor.health_check().await {
                    Ok(true) => {
                        tracing::info!("✓ Gemini CLI is available and ready");
                    }
                    Ok(false) => {
                        tracing::warn!("⚠ Gemini CLI is not installed. LLM post-processing will not be available.");
                        tracing::warn!("  Install gemini-cli: https://github.com/google/generative-ai-cli");
                    }
                    Err(e) => {
                        tracing::error!("✗ Failed to check Gemini CLI health: {}", e);
                    }
                }
            });

            // Try to register global shortcut for recording toggle
            let app_handle = app.handle().clone();

            // Register the shortcut handler first
            if let Err(e) =
                app.global_shortcut()
                    .on_shortcut("Cmd+Shift+Space", move |_app, _shortcut, _event| {
                        // Toggle recording using the cloned handle
                        let handle = app_handle.clone();

                        tokio::spawn(async move {
                            let state = handle.state::<AppState>();

                            let is_currently_recording = {
                                let capture_guard = state.audio_capture.lock().await;
                                if let Some(capture) = &*capture_guard {
                                    capture.is_running()
                                } else {
                                    false
                                }
                            };

                            if is_currently_recording {
                                // Stop recording
                                let _ = stop_recording(handle.clone(), state).await;
                            } else {
                                // Start recording
                                let _ = start_recording(handle.clone(), state).await;
                            }
                        });
                    })
            {
                tracing::warn!("Failed to set up shortcut handler: {}", e);
            }

            // Try to register the global shortcut (non-fatal if it fails)
            match app.global_shortcut().register("Cmd+Shift+Space") {
                Ok(_) => tracing::info!("Global shortcut Cmd+Shift+Space registered successfully"),
                Err(e) => tracing::warn!(
                    "Failed to register global shortcut: {}. The app will still work, but you'll need to use the window controls.",
                    e
                ),
            }

            tracing::info!("Localtype started successfully");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
