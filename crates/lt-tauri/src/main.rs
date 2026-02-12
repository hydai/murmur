// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use lt_core::llm::LlmProcessor;
use lt_core::output::OutputMode;
use lt_core::{AppConfig, PersonalDictionary};
use lt_llm::GeminiProcessor;
use lt_output::CombinedOutput;
use lt_pipeline::{PipelineEvent, PipelineOrchestrator, PipelineState};
use lt_stt::ElevenLabsProvider;
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;
use tracing_subscriber;

/// Application state using unified pipeline
#[derive(Clone)]
struct AppState {
    pipeline: Arc<Mutex<PipelineOrchestrator>>,
    event_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

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
async fn start_pipeline(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("Starting pipeline");

    let pipeline = state.pipeline.lock().await;

    // Check if pipeline is already running
    let current_state = pipeline.get_state().await;
    if current_state != PipelineState::Idle {
        return Err(format!("Pipeline is already running (state: {:?})", current_state));
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

    // Create STT provider
    let stt = ElevenLabsProvider::new(api_key);

    // Subscribe to pipeline events before starting
    let mut event_rx = pipeline.subscribe_events();
    let app_clone = app.clone();

    // Spawn task to forward pipeline events to frontend
    let event_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                PipelineEvent::StateChanged { state, timestamp_ms } => {
                    let state_str = match state {
                        PipelineState::Idle => "idle",
                        PipelineState::Recording => "recording",
                        PipelineState::Transcribing => "transcribing",
                        PipelineState::Processing => "processing",
                        PipelineState::Done => "done",
                        PipelineState::Error => "error",
                    };

                    let _ = app_clone.emit("pipeline-state", PipelineStateEvent {
                        state: state_str.to_string(),
                        timestamp_ms,
                    });

                    // Also emit recording-state for compatibility
                    let is_recording = matches!(state, PipelineState::Recording | PipelineState::Transcribing);
                    let _ = app_clone.emit("recording-state", serde_json::json!({
                        "is_recording": is_recording
                    }));
                }
                PipelineEvent::AudioLevel { rms, voice_active, timestamp_ms } => {
                    let _ = app_clone.emit("audio-level", AudioLevelEvent {
                        rms,
                        voice_active,
                        timestamp_ms,
                    });
                }
                PipelineEvent::PartialTranscription { text, timestamp_ms } => {
                    let _ = app_clone.emit("transcription-partial", TranscriptionEvent {
                        text,
                        timestamp_ms,
                    });
                }
                PipelineEvent::CommittedTranscription { text, timestamp_ms } => {
                    let _ = app_clone.emit("transcription-committed", TranscriptionEvent {
                        text,
                        timestamp_ms,
                    });
                }
                PipelineEvent::FinalResult { text, processing_time_ms } => {
                    tracing::info!("Pipeline completed: {} chars in {}ms", text.len(), processing_time_ms);

                    let _ = app_clone.emit("pipeline-result", FinalResultEvent {
                        text: text.clone(),
                        processing_time_ms,
                    });

                    // Emit as transcription-processed for compatibility
                    let _ = app_clone.emit("transcription-processed", serde_json::json!({
                        "text": text,
                        "processing_time_ms": processing_time_ms
                    }));
                }
                PipelineEvent::Error { message, recoverable } => {
                    tracing::error!("Pipeline error: {} (recoverable: {})", message, recoverable);

                    let _ = app_clone.emit("pipeline-error", ErrorEvent {
                        message: message.clone(),
                        recoverable,
                    });

                    // Emit as audio-error for compatibility
                    let _ = app_clone.emit("audio-error", serde_json::json!({
                        "message": message
                    }));
                }
            }
        }
        tracing::debug!("Pipeline event forwarding task finished");
    });

    *state.event_task.lock().await = Some(event_task);

    // Start the pipeline
    pipeline.start(Box::new(stt)).await
        .map_err(|e| {
            tracing::error!("Failed to start pipeline: {}", e);
            format!("Failed to start pipeline: {}", e)
        })?;

    tracing::info!("Pipeline started successfully");
    Ok(())
}

#[tauri::command]
async fn stop_pipeline(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    tracing::info!("Stopping pipeline");

    let pipeline = state.pipeline.lock().await;

    pipeline.stop().await
        .map_err(|e| {
            tracing::error!("Failed to stop pipeline: {}", e);
            format!("Failed to stop pipeline: {}", e)
        })?;

    tracing::info!("Pipeline stopped successfully");
    Ok(())
}

#[tauri::command]
async fn is_recording(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let pipeline = state.pipeline.lock().await;
    let current_state = pipeline.get_state().await;

    Ok(matches!(current_state, PipelineState::Recording | PipelineState::Transcribing))
}

#[tauri::command]
async fn get_pipeline_state(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let pipeline = state.pipeline.lock().await;
    let current_state = pipeline.get_state().await;

    let state_str = match current_state {
        PipelineState::Idle => "idle",
        PipelineState::Recording => "recording",
        PipelineState::Transcribing => "transcribing",
        PipelineState::Processing => "processing",
        PipelineState::Done => "done",
        PipelineState::Error => "error",
    };

    Ok(state_str.to_string())
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lt_tauri=debug,lt_audio=debug,lt_stt=debug,lt_llm=debug,lt_pipeline=debug,lt_output=debug,info")
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

    // Initialize output sink (clipboard by default)
    let output_sink = Arc::new(
        CombinedOutput::new(OutputMode::Clipboard)
            .expect("Failed to initialize output sink")
    );

    // Create pipeline orchestrator
    let pipeline = PipelineOrchestrator::new(
        llm_processor.clone() as Arc<dyn lt_core::llm::LlmProcessor>,
        output_sink,
        Arc::new(Mutex::new(dictionary)),
    );

    // Create app state
    let app_state = AppState {
        pipeline: Arc::new(Mutex::new(pipeline)),
        event_task: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            toggle_overlay,
            get_status,
            start_pipeline,
            stop_pipeline,
            is_recording,
            get_pipeline_state
        ])
        .setup(|app| {
            // Perform LLM health check
            tokio::spawn(async move {
                let llm_processor = GeminiProcessor::new();
                match llm_processor.health_check().await {
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

            // Try to register global shortcut for pipeline toggle
            let app_handle = app.handle().clone();

            // Register the shortcut handler first
            if let Err(e) =
                app.global_shortcut()
                    .on_shortcut("Cmd+Shift+Space", move |_app, _shortcut, _event| {
                        // Toggle pipeline using the cloned handle
                        let handle = app_handle.clone();

                        tokio::spawn(async move {
                            let state = handle.state::<AppState>();

                            let is_currently_recording = {
                                let pipeline = state.pipeline.lock().await;
                                let current_state = pipeline.get_state().await;
                                matches!(current_state, PipelineState::Recording | PipelineState::Transcribing)
                            };

                            if is_currently_recording {
                                // Stop pipeline
                                let _ = stop_pipeline(handle.clone(), state).await;
                            } else {
                                // Start pipeline
                                let _ = start_pipeline(handle.clone(), state).await;
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

            tracing::info!("Localtype started successfully with unified pipeline");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
