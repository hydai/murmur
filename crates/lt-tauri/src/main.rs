// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod permissions;
mod sound;

use lt_core::config::{LlmProcessorType, SttProviderType};
use lt_core::llm::LlmProcessor;
use lt_core::output::OutputMode;
use lt_core::stt::SttProvider;
use lt_core::{AppConfig, PersonalDictionary, TranscriptionHistory};
#[cfg(target_os = "macos")]
use lt_llm::AppleLlmProcessor;
use lt_llm::{CopilotProcessor, GeminiProcessor};
use lt_output::CombinedOutput;
use lt_pipeline::{PipelineEvent, PipelineOrchestrator, PipelineState};
#[cfg(target_os = "macos")]
use lt_stt::AppleSttProvider;
use lt_stt::{ElevenLabsProvider, GroqProvider, OpenAIProvider};
use std::sync::Arc;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tokio::sync::Mutex;

/// Application state using unified pipeline
#[derive(Clone)]
struct AppState {
    pipeline: Arc<Mutex<PipelineOrchestrator>>,
    event_task: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
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
fn get_status() -> String {
    "Ready".to_string()
}

#[tauri::command]
async fn get_config() -> Result<AppConfig, String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    if config_path.exists() {
        AppConfig::load_from_file(&config_path).map_err(|e| format!("Failed to load config: {}", e))
    } else {
        Ok(AppConfig::default())
    }
}

#[tauri::command]
async fn save_config(config: AppConfig) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    config
        .save_to_file(&config_path)
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
        "apple_stt" => SttProviderType::AppleStt,
        _ => return Err(format!("Unknown STT provider: {}", provider)),
    };

    config.stt_provider = provider_type;

    config
        .save_to_file(&config_path)
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

    config
        .save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
}

#[derive(Clone, serde::Serialize)]
struct SttProviderInfo {
    name: String,
    id: String,
    provider_type: String,
    configured: bool,
    requires_api_key: bool,
    model_status: Option<String>,
}

#[tauri::command]
async fn get_stt_providers() -> Result<Vec<SttProviderInfo>, String> {
    let config = get_config().await?;

    let mut providers = vec![
        SttProviderInfo {
            name: "ElevenLabs Scribe".to_string(),
            id: "elevenlabs".to_string(),
            provider_type: "streaming".to_string(),
            configured: config.api_keys.contains_key("elevenlabs"),
            requires_api_key: true,
            model_status: None,
        },
        SttProviderInfo {
            name: "OpenAI Whisper".to_string(),
            id: "openai".to_string(),
            provider_type: "batch".to_string(),
            configured: config.api_keys.contains_key("openai"),
            requires_api_key: true,
            model_status: None,
        },
        SttProviderInfo {
            name: "Groq Whisper Turbo".to_string(),
            id: "groq".to_string(),
            provider_type: "batch".to_string(),
            configured: config.api_keys.contains_key("groq"),
            requires_api_key: true,
            model_status: None,
        },
    ];

    // Add Apple STT on macOS
    #[cfg(target_os = "macos")]
    {
        let available = lt_stt::apple::is_available();
        let model_status = if !available {
            "unavailable".to_string()
        } else {
            let check_locale = resolve_apple_locale(&config.apple_stt_locale);
            match lt_stt::apple::check_model_status(&check_locale) {
                lt_stt::apple::SpeechModelStatus::Installed => "installed".to_string(),
                lt_stt::apple::SpeechModelStatus::NotInstalled => "not_installed".to_string(),
                lt_stt::apple::SpeechModelStatus::Downloading => "downloading".to_string(),
                lt_stt::apple::SpeechModelStatus::Unavailable => "unavailable".to_string(),
            }
        };

        providers.push(SttProviderInfo {
            name: "Apple Speech".to_string(),
            id: "apple_stt".to_string(),
            provider_type: "local".to_string(),
            configured: available && model_status == "installed",
            requires_api_key: false,
            model_status: Some(model_status),
        });
    }

    Ok(providers)
}

// ============================================================================
// Apple STT Commands (macOS only)
// ============================================================================

