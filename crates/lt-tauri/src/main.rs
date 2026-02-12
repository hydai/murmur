// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod permissions;

use lt_core::config::{SttProviderType, LlmProcessorType};
use lt_core::llm::LlmProcessor;
use lt_core::output::OutputMode;
use lt_core::stt::SttProvider;
use lt_core::{AppConfig, PersonalDictionary};
use lt_llm::{GeminiProcessor, CopilotProcessor};
use lt_output::CombinedOutput;
use lt_pipeline::{PipelineEvent, PipelineOrchestrator, PipelineState};
use lt_stt::{ElevenLabsProvider, GroqProvider, OpenAIProvider};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
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
fn toggle_overlay(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app.get_webview_window("main").ok_or("Main window not found")?;
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
async fn get_config() -> Result<AppConfig, String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command]
async fn save_config(config: AppConfig) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[tauri::command]
async fn set_stt_provider(provider: String) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    // Parse provider string to SttProviderType
    let provider_type = match provider.to_lowercase().as_str() {
        "elevenlabs" => SttProviderType::ElevenLabs,
        "openai" => SttProviderType::OpenAI,
        "groq" => SttProviderType::Groq,
        _ => return Err(format!("Unknown STT provider: {}", provider)),
    };

    config.stt_provider = provider_type;

    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[tauri::command]
async fn save_api_key(provider: String, api_key: String) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    config.api_keys.insert(provider.to_lowercase(), api_key);

    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[derive(Clone, serde::Serialize)]
struct SttProviderInfo {
    name: String,
    id: String,
    provider_type: String,
    configured: bool,
}

#[tauri::command]
async fn get_stt_providers() -> Result<Vec<SttProviderInfo>, String> {
    let config = get_config().await?;

    let providers = vec![
        SttProviderInfo {
            name: "ElevenLabs Scribe".to_string(),
            id: "elevenlabs".to_string(),
            provider_type: "streaming".to_string(),
            configured: config.api_keys.contains_key("elevenlabs"),
        },
        SttProviderInfo {
            name: "OpenAI Whisper".to_string(),
            id: "openai".to_string(),
            provider_type: "batch".to_string(),
            configured: config.api_keys.contains_key("openai"),
        },
        SttProviderInfo {
            name: "Groq Whisper Turbo".to_string(),
            id: "groq".to_string(),
            provider_type: "batch".to_string(),
            configured: config.api_keys.contains_key("groq"),
        },
    ];

    Ok(providers)
}

#[derive(Clone, serde::Serialize)]
struct LlmProcessorInfo {
    name: String,
    id: String,
    available: bool,
}

#[tauri::command]
async fn get_llm_processors() -> Result<Vec<LlmProcessorInfo>, String> {
    // Check health for each processor
    let gemini = GeminiProcessor::new();
    let copilot = CopilotProcessor::new();

    let gemini_available = gemini.health_check().await.unwrap_or(false);
    let copilot_available = copilot.health_check().await.unwrap_or(false);

    let processors = vec![
        LlmProcessorInfo {
            name: "Gemini CLI".to_string(),
            id: "gemini".to_string(),
            available: gemini_available,
        },
        LlmProcessorInfo {
            name: "Copilot CLI".to_string(),
            id: "copilot".to_string(),
            available: copilot_available,
        },
    ];

    Ok(processors)
}

#[tauri::command]
async fn set_llm_processor(processor: String) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    // Parse processor string to LlmProcessorType
    let processor_type = match processor.to_lowercase().as_str() {
        "gemini" => LlmProcessorType::Gemini,
        "copilot" => LlmProcessorType::Copilot,
        _ => return Err(format!("Unknown LLM processor: {}", processor)),
    };

    config.llm_processor = processor_type;

    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[tauri::command]
async fn set_output_mode(mode: String) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    // Parse mode string to OutputMode
    let output_mode = match mode.to_lowercase().as_str() {
        "clipboard" => OutputMode::Clipboard,
        "keyboard" => OutputMode::Keyboard,
        "both" => OutputMode::Both,
        _ => return Err(format!("Unknown output mode: {}", mode)),
    };

    config.output_mode = output_mode;

    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[tauri::command]
