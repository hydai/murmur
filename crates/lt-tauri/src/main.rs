// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lt_audio::{AudioCapture, AudioLevel};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tracing_subscriber;

/// Application state managing audio capture
struct AppState {
    audio_capture: Arc<Mutex<Option<AudioCapture>>>,
    level_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
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

    let mut capture_guard = state.audio_capture.lock().unwrap();

    // Check if already recording
    if let Some(capture) = &*capture_guard {
        if capture.is_running() {
            return Err("Recording already in progress".to_string());
        }
    }

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

        let mut task_guard = state.level_task.lock().unwrap();
        *task_guard = Some(task);
    }

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

    let mut capture_guard = state.audio_capture.lock().unwrap();

    if let Some(mut capture) = capture_guard.take() {
        if let Err(e) = capture.stop() {
            tracing::error!("Error stopping capture: {}", e);
            return Err(format!("Failed to stop recording: {}", e));
        }
    } else {
        return Err("No recording in progress".to_string());
    }

    // Cancel level task
    let mut task_guard = state.level_task.lock().unwrap();
    if let Some(task) = task_guard.take() {
        task.abort();
    }

    // Emit recording state event
    let _ = app.emit("recording-state", RecordingStateEvent {
        is_recording: false,
    });

    tracing::info!("Recording stopped successfully");
    Ok(())
}

#[tauri::command]
fn is_recording(state: tauri::State<'_, AppState>) -> bool {
    let capture_guard = state.audio_capture.lock().unwrap();
    if let Some(capture) = &*capture_guard {
        capture.is_running()
    } else {
        false
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
                                let capture_guard = state.audio_capture.lock().unwrap();
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