/// Resolve "auto" locale to the actual system locale (normalized with underscores).
/// Used by both `get_stt_providers` and `download_apple_stt_model` to avoid passing
/// the literal string "auto" to Swift FFI (which creates an invalid Locale).
fn resolve_apple_locale(locale: &str) -> String {
    if locale == "auto" {
        sys_locale::get_locale()
            .unwrap_or_else(|| "en_US".to_string())
            .replace('-', "_")
    } else {
        locale.to_string()
    }
}

#[tauri::command]
async fn get_apple_stt_locales() -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(lt_stt::apple::get_supported_locales())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(vec![])
    }
}

#[tauri::command]
async fn download_apple_stt_model(locale: String, app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let resolved_locale = resolve_apple_locale(&locale);
        let mut rx = lt_stt::apple::download_model(&resolved_locale);
        let app_clone = app.clone();

        tauri::async_runtime::spawn(async move {
            while let Some((progress, finished)) = rx.recv().await {
                let error = if progress == 0.0 && finished {
                    Some("Download failed or model unavailable for this locale")
                } else {
                    None
                };
                let _ = app_clone.emit(
                    "apple-stt-model-progress",
                    serde_json::json!({
                        "locale": resolved_locale,
                        "progress": progress,
                        "finished": finished,
                        "error": error
                    }),
                );
                if finished {
                    break;
                }
            }
        });

        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (locale, app);
        Err("Apple STT is only available on macOS".to_string())
    }
}

#[tauri::command]
async fn set_apple_stt_locale(locale: String) -> Result<(), String> {
    let config_path = AppConfig::default_config_file()
        .map_err(|e| format!("Failed to get config path: {}", e))?;

    let mut config = if config_path.exists() {
        AppConfig::load_from_file(&config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?
    } else {
        AppConfig::default()
    };

    config.apple_stt_locale = locale;

    config
        .save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))
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

    let mut processors = vec![
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

    #[cfg(target_os = "macos")]
    {
        processors.push(LlmProcessorInfo {
            name: "Apple Intelligence".to_string(),
            id: "apple_llm".to_string(),
            available: AppleLlmProcessor::is_available(),
        });
    }

    Ok(processors)
}

/// Create an LLM processor from its config type.
/// Shared between startup and hot-swap to avoid duplicating the factory logic.
fn create_llm_processor(processor_type: &LlmProcessorType) -> Arc<dyn LlmProcessor> {
    match processor_type {
        LlmProcessorType::Gemini => {
            tracing::info!("Using Gemini CLI as LLM processor");
            Arc::new(GeminiProcessor::new())
        }
        LlmProcessorType::Copilot => {
            tracing::info!("Using Copilot CLI as LLM processor");
            Arc::new(CopilotProcessor::new())
        }
        LlmProcessorType::AppleLlm => {
            #[cfg(target_os = "macos")]
            {
                tracing::info!("Using Apple Intelligence as LLM processor");
                Arc::new(AppleLlmProcessor::new())
            }
            #[cfg(not(target_os = "macos"))]
            {
                tracing::warn!(
                    "Apple Intelligence is only available on macOS, falling back to Gemini"
                );
                Arc::new(GeminiProcessor::new())
            }
        }
    }
}