async fn set_hotkey(hotkey: String, app: tauri::AppHandle) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    // Validate hotkey format (basic validation)
    if hotkey.is_empty() {
        return Err("Hotkey cannot be empty".to_string());
    }

    // Unregister old hotkey
    let old_hotkey = config.hotkey.clone();
    if let Err(e) = app.global_shortcut().unregister(old_hotkey.as_str()) {
        tracing::warn!("Failed to unregister old hotkey '{}': {}", old_hotkey, e);
    }

    // Update config
    config.hotkey = hotkey.clone();
    config.save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Register new hotkey
    let app_handle = app.clone();
    let hotkey_str = hotkey.clone();

    // Set up the handler for the new hotkey
    app.global_shortcut()
        .on_shortcut(hotkey_str.as_str(), move |_app, _shortcut, _event| {
            let handle = app_handle.clone();
            tokio::spawn(async move {
                let state = handle.state::<AppState>();
                let is_currently_recording = {
                    let pipeline = state.pipeline.lock().await;
                    let current_state = pipeline.get_state().await;
                    matches!(current_state, PipelineState::Recording | PipelineState::Transcribing)
                };

                if is_currently_recording {
                    let _ = stop_pipeline(handle.clone(), state).await;
                } else {
                    let _ = start_pipeline(handle.clone(), state).await;
                }
            });
        })
        .map_err(|e| format!("Failed to set hotkey handler: {}", e))?;

    // Register the hotkey
    app.global_shortcut()
        .register(hotkey.as_str())
        .map_err(|e| format!("Failed to register hotkey '{}': {}", hotkey, e))?;

    tracing::info!("Hotkey updated to: {}", hotkey);
    Ok(())
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

    // Create STT provider based on config
    let stt: Box<dyn SttProvider> = match config.stt_provider {
        SttProviderType::ElevenLabs => {
            let api_key = config.api_keys.get("elevenlabs")
                .ok_or_else(|| {
                    "ElevenLabs API key not configured. Please add your API key to ~/.config/localtype/config.toml".to_string()
                })?
                .clone();
            Box::new(ElevenLabsProvider::new(api_key))
        }
        SttProviderType::OpenAI => {
            let api_key = config.api_keys.get("openai")
                .ok_or_else(|| {
                    "OpenAI API key not configured. Please add your API key to ~/.config/localtype/config.toml".to_string()
                })?
                .clone();
            Box::new(OpenAIProvider::new(api_key))
        }
        SttProviderType::Groq => {
            let api_key = config.api_keys.get("groq")
                .ok_or_else(|| {
                    "Groq API key not configured. Please add your API key to ~/.config/localtype/config.toml".to_string()
                })?
                .clone();
            Box::new(GroqProvider::new(api_key))
        }
    };

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

                    // Update tray menu to reflect recording state
                    if let Err(e) = rebuild_tray_menu(&app_clone, is_recording) {
                        tracing::warn!("Failed to update tray menu: {}", e);
                    }
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
                PipelineEvent::CommandDetected { command_name, timestamp_ms } => {
                    let _ = app_clone.emit("command-detected", serde_json::json!({
                        "command_name": command_name,
                        "timestamp_ms": timestamp_ms
                    }));
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
    pipeline.start(stt).await
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

// Dictionary management commands

#[tauri::command]
async fn get_dictionary() -> Result<PersonalDictionary, String> {
    let dict_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("dictionary.json");

    if dict_path.exists() {
        PersonalDictionary::load_from_file(&dict_path)
            .map_err(|e| format!("Failed to load dictionary: {}", e))
    } else {
        Ok(PersonalDictionary::new())
    }
}

#[derive(serde::Deserialize)]
struct AddEntryParams {
    term: String,
    aliases: Vec<String>,
    description: Option<String>,
}

#[tauri::command]
async fn add_dictionary_entry(
    params: AddEntryParams,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dict_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("dictionary.json");

    // Ensure directory exists
    if let Some(parent) = dict_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    let mut dict = if dict_path.exists() {
        PersonalDictionary::load_from_file(&dict_path)
            .map_err(|e| format!("Failed to load dictionary: {}", e))?
    } else {
        PersonalDictionary::new()
    };

    let entry = lt_core::dictionary::DictionaryEntry {
        term: params.term,
        aliases: params.aliases,
        description: params.description,
    };

    dict.add_entry(entry);
    dict.save_to_file(&dict_path)
        .map_err(|e| format!("Failed to save dictionary: {}", e))?;

    // Update the dictionary in the pipeline
    let pipeline = state.pipeline.lock().await;
    let pipeline_dict = pipeline.get_dictionary();
    *pipeline_dict.lock().await = dict;

    Ok(())
}

#[derive(serde::Deserialize)]
struct UpdateEntryParams {
    old_term: String,
    term: String,
    aliases: Vec<String>,
    description: Option<String>,
}

#[tauri::command]
async fn update_dictionary_entry(
    params: UpdateEntryParams,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dict_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("dictionary.json");

    let mut dict = if dict_path.exists() {
        PersonalDictionary::load_from_file(&dict_path)
            .map_err(|e| format!("Failed to load dictionary: {}", e))?
    } else {
        return Err("Dictionary file not found".to_string());
    };

    let new_entry = lt_core::dictionary::DictionaryEntry {
        term: params.term,
        aliases: params.aliases,
        description: params.description,
    };

    if !dict.update_entry(&params.old_term, new_entry) {
        return Err(format!("Entry '{}' not found", params.old_term));
    }

    dict.save_to_file(&dict_path)
        .map_err(|e| format!("Failed to save dictionary: {}", e))?;

    // Update the dictionary in the pipeline
    let pipeline = state.pipeline.lock().await;
    let pipeline_dict = pipeline.get_dictionary();
    *pipeline_dict.lock().await = dict;

    Ok(())
}

#[tauri::command]
async fn delete_dictionary_entry(
    term: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let dict_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("dictionary.json");

    let mut dict = if dict_path.exists() {
        PersonalDictionary::load_from_file(&dict_path)
            .map_err(|e| format!("Failed to load dictionary: {}", e))?
    } else {
        return Err("Dictionary file not found".to_string());
    };

    if !dict.remove_entry(&term) {
        return Err(format!("Entry '{}' not found", term));
    }

    dict.save_to_file(&dict_path)
        .map_err(|e| format!("Failed to save dictionary: {}", e))?;

    // Update the dictionary in the pipeline
    let pipeline = state.pipeline.lock().await;
    let pipeline_dict = pipeline.get_dictionary();
    *pipeline_dict.lock().await = dict;

    Ok(())
}

#[tauri::command]
async fn search_dictionary(query: String) -> Result<Vec<lt_core::dictionary::DictionaryEntry>, String> {
    let dict_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("dictionary.json");

    let dict = if dict_path.exists() {
        PersonalDictionary::load_from_file(&dict_path)
            .map_err(|e| format!("Failed to load dictionary: {}", e))?
    } else {
        PersonalDictionary::new()
    };

    Ok(dict.search_entries(&query))
}

// ============================================================================
// Permission Management Commands
// ============================================================================

#[tauri::command]
fn check_permissions() -> permissions::PermissionsResult {
    permissions::PermissionsResult {
        microphone: permissions::check_microphone_permission(),
        accessibility: permissions::check_accessibility_permission(),
    }
}

#[tauri::command]
fn request_microphone_permission() -> Result<(), String> {
    permissions::request_microphone_permission()
}

#[tauri::command]
fn open_system_preferences(section: String) -> Result<(), String> {
    permissions::open_system_preferences(&section)
}

/// Helper function to create a red-tinted version of the icon for recording state
fn create_recording_icon(original_bytes: &[u8], _width: u32, _height: u32) -> Vec<u8> {
    let mut tinted = original_bytes.to_vec();
    // Apply red tint to the icon (increase red, decrease green/blue)
    for chunk in tinted.chunks_mut(4) {
        if chunk.len() == 4 {
            let alpha = chunk[3];
            if alpha > 0 {
                // Boost red channel
                chunk[0] = chunk[0].saturating_add(80);
                // Reduce green and blue
                chunk[1] = chunk[1].saturating_sub(40);
                chunk[2] = chunk[2].saturating_sub(40);
            }
        }
    }
    tinted
}

/// Helper function to rebuild tray menu with updated recording state
fn rebuild_tray_menu(app: &tauri::AppHandle, is_recording: bool) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app.tray_by_id("main-tray").ok_or("Tray not found")?;

    // Build menu items
    let toggle_item = MenuItemBuilder::with_id(
        "toggle_recording",
        if is_recording { "⏸ Stop Recording" } else { "⏺ Start Recording" }
    ).build(app)?;

    let settings_item = MenuItemBuilder::with_id("open_settings", "⚙ Open Settings").build(app)?;
    let overlay_item = MenuItemBuilder::with_id("toggle_overlay", "👁 Show/Hide Overlay").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&toggle_item)
        .item(&settings_item)
        .item(&overlay_item)
        .separator()
        .item(&quit_item)
        .build()?;

    tray.set_menu(Some(menu))?;

    // Update tooltip to reflect recording state
    let tooltip = if is_recording {
        "Localtype - Recording"
    } else {
        "Localtype"
    };
    tray.set_tooltip(Some(tooltip))?;

    // Update icon to reflect recording state
    if let Ok(tray_icon_path) = app.path().resolve("icons/32x32.png", tauri::path::BaseDirectory::Resource) {
        if let Ok(icon_image) = image::open(&tray_icon_path) {
            let rgba_image = icon_image.to_rgba8();
            let (width, height) = rgba_image.dimensions();
            let original_bytes = rgba_image.into_raw();

            let icon_bytes = if is_recording {
                create_recording_icon(&original_bytes, width, height)
            } else {
                original_bytes
            };

            let icon = tauri::image::Image::new(&icon_bytes, width, height);
            let _ = tray.set_icon(Some(icon));
        }
    }

    Ok(())
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lt_tauri=debug,lt_audio=debug,lt_stt=debug,lt_llm=debug,lt_pipeline=debug,lt_output=debug,info")
        .init();

    // Load config to determine LLM processor
    let config = AppConfig::default_config_file()
        .ok()
        .and_then(|path| {
            if path.exists() {
                AppConfig::load_from_file(&path).ok()
            } else {
                None
            }
        })
        .unwrap_or_default();

    // Initialize LLM processor based on config
    let llm_processor: Arc<dyn LlmProcessor> = match config.llm_processor {
        LlmProcessorType::Gemini => {
            tracing::info!("Using Gemini CLI as LLM processor");
            Arc::new(GeminiProcessor::new())
        }
        LlmProcessorType::Copilot => {
            tracing::info!("Using Copilot CLI as LLM processor");
            Arc::new(CopilotProcessor::new())
        }
    };

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
        llm_processor.clone(),
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
            get_pipeline_state,
            get_config,
            save_config,
            set_stt_provider,
            save_api_key,
            get_stt_providers,
            get_llm_processors,
            set_llm_processor,
            set_output_mode,
            set_hotkey,
            get_dictionary,
            add_dictionary_entry,
            update_dictionary_entry,
            delete_dictionary_entry,
            search_dictionary,
            check_permissions,
            request_microphone_permission,
            open_system_preferences
        ])
        .setup(|app| {
            // Set up system tray
            let tray_icon_path = app.path().resolve("icons/32x32.png", tauri::path::BaseDirectory::Resource)
                .expect("Failed to resolve tray icon path");

            // Load icon from file
            let icon_image = image::open(&tray_icon_path)
                .expect("Failed to load tray icon")
                .to_rgba8();
            let (width, height) = icon_image.dimensions();
            let icon_bytes = icon_image.into_raw();
            let icon = tauri::image::Image::new(
                &icon_bytes,
                width,
                height
            );

            // Build initial menu
            let toggle_item = MenuItemBuilder::with_id("toggle_recording", "⏺ Start Recording")
                .build(app)?;
            let settings_item = MenuItemBuilder::with_id("open_settings", "⚙ Open Settings")
                .build(app)?;
            let overlay_item = MenuItemBuilder::with_id("toggle_overlay", "👁 Show/Hide Overlay")
                .build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit")
                .build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .item(&settings_item)
                .item(&overlay_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Create tray icon
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .tooltip("Localtype")
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    let app_handle = app.clone();
                    match event.id.as_ref() {
                        "toggle_recording" => {
                            tokio::spawn(async move {
                                let state = app_handle.state::<AppState>();
                                let is_currently_recording = {
                                    let pipeline = state.pipeline.lock().await;
                                    let current_state = pipeline.get_state().await;
                                    matches!(current_state, PipelineState::Recording | PipelineState::Transcribing)
                                };

                                if is_currently_recording {
                                    let _ = stop_pipeline(app_handle.clone(), state).await;
                                } else {
                                    let _ = start_pipeline(app_handle.clone(), state).await;
                                }
                            });
                        }
                        "open_settings" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                                let _ = window.emit("open-settings", ());
                            }
                        }
                        "toggle_overlay" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let is_visible = window.is_visible().unwrap_or(false);
                                if is_visible {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|_tray, event| {
                    if let TrayIconEvent::Click { button, .. } = event {
                        tracing::debug!("Tray icon clicked with {:?}", button);
                    }
                })
                .build(app)?;

            // Configure macOS activation policy for background mode
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                app.set_activation_policy(ActivationPolicy::Accessory);
                tracing::info!("macOS activation policy set to Accessory (background mode)");
            }

            // Perform LLM health checks
            tokio::spawn(async move {
                tracing::info!("Checking available LLM processors...");

                // Check Gemini CLI
                let gemini = GeminiProcessor::new();
                match gemini.health_check().await {
                    Ok(true) => {
                        tracing::info!("✓ Gemini CLI is available");
                    }
                    Ok(false) => {
                        tracing::warn!("⚠ Gemini CLI is not installed.");
                        tracing::warn!("  Install: https://github.com/google/generative-ai-cli");
                    }
                    Err(e) => {
                        tracing::error!("✗ Failed to check Gemini CLI: {}", e);
                    }
                }

                // Check Copilot CLI
                let copilot = CopilotProcessor::new();
                match copilot.health_check().await {
                    Ok(true) => {
                        tracing::info!("✓ Copilot CLI is available");
                    }
                    Ok(false) => {
                        tracing::warn!("⚠ Copilot CLI is not installed.");
                        tracing::warn!("  Install: npm install -g @githubnext/github-copilot-cli");
                    }
                    Err(e) => {
                        tracing::error!("✗ Failed to check Copilot CLI: {}", e);
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
