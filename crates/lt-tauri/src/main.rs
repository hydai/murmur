// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lt_audio::{AudioCapture, AudioLevel};
use lt_core::stt::{SttProvider, TranscriptionEvent};
use lt_core::AppConfig;
use lt_stt::ElevenLabsProvider;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;
use tracing_subscriber;

/// Application state managing audio capture and STT
struct AppState {
    audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    level_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    stt_provider: Arc<Mutex<Option<ElevenLabsProvider>>>,
    audio_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    transcription_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
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

    let transcription_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match &event {
                TranscriptionEvent::Partial { text, .. } => {
                    tracing::debug!("Partial transcript: {}", text);
                    let _ = app_clone.emit("transcription-partial", event);
                }
                TranscriptionEvent::Committed { text, .. } => {
                    tracing::info!("Committed transcript: {}", text);
                    let _ = app_clone.emit("transcription-committed", event);
                }
                TranscriptionEvent::Error { message } => {
                    tracing::error!("STT error: {}", message);
                    let _ = app_clone.emit("transcription-error", event);
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

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lt_tauri=debug,lt_audio=debug,info")
        .init();

    // Create app state
    let app_state = AppState {
        audio_capture: Arc::new(Mutex::new(None)),
        level_task: Arc::new(Mutex::new(None)),
        stt_provider: Arc::new(Mutex::new(None)),
        audio_task: Arc::new(Mutex::new(None)),
        transcription_task: Arc::new(Mutex::new(None)),
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
            is_recording
        ])
        .setup(|app| {
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