#[tauri::command]
async fn set_llm_processor(
    processor: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
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
        "apple_llm" => LlmProcessorType::AppleLlm,
        _ => return Err(format!("Unknown LLM processor: {}", processor)),
    };

    config.llm_processor = processor_type;

    config
        .save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Hot-swap the live pipeline's LLM processor
    let new_processor = create_llm_processor(&processor_type);
    let pipeline = state.pipeline.lock().await;
    pipeline.set_llm_processor(new_processor).await;

    Ok(())
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

    config
        .save_to_file(&config_path)
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
    config
        .save_to_file(&config_path)
        .map_err(|e| format!("Failed to save config: {}", e))?;

    // Register new hotkey
    let app_handle = app.clone();
    let hotkey_str = hotkey.clone();

    // Set up the handler for the new hotkey
    app.global_shortcut()
        .on_shortcut(hotkey_str.as_str(), move |_app, _shortcut, event| {
            // Only process key PRESS, not release
            if event.state != ShortcutState::Pressed {
                return;
            }
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let is_currently_recording = {
                    let pipeline = state.pipeline.lock().await;
                    let current_state = pipeline.get_state().await;
                    matches!(
                        current_state,
                        PipelineState::Recording | PipelineState::Transcribing
                    )
                };

                if is_currently_recording {
                    let _ = stop_pipeline(handle.clone(), state).await;
                } else {
                    let _ = start_pipeline(handle.clone(), state).await;
                }
            });
        })
        .map_err(|e| format!("Failed to set hotkey handler: {}", e))?;

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
    match current_state {
        PipelineState::Recording | PipelineState::Transcribing | PipelineState::Processing => {
            return Err(format!(
                "Pipeline is already running (state: {:?})",
                current_state
            ));
        }
        _ => {} // Idle, Done, Error are all acceptable starting states
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
            let api_key = config
                .api_keys
                .get("elevenlabs")
                .ok_or_else(|| {
                    "ElevenLabs API key not configured. Please add your API key in Settings"
                        .to_string()
                })?
                .clone();
            Box::new(ElevenLabsProvider::new(api_key))
        }
        SttProviderType::OpenAI => {
            let api_key = config
                .api_keys
                .get("openai")
                .ok_or_else(|| {
                    "OpenAI API key not configured. Please add your API key in Settings".to_string()
                })?
                .clone();
            Box::new(OpenAIProvider::new(api_key))
        }
        SttProviderType::Groq => {
            let api_key = config
                .api_keys
                .get("groq")
                .ok_or_else(|| {
                    "Groq API key not configured. Please add your API key in Settings".to_string()
                })?
                .clone();
            Box::new(GroqProvider::new(api_key))
        }
        SttProviderType::AppleStt => {
            #[cfg(target_os = "macos")]
            {
                Box::new(AppleSttProvider::new(config.apple_stt_locale.clone()))
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err("Apple STT is only available on macOS 26+".to_string());
            }
        }
    };

    // Subscribe to pipeline events before starting
    let mut event_rx = pipeline.subscribe_events();
    let app_clone = app.clone();

    // Spawn task to forward pipeline events to frontend
    let event_task = tauri::async_runtime::spawn(async move {
        // Track raw transcription and command for history
        let mut raw_transcription = String::new();
        let mut detected_command: Option<String> = None;

        while let Ok(event) = event_rx.recv().await {
            match event {
                PipelineEvent::StateChanged {
                    state,
                    timestamp_ms,
                } => {
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

                    // Emit as transcription-processed for compatibility
                    let _ = app_clone.emit(
                        "transcription-processed",
                        serde_json::json!({
                            "text": text,
                            "processing_time_ms": processing_time_ms
                        }),
                    );

                    // Save to history
                    let timestamp_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let raw = std::mem::take(&mut raw_transcription);
                    let cmd = detected_command.take();
                    let entry = lt_core::history::HistoryEntry {
                        id: timestamp_ms.to_string(),
                        final_text: text,
                        raw_text: if raw.is_empty() { None } else { Some(raw) },
                        timestamp_ms,
                        processing_time_ms,
                        command_name: cmd,
                    };
                    if let Ok(config_dir) = AppConfig::default_config_dir() {
                        let history_path = config_dir.join("history.json");
                        let mut history = if history_path.exists() {
                            TranscriptionHistory::load_from_file(&history_path).unwrap_or_default()
                        } else {
                            TranscriptionHistory::new()
                        };
                        history.add_entry(entry);
                        if let Err(e) = history.save_to_file(&history_path) {
                            tracing::warn!("Failed to save history: {}", e);
                        }
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
    });

    *state.event_task.lock().await = Some(event_task);

    // Start the pipeline
    pipeline.start(stt).await.map_err(|e| {
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

    pipeline.stop().await.map_err(|e| {
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

    Ok(matches!(
        current_state,
        PipelineState::Recording | PipelineState::Transcribing
    ))
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
async fn search_dictionary(
    query: String,
) -> Result<Vec<lt_core::dictionary::DictionaryEntry>, String> {
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

#[tauri::command]
async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    // If settings window already exists, just focus it
    if let Some(window) = app.get_webview_window("settings") {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Create new settings window
    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html?view=settings".into()),
    )
    .title("Murmur Settings")
    .inner_size(720.0, 560.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ============================================================================
// History Commands
// ============================================================================

#[tauri::command]
async fn get_history(
    offset: usize,
    limit: usize,
) -> Result<Vec<lt_core::history::HistoryEntry>, String> {
    let history_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("history.json");

    let history = if history_path.exists() {
        TranscriptionHistory::load_from_file(&history_path)
            .map_err(|e| format!("Failed to load history: {}", e))?
    } else {
        TranscriptionHistory::new()
    };

    let entries: Vec<_> = history
        .entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    Ok(entries)
}

#[tauri::command]
async fn search_history(query: String) -> Result<Vec<lt_core::history::HistoryEntry>, String> {
    let history_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("history.json");

    let history = if history_path.exists() {
        TranscriptionHistory::load_from_file(&history_path)
            .map_err(|e| format!("Failed to load history: {}", e))?
    } else {
        TranscriptionHistory::new()
    };

    Ok(history.search_entries(&query))
}

#[tauri::command]
async fn delete_history_entry(id: String) -> Result<(), String> {
    let history_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("history.json");

    let mut history = if history_path.exists() {
        TranscriptionHistory::load_from_file(&history_path)
            .map_err(|e| format!("Failed to load history: {}", e))?
    } else {
        return Err("History file not found".to_string());
    };

    if !history.delete_entry(&id) {
        return Err(format!("History entry '{}' not found", id));
    }

    history
        .save_to_file(&history_path)
        .map_err(|e| format!("Failed to save history: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn clear_history() -> Result<(), String> {
    let history_path = AppConfig::default_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?
        .join("history.json");

    let mut history = if history_path.exists() {
        TranscriptionHistory::load_from_file(&history_path)
            .map_err(|e| format!("Failed to load history: {}", e))?
    } else {
        TranscriptionHistory::new()
    };

    history.clear();
    history
        .save_to_file(&history_path)
        .map_err(|e| format!("Failed to save history: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn open_history_window(app: tauri::AppHandle) -> Result<(), String> {
    // If history window already exists, just focus it
    if let Some(window) = app.get_webview_window("history") {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Create new history window
    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        "history",
        tauri::WebviewUrl::App("index.html?view=history".into()),
    )
    .title("Murmur History")
    .inner_size(720.0, 560.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
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
fn rebuild_tray_menu(
    app: &tauri::AppHandle,
    is_recording: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app.tray_by_id("main-tray").ok_or("Tray not found")?;

    // Build menu items
    let toggle_item = MenuItemBuilder::with_id(
        "toggle_recording",
        if is_recording {
            "⏸ Stop Recording"
        } else {
            "⏺ Start Recording"
        },
    )
    .build(app)?;

    let settings_item = MenuItemBuilder::with_id("open_settings", "⚙ Open Settings").build(app)?;
    let history_item = MenuItemBuilder::with_id("open_history", "📋 History").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&toggle_item)
        .item(&settings_item)
        .item(&history_item)
        .separator()
        .item(&quit_item)
        .build()?;

    tray.set_menu(Some(menu))?;

    // Update tooltip to reflect recording state
    let tooltip = if is_recording {
        "Murmur - Recording"
    } else {
        "Murmur"
    };
    tray.set_tooltip(Some(tooltip))?;

    // Update icon to reflect recording state using embedded icon
    let icon_png_bytes = include_bytes!("../icons/32x32.png");
    if let Ok(icon_image) = image::load_from_memory(icon_png_bytes) {
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

    let is_first_launch = AppConfig::default_config_file()
        .map(|path| !path.exists())
        .unwrap_or(false);

    let startup_hotkey = config.hotkey.clone();

    // Initialize LLM processor based on config
    let llm_processor = create_llm_processor(&config.llm_processor);

    // Load dictionary (or create empty if not exists)
    let dictionary = {
        let dict_path = AppConfig::default_config_dir()
            .ok()
            .map(|dir| dir.join("dictionary.json"));

        if let Some(path) = dict_path.as_ref() {
            if path.exists() {
                match PersonalDictionary::load_from_file(path) {
                    Ok(dict) => {
                        tracing::info!(
                            "Loaded personal dictionary with {} entries",
                            dict.entries.len()
                        );
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
    let output_sink = match CombinedOutput::new(OutputMode::Clipboard) {
        Ok(output) => Arc::new(output),
        Err(e) => {
            eprintln!("Fatal: Failed to initialize output sink: {e}");
            std::process::exit(1);
        }
    };

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
            open_settings_window,
            get_history,
            search_history,
            delete_history_entry,
            clear_history,
            open_history_window,
            check_permissions,
            request_microphone_permission,
            open_system_preferences,
            get_apple_stt_locales,
            download_apple_stt_model,
            set_apple_stt_locale
        ])
        .setup(move |app| {
            // Set up system tray - embed icon at compile time to avoid runtime path issues
            let icon_png_bytes = include_bytes!("../icons/32x32.png");
            let icon_image = match image::load_from_memory(icon_png_bytes) {
                Ok(img) => img.to_rgba8(),
                Err(e) => {
                    eprintln!("Fatal: Failed to decode embedded tray icon: {e}");
                    std::process::exit(1);
                }
            };
            let (width, height) = icon_image.dimensions();
            let icon_bytes = icon_image.into_raw();
            let icon = tauri::image::Image::new(&icon_bytes, width, height);

            // Build initial menu
            let toggle_item =
                MenuItemBuilder::with_id("toggle_recording", "⏺ Start Recording").build(app)?;
            let settings_item =
                MenuItemBuilder::with_id("open_settings", "⚙ Open Settings").build(app)?;
            let history_item = MenuItemBuilder::with_id("open_history", "📋 History").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .item(&settings_item)
                .item(&history_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Create tray icon
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .tooltip("Murmur")
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    let app_handle = app.clone();
                    match event.id.as_ref() {
                        "toggle_recording" => {
                            tauri::async_runtime::spawn(async move {
                                let state = app_handle.state::<AppState>();
                                let is_currently_recording = {
                                    let pipeline = state.pipeline.lock().await;
                                    let current_state = pipeline.get_state().await;
                                    matches!(
                                        current_state,
                                        PipelineState::Recording | PipelineState::Transcribing
                                    )
                                };

                                if is_currently_recording {
                                    let _ = stop_pipeline(app_handle.clone(), state).await;
                                } else {
                                    let _ = start_pipeline(app_handle.clone(), state).await;
                                }
                            });
                        }
                        "open_settings" => {
                            let handle = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = open_settings_window(handle).await {
                                    tracing::warn!("Failed to open settings window: {}", e);
                                }
                            });
                        }
                        "open_history" => {
                            let handle = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = open_history_window(handle).await {
                                    tracing::warn!("Failed to open history window: {}", e);
                                }
                            });
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
            tauri::async_runtime::spawn(async move {
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

            // Register the shortcut handler (on_shortcut registers internally)
            if let Err(e) = app.global_shortcut().on_shortcut(
                startup_hotkey.as_str(),
                move |_app, _shortcut, event| {
                    // Only process key PRESS, not release
                    if event.state != ShortcutState::Pressed {
                        return;
                    }

                    // Toggle pipeline using the cloned handle
                    let handle = app_handle.clone();

                    tauri::async_runtime::spawn(async move {
                        let state = handle.state::<AppState>();

                        let is_currently_recording = {
                            let pipeline = state.pipeline.lock().await;
                            let current_state = pipeline.get_state().await;
                            matches!(
                                current_state,
                                PipelineState::Recording | PipelineState::Transcribing
                            )
                        };

                        if is_currently_recording {
                            // Stop pipeline
                            let _ = stop_pipeline(handle.clone(), state).await;
                        } else {
                            // Start pipeline
                            let _ = start_pipeline(handle.clone(), state).await;
                        }
                    });
                },
            ) {
                tracing::warn!("Failed to set up shortcut handler: {}", e);
            }
            // Note: on_shortcut() internally registers the shortcut, so no
            // separate register() call is needed.

            if is_first_launch {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    if let Err(e) = open_settings_window(handle).await {
                        tracing::warn!("Failed to auto-open settings on first launch: {}", e);
                    }
                });
                tracing::info!("First launch detected — opening settings window");
            }

            tracing::info!("Murmur started successfully with unified pipeline");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
