// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod diagnostics;
mod events;
mod permissions;
mod recording;
mod shortcuts;
mod sound;
mod storage;

use lt_core::config::{LlmProcessorType, SttProviderType};
use lt_core::llm::LlmProcessor;
use lt_core::output::OutputMode;
use lt_core::stt::SttProvider;
use lt_core::{AppConfig, PersonalDictionary};
#[cfg(target_os = "macos")]
use lt_llm::AppleLlmProcessor;
use lt_llm::{
    CopilotProcessor, GeminiProcessor, HttpLlmProcessor, PromptManager, PromptName, PromptSet,
    PromptStore,
};
use lt_output::CombinedOutput;
use lt_pipeline::{PipelineOrchestrator, PipelineState};
#[cfg(target_os = "macos")]
use lt_stt::AppleSttProvider;
use lt_stt::{CustomSttProvider, ElevenLabsProvider, GroqProvider, OpenAIProvider};
use std::sync::Arc;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Application state using unified pipeline
#[derive(Clone)]
struct AppState {
    pipeline: Arc<Mutex<PipelineOrchestrator>>,
    store: storage::AppStore,
    hotkey_updates: Arc<Mutex<()>>,
    prompts: PromptManager,
}

fn nonempty(value: String) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

fn register_recording_shortcut(app: &tauri::AppHandle, shortcut: Shortcut) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<AppState>();
                let result = toggle_recording(app.clone(), state).await;
                if let Err(message) = result {
                    tracing::warn!("Shortcut action failed: {message}");
                    let _ = app.emit(
                        "pipeline-error",
                        serde_json::json!({ "message": message, "recoverable": true }),
                    );
                }
            });
        })
        .map_err(|error| format!("Failed to register shortcut: {error}"))
}

impl shortcuts::Registry for tauri::AppHandle {
    fn register(&self, shortcut: Shortcut) -> Result<(), String> {
        register_recording_shortcut(self, shortcut)
    }
    fn unregister(&self, shortcut: Shortcut) -> Result<(), String> {
        self.global_shortcut()
            .unregister(shortcut)
            .map_err(|error| error.to_string())
    }
    fn is_registered(&self, shortcut: Shortcut) -> bool {
        self.global_shortcut().is_registered(shortcut)
    }
}

async fn update_shortcut(
    app: tauri::AppHandle,
    updates: tokio::sync::OwnedMutexGuard<()>,
    old: &str,
    new: &str,
    persist: impl std::future::Future<Output = Result<(), String>> + Send + 'static,
) -> Result<(), String> {
    let new = new
        .parse::<Shortcut>()
        .map_err(|error| format!("Invalid shortcut: {error}"))?;
    shortcuts::replace_detached(app, updates, old.parse().ok(), new, persist).await
}

#[tauri::command]
async fn get_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    // Secrets never reach the webview; providers report `configured` instead.
    Ok(state.store.config.read().await?.redacted())
}

#[tauri::command]
async fn save_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let updates = state.hotkey_updates.clone().lock_owned().await;
    let old = state.store.config.read().await?.hotkey;
    let new = config.hotkey.clone();
    let store = state.store.config.clone();
    update_shortcut(app, updates, &old, &new, async move {
        store
            .update(move |current| {
                // The caller only ever holds a redacted copy.
                current.apply_redacted(config);
                Ok(())
            })
            .await
            .map(|_| ())
    })
    .await
}

#[tauri::command]
async fn get_diagnostic_logs() -> Vec<diagnostics::DiagnosticLogEntry> {
    diagnostics::shared_diagnostic_log_store().entries()
}

#[tauri::command]
async fn clear_diagnostic_logs() -> Result<(), String> {
    diagnostics::shared_diagnostic_log_store().clear();
    Ok(())
}

#[tauri::command]
async fn set_stt_provider(
    provider: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let provider: SttProviderType = provider.parse()?;
    state
        .store
        .config
        .update(move |config| {
            config.stt_provider = provider;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn save_api_key(
    provider: String,
    api_key: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            if api_key.trim().is_empty() {
                config.api_keys.remove(&provider.to_lowercase());
            } else {
                config.api_keys.insert(provider.to_lowercase(), api_key);
            }
            Ok(())
        })
        .await
        .map(|_| ())
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

/// Whether the provider has everything it needs for the next recording.
/// Apple's model availability is layered on separately, since it lives in
/// lt-stt rather than the config.
fn stt_provider_configured(provider: SttProviderType, config: &AppConfig) -> bool {
    match provider {
        // The base URL is what makes the endpoint usable; a local Whisper
        // server usually needs no key.
        SttProviderType::CustomStt => config.http_stt_config.custom_base_url.is_some(),
        SttProviderType::AppleStt => true,
        other => other
            .api_key_name()
            .is_some_and(|key| config.api_keys.contains_key(key)),
    }
}

#[tauri::command]
async fn get_stt_providers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SttProviderInfo>, String> {
    let config = state.store.config.read().await?;

    let mut providers = Vec::new();
    for provider in SttProviderType::ALL {
        if provider.is_macos_only() && !cfg!(target_os = "macos") {
            continue;
        }

        let mut configured = stt_provider_configured(provider, &config);
        let mut model_status = None;

        #[cfg(target_os = "macos")]
        if provider == SttProviderType::AppleStt {
            let status = if lt_stt::apple::is_available() {
                let locale = resolve_apple_locale(&config.apple_stt_locale);
                match lt_stt::apple::check_model_status(&locale) {
                    lt_stt::apple::SpeechModelStatus::Installed => "installed",
                    lt_stt::apple::SpeechModelStatus::NotInstalled => "not_installed",
                    lt_stt::apple::SpeechModelStatus::Downloading => "downloading",
                    lt_stt::apple::SpeechModelStatus::Unavailable => "unavailable",
                }
            } else {
                "unavailable"
            };
            configured = status == "installed";
            model_status = Some(status.to_string());
        }

        providers.push(SttProviderInfo {
            name: match provider {
                // The user names their own endpoint.
                SttProviderType::CustomStt => config
                    .http_stt_config
                    .custom_display_name
                    .clone()
                    .unwrap_or_else(|| provider.display_name().to_string()),
                other => other.display_name().to_string(),
            },
            id: provider.id().to_string(),
            provider_type: provider.kind().as_str().to_string(),
            configured,
            requires_api_key: provider.api_key_name().is_some(),
            model_status,
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
async fn set_apple_stt_locale(
    locale: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.apple_stt_locale = locale;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn get_elevenlabs_languages() -> Result<Vec<(String, String)>, String> {
    // Full list of ElevenLabs Scribe v2 supported languages (ISO 639-3 codes)
    Ok(vec![
        ("auto".to_string(), "Auto-detect".to_string()),
        ("afr".to_string(), "Afrikaans".to_string()),
        ("amh".to_string(), "Amharic".to_string()),
        ("ara".to_string(), "Arabic".to_string()),
        ("hye".to_string(), "Armenian".to_string()),
        ("asm".to_string(), "Assamese".to_string()),
        ("ast".to_string(), "Asturian".to_string()),
        ("aze".to_string(), "Azerbaijani".to_string()),
        ("bel".to_string(), "Belarusian".to_string()),
        ("ben".to_string(), "Bengali".to_string()),
        ("bos".to_string(), "Bosnian".to_string()),
        ("bul".to_string(), "Bulgarian".to_string()),
        ("mya".to_string(), "Burmese".to_string()),
        ("yue".to_string(), "Cantonese".to_string()),
        ("cat".to_string(), "Catalan".to_string()),
        ("ceb".to_string(), "Cebuano".to_string()),
        ("nya".to_string(), "Chichewa".to_string()),
        ("hrv".to_string(), "Croatian".to_string()),
        ("ces".to_string(), "Czech".to_string()),
        ("dan".to_string(), "Danish".to_string()),
        ("nld".to_string(), "Dutch".to_string()),
        ("eng".to_string(), "English".to_string()),
        ("est".to_string(), "Estonian".to_string()),
        ("fil".to_string(), "Filipino".to_string()),
        ("fin".to_string(), "Finnish".to_string()),
        ("fra".to_string(), "French".to_string()),
        ("ful".to_string(), "Fulah".to_string()),
        ("glg".to_string(), "Galician".to_string()),
        ("lug".to_string(), "Ganda".to_string()),
        ("kat".to_string(), "Georgian".to_string()),
        ("deu".to_string(), "German".to_string()),
        ("ell".to_string(), "Greek".to_string()),
        ("guj".to_string(), "Gujarati".to_string()),
        ("hau".to_string(), "Hausa".to_string()),
        ("heb".to_string(), "Hebrew".to_string()),
        ("hin".to_string(), "Hindi".to_string()),
        ("hun".to_string(), "Hungarian".to_string()),
        ("isl".to_string(), "Icelandic".to_string()),
        ("ibo".to_string(), "Igbo".to_string()),
        ("ind".to_string(), "Indonesian".to_string()),
        ("gle".to_string(), "Irish".to_string()),
        ("ita".to_string(), "Italian".to_string()),
        ("jpn".to_string(), "Japanese".to_string()),
        ("jav".to_string(), "Javanese".to_string()),
        ("kea".to_string(), "Kabuverdianu".to_string()),
        ("kan".to_string(), "Kannada".to_string()),
        ("kaz".to_string(), "Kazakh".to_string()),
        ("khm".to_string(), "Khmer".to_string()),
        ("kor".to_string(), "Korean".to_string()),
        ("kur".to_string(), "Kurdish".to_string()),
        ("kir".to_string(), "Kyrgyz".to_string()),
        ("lao".to_string(), "Lao".to_string()),
        ("lav".to_string(), "Latvian".to_string()),
        ("lin".to_string(), "Lingala".to_string()),
        ("lit".to_string(), "Lithuanian".to_string()),
        ("luo".to_string(), "Luo".to_string()),
        ("ltz".to_string(), "Luxembourgish".to_string()),
        ("mkd".to_string(), "Macedonian".to_string()),
        ("msa".to_string(), "Malay".to_string()),
        ("mal".to_string(), "Malayalam".to_string()),
        ("mlt".to_string(), "Maltese".to_string()),
        ("zho".to_string(), "Mandarin Chinese".to_string()),
        ("mri".to_string(), "Māori".to_string()),
        ("mar".to_string(), "Marathi".to_string()),
        ("mon".to_string(), "Mongolian".to_string()),
        ("nep".to_string(), "Nepali".to_string()),
        ("nso".to_string(), "Northern Sotho".to_string()),
        ("nor".to_string(), "Norwegian".to_string()),
        ("oci".to_string(), "Occitan".to_string()),
        ("ori".to_string(), "Odia".to_string()),
        ("pus".to_string(), "Pashto".to_string()),
        ("fas".to_string(), "Persian".to_string()),
        ("pol".to_string(), "Polish".to_string()),
        ("por".to_string(), "Portuguese".to_string()),
        ("pan".to_string(), "Punjabi".to_string()),
        ("ron".to_string(), "Romanian".to_string()),
        ("rus".to_string(), "Russian".to_string()),
        ("srp".to_string(), "Serbian".to_string()),
        ("sna".to_string(), "Shona".to_string()),
        ("snd".to_string(), "Sindhi".to_string()),
        ("slk".to_string(), "Slovak".to_string()),
        ("slv".to_string(), "Slovenian".to_string()),
        ("som".to_string(), "Somali".to_string()),
        ("spa".to_string(), "Spanish".to_string()),
        ("swa".to_string(), "Swahili".to_string()),
        ("swe".to_string(), "Swedish".to_string()),
        ("tgk".to_string(), "Tajik".to_string()),
        ("tam".to_string(), "Tamil".to_string()),
        ("tel".to_string(), "Telugu".to_string()),
        ("tha".to_string(), "Thai".to_string()),
        ("tur".to_string(), "Turkish".to_string()),
        ("ukr".to_string(), "Ukrainian".to_string()),
        ("umb".to_string(), "Umbundu".to_string()),
        ("urd".to_string(), "Urdu".to_string()),
        ("uzb".to_string(), "Uzbek".to_string()),
        ("vie".to_string(), "Vietnamese".to_string()),
        ("cym".to_string(), "Welsh".to_string()),
        ("wol".to_string(), "Wolof".to_string()),
        ("xho".to_string(), "Xhosa".to_string()),
        ("zul".to_string(), "Zulu".to_string()),
    ])
}

#[tauri::command]
async fn set_elevenlabs_language(
    language: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.elevenlabs_language = language;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[derive(Clone, serde::Serialize)]
struct LlmProcessorInfo {
    name: String,
    id: String,
    available: bool,
    default_model: String,
    provider_type: String,
    requires_api_key: bool,
    configured: bool,
    api_key_name: Option<String>,
}

/// Whether the processor has everything it needs for the next recording.
/// CLI and on-device availability is layered on separately, since probing the
/// binary is async and platform-specific.
fn llm_processor_configured(processor: LlmProcessorType, config: &AppConfig) -> bool {
    match processor {
        // The base URL is what makes the endpoint usable; a local server
        // usually needs no key.
        LlmProcessorType::CustomApi => config.http_llm_config.custom_base_url.is_some(),
        other => other
            .api_key_name()
            .is_none_or(|key| config.api_keys.contains_key(key)),
    }
}

#[tauri::command]
async fn get_llm_processors(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LlmProcessorInfo>, String> {
    let config = state.store.config.read().await?;

    // Probe both CLIs at once: a missing binary costs a process spawn each.
    let gemini = GeminiProcessor::new();
    let copilot = CopilotProcessor::new();
    let (gemini_available, copilot_available) =
        tokio::join!(gemini.health_check(), copilot.health_check());
    let gemini_available = gemini_available.unwrap_or(false);
    let copilot_available = copilot_available.unwrap_or(false);

    let mut processors = Vec::new();
    for processor in LlmProcessorType::ALL {
        if processor.is_macos_only() && !cfg!(target_os = "macos") {
            continue;
        }

        let configured = llm_processor_configured(processor, &config);
        let available = match processor {
            LlmProcessorType::Gemini => gemini_available,
            LlmProcessorType::Copilot => copilot_available,
            #[cfg(target_os = "macos")]
            LlmProcessorType::AppleLlm => AppleLlmProcessor::is_available(),
            #[cfg(not(target_os = "macos"))]
            LlmProcessorType::AppleLlm => false,
            _ => configured,
        };

        processors.push(LlmProcessorInfo {
            name: match processor {
                // The user names their own endpoint.
                LlmProcessorType::CustomApi => config
                    .http_llm_config
                    .custom_display_name
                    .clone()
                    .unwrap_or_else(|| processor.display_name().to_string()),
                other => other.display_name().to_string(),
            },
            id: processor.id().to_string(),
            available,
            default_model: lt_llm::default_model(processor).to_string(),
            provider_type: processor.kind().as_str().to_string(),
            requires_api_key: processor.requires_api_key(),
            configured,
            api_key_name: processor.api_key_name().map(str::to_owned),
        });
    }

    Ok(processors)
}

/// Read a required API key, rejecting blanks so a missing key fails with an
/// actionable message instead of a provider-side 401.
fn require_api_key(config: &AppConfig, provider: &str, label: &str) -> Result<String, String> {
    config
        .api_keys
        .get(provider)
        .map(|key| key.trim())
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            format!("{label} API key not configured. Please add your API key in Settings")
        })
}

/// Create an LLM processor from its config type and optional model override.
/// Shared between startup and recording configuration. Each recording receives
/// an independent prompt snapshot so edits apply to the next recording.
///
/// Missing required configuration is an error rather than a silent default: an
/// empty key would reach the provider as `Authorization: Bearer `, and a custom
/// endpoint without a base URL would post the transcript to whatever happens to
/// be listening on a fallback address.
fn create_llm_processor(
    processor_type: &LlmProcessorType,
    model: Option<String>,
    config: &AppConfig,
    prompts: &PromptManager,
) -> Result<Arc<dyn LlmProcessor>, String> {
    Ok(match processor_type {
        LlmProcessorType::Gemini => {
            tracing::info!("Using Gemini CLI as LLM processor");
            Arc::new(GeminiProcessor::with_model_and_prompts(
                model,
                prompts.clone(),
            ))
        }
        LlmProcessorType::Copilot => {
            tracing::info!("Using Copilot CLI as LLM processor");
            Arc::new(CopilotProcessor::with_model_and_prompts(
                model,
                prompts.clone(),
            ))
        }
        LlmProcessorType::AppleLlm => {
            #[cfg(target_os = "macos")]
            {
                tracing::info!("Using Apple Intelligence as LLM processor");
                Arc::new(AppleLlmProcessor::with_model_and_prompts(
                    model,
                    prompts.clone(),
                ))
            }
            #[cfg(not(target_os = "macos"))]
            {
                tracing::warn!(
                    "Apple Intelligence is only available on macOS, falling back to Gemini"
                );
                Arc::new(GeminiProcessor::with_model_and_prompts(
                    model,
                    prompts.clone(),
                ))
            }
        }
        LlmProcessorType::OpenAiApi => {
            let api_key = require_api_key(config, "openai", "OpenAI")?;
            tracing::info!("Using OpenAI API as LLM processor");
            Arc::new(HttpLlmProcessor::openai_with_prompts(
                api_key,
                model,
                prompts.clone(),
            ))
        }
        LlmProcessorType::ClaudeApi => {
            let api_key = require_api_key(config, "anthropic", "Claude")?;
            tracing::info!("Using Claude API as LLM processor");
            Arc::new(HttpLlmProcessor::claude_with_prompts(
                api_key,
                model,
                prompts.clone(),
            ))
        }
        LlmProcessorType::GeminiApi => {
            let api_key = require_api_key(config, "google_ai", "Gemini")?;
            tracing::info!("Using Gemini API as LLM processor");
            Arc::new(HttpLlmProcessor::gemini_api_with_prompts(
                api_key,
                model,
                prompts.clone(),
            ))
        }
        LlmProcessorType::CustomApi => {
            // A local server needs no auth, so only the base URL is required.
            let api_key = config
                .api_keys
                .get("custom_llm")
                .cloned()
                .unwrap_or_default();
            let base_url = config
                .http_llm_config
                .custom_base_url
                .clone()
                .ok_or_else(|| {
                    "Custom LLM endpoint not configured. Please set a base URL in Settings"
                        .to_string()
                })?;
            tracing::info!("Using custom endpoint as LLM processor");
            Arc::new(HttpLlmProcessor::custom_with_prompts(
                base_url,
                api_key,
                model,
                prompts.clone(),
            ))
        }
    })
}

#[tauri::command]
async fn set_llm_processor(
    processor: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let processor: LlmProcessorType = processor.parse()?;
    state
        .store
        .config
        .update(move |config| {
            config.llm_processor = processor;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_llm_model(model: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.llm_model = nonempty(model);
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_custom_llm_endpoint(
    base_url: String,
    display_name: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.http_llm_config.custom_base_url = nonempty(base_url);
            config.http_llm_config.custom_display_name = display_name.and_then(nonempty);
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_custom_stt_endpoint(
    base_url: String,
    display_name: Option<String>,
    model: Option<String>,
    language: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.http_stt_config.custom_base_url = nonempty(base_url);
            config.http_stt_config.custom_display_name = display_name.and_then(nonempty);
            config.http_stt_config.custom_model = model.and_then(nonempty);
            config.http_stt_config.language = language.and_then(nonempty);
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_output_mode(mode: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mode = match mode.to_lowercase().as_str() {
        "clipboard" => OutputMode::Clipboard,
        "keyboard" => OutputMode::Keyboard,
        "both" => OutputMode::Both,
        _ => return Err(format!("Unknown output mode: {mode}")),
    };
    state
        .store
        .config
        .update(move |config| {
            config.output_mode = mode;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_chinese_conversion(
    mode: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let conversion = match mode.to_lowercase().as_str() {
        "traditional" => lt_core::config::ChineseConversion::Traditional,
        "none" => lt_core::config::ChineseConversion::None,
        _ => return Err(format!("Unknown Chinese conversion mode: {mode}")),
    };
    state
        .store
        .config
        .update(move |config| {
            config.chinese_conversion = conversion;
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn set_save_history(enabled: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .config
        .update(move |config| {
            config.save_history = enabled;
            Ok(())
        })
        .await?;
    state.store.history.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
async fn set_hotkey(
    hotkey: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let updates = state.hotkey_updates.clone().lock_owned().await;
    let old = state.store.config.read().await?.hotkey;
    let store = state.store.config.clone();
    let persisted = hotkey.clone();
    update_shortcut(app, updates, &old, &hotkey, async move {
        store
            .update(move |config| {
                config.hotkey = persisted;
                Ok(())
            })
            .await
            .map(|_| ())
    })
    .await
}

#[tauri::command]
async fn start_pipeline(
    _app: tauri::AppHandle,
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

    let config = state.store.config.read().await?;

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
            Box::new(ElevenLabsProvider::with_config(
                api_key,
                "scribe_v2_realtime".to_string(),
                config.elevenlabs_language.clone(),
            ))
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
        SttProviderType::CustomStt => {
            let base_url = config
                .http_stt_config
                .custom_base_url
                .clone()
                .ok_or_else(|| {
                    "Custom STT endpoint not configured. Please set a base URL in Settings"
                        .to_string()
                })?;
            let api_key = config.api_keys.get("custom_stt").cloned();
            Box::new(CustomSttProvider::new(
                base_url,
                api_key,
                config.http_stt_config.custom_model.clone(),
                config.http_stt_config.language.clone(),
            ))
        }
    };

    // Apply one coherent persisted configuration snapshot to the next recording.
    let prompts = PromptManager::from_set(state.prompts.shared().read().await.clone());
    pipeline
        .set_llm_processor(create_llm_processor(
            &config.llm_processor,
            config.llm_model.clone(),
            &config,
            &prompts,
        )?)
        .await;
    let output = CombinedOutput::new(config.output_mode)
        .map_err(|error| format!("Failed to initialize output: {error}"))?;
    pipeline.set_output_sink(Arc::new(output)).await;
    pipeline
        .set_chinese_conversion(config.chinese_conversion)
        .await;
    *pipeline.get_dictionary().lock().await = state.store.dictionary.read().await?;

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

/// Shared by the hotkey, the tray, and the overlay button.
#[tauri::command]
async fn toggle_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let action = {
        let pipeline = state.pipeline.lock().await;
        recording::toggle_action(pipeline.get_state().await, pipeline.is_capturing().await)
    };
    match action {
        recording::ToggleAction::Start => start_pipeline(app, state).await,
        recording::ToggleAction::Stop => stop_pipeline(app, state).await,
        recording::ToggleAction::Cancel => {
            tracing::info!("Cancelling pipeline");
            state
                .pipeline
                .lock()
                .await
                .reset()
                .await
                .map_err(|e| format!("Failed to cancel pipeline: {e}"))
        }
    }
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
async fn get_dictionary(state: tauri::State<'_, AppState>) -> Result<PersonalDictionary, String> {
    state.store.dictionary.read().await
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
    state
        .store
        .dictionary
        .update(move |dict| {
            dict.add_entry(lt_core::DictionaryEntry {
                term: params.term,
                aliases: params.aliases,
                description: params.description,
            });
            Ok(())
        })
        .await
        .map(|_| ())
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
    state
        .store
        .dictionary
        .update(move |dict| {
            if !dict.update_entry(
                &params.old_term,
                lt_core::DictionaryEntry {
                    term: params.term,
                    aliases: params.aliases,
                    description: params.description,
                },
            ) {
                return Err(format!("Entry '{}' not found", params.old_term));
            }
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn delete_dictionary_entry(
    term: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .dictionary
        .update(move |dict| {
            if !dict.remove_entry(&term) {
                return Err(format!("Entry '{term}' not found"));
            }
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn search_dictionary(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<lt_core::DictionaryEntry>, String> {
    Ok(state.store.dictionary.read().await?.search_entries(&query))
}

// Prompt template management commands

#[derive(serde::Serialize)]
struct PromptInfo {
    name: PromptName,
    title: &'static str,
    description: &'static str,
    required_placeholders: &'static [&'static str],
    task_variant: &'static str,
    content: String,
    is_override: bool,
    default_content: String,
}

#[tauri::command]
async fn get_prompts(state: tauri::State<'_, AppState>) -> Result<Vec<PromptInfo>, String> {
    let set = state.prompts.shared();
    let guard = set.read().await;
    Ok(PromptName::ALL
        .iter()
        .map(|&name| PromptInfo {
            name,
            title: name.display_title(),
            description: name.description(),
            required_placeholders: name.required_placeholders(),
            task_variant: name.task_variant_label(),
            content: guard.get(name).to_string(),
            is_override: guard.has_override(name),
            default_content: name.default_template().to_string(),
        })
        .collect())
}

#[derive(serde::Deserialize)]
struct SetPromptParams {
    name: PromptName,
    content: String,
}

#[tauri::command]
async fn set_prompt(
    params: SetPromptParams,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if params.content.trim().is_empty() {
        return Err("Prompt content cannot be empty".to_owned());
    }
    let config_dir = AppConfig::default_config_dir().map_err(|error| error.to_string())?;
    let mut prompts = state.prompts.shared().write_owned().await;
    tokio::task::spawn_blocking(move || {
        PromptStore::save(&config_dir, params.name, &params.content)
            .map_err(|error| format!("Failed to save prompt: {error}"))?;
        prompts.set_override(params.name, params.content);
        Ok(())
    })
    .await
    .map_err(|error| format!("Prompt storage task failed: {error}"))?
}

#[derive(serde::Deserialize)]
struct ResetPromptParams {
    name: PromptName,
}

#[tauri::command]
async fn reset_prompt(
    params: ResetPromptParams,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let config_dir = AppConfig::default_config_dir().map_err(|error| error.to_string())?;
    let mut prompts = state.prompts.shared().write_owned().await;
    tokio::task::spawn_blocking(move || {
        PromptStore::reset(&config_dir, params.name)
            .map_err(|error| format!("Failed to reset prompt: {error}"))?;
        prompts.clear_override(params.name);
        Ok(())
    })
    .await
    .map_err(|error| format!("Prompt storage task failed: {error}"))?
}

fn ensure_settings_window_open(app: &tauri::AppHandle, query: &str) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App(format!("index.html?{}", query).into()),
    )
    .title("Murmur Settings")
    .inner_size(720.0, 560.0)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn open_settings_window(app: tauri::AppHandle) -> Result<(), String> {
    ensure_settings_window_open(&app, "view=settings")
}

// ============================================================================
// History Commands
// ============================================================================

#[tauri::command]
async fn get_history(
    offset: usize,
    limit: usize,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<lt_core::HistoryEntry>, String> {
    Ok(state
        .store
        .history
        .read()
        .await?
        .entries
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect())
}

#[tauri::command]
async fn search_history(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<lt_core::HistoryEntry>, String> {
    Ok(state.store.history.read().await?.search_entries(&query))
}

#[tauri::command]
async fn delete_history_entry(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .store
        .history
        .update(move |history| {
            if !history.delete_entry(&id) {
                return Err(format!("History entry '{id}' not found"));
            }
            Ok(())
        })
        .await
        .map(|_| ())
}

#[tauri::command]
async fn clear_history(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.store.history.clear().await
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
async fn request_microphone_permission() -> Result<(), String> {
    permissions::request_microphone_permission().await
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
            "Stop Recording"
        } else {
            "Start Recording"
        },
    )
    .build(app)?;

    let settings_item = MenuItemBuilder::with_id("open_settings", "Open Settings").build(app)?;
    let history_item = MenuItemBuilder::with_id("open_history", "History").build(app)?;
    let update_item = MenuItemBuilder::with_id("check_updates", "Check for Updates").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&toggle_item)
        .item(&settings_item)
        .item(&history_item)
        .separator()
        .item(&update_item)
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
    let icon_png_bytes = include_bytes!("../icons/tray-icon.png");
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
    let diagnostic_log_store = diagnostics::shared_diagnostic_log_store();
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(
                "lt_tauri=debug,lt_audio=debug,lt_stt=debug,lt_llm=debug,lt_pipeline=debug,lt_output=debug,info",
            )
        }))
        .with(tracing_subscriber::fmt::layer())
        .with(diagnostics::DiagnosticLogLayer::new(diagnostic_log_store))
        .init();

    // Load config to determine LLM processor. A corrupt file is reported at
    // WARN so the diagnostics view shows why the defaults are in effect.
    let config_path = AppConfig::default_config_file().ok();
    let (config, config_warning) = storage::load_startup_config(config_path.as_deref());
    if let Some(warning) = config_warning {
        tracing::warn!("{warning}");
    }

    let is_first_launch = storage::claim_first_launch(config_path.as_deref(), &config);

    let startup_hotkey = config.hotkey.clone();

    // Load user prompt overrides from disk (falls back to embedded defaults).
    let prompts = {
        let config_dir = AppConfig::default_config_dir().ok();
        let set = config_dir
            .as_ref()
            .map(|dir| match PromptStore::load_all(dir) {
                Ok(set) => set,
                Err(e) => {
                    tracing::warn!("Failed to load prompt overrides: {}, using defaults", e);
                    PromptSet::default()
                }
            })
            .unwrap_or_default();
        PromptManager::from_set(set)
    };

    // Initialize LLM processor based on config. An unconfigured selection is
    // only reported here; `start_pipeline` rebuilds it and surfaces the error
    // to the user before each recording.
    let llm_processor = create_llm_processor(
        &config.llm_processor,
        config.llm_model.clone(),
        &config,
        &prompts,
    )
    .unwrap_or_else(|error| {
        tracing::warn!("{error}; falling back to the default LLM processor");
        Arc::new(GeminiProcessor::with_model_and_prompts(
            config.llm_model.clone(),
            prompts.clone(),
        ))
    });

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
    let output_sink = match CombinedOutput::new(config.output_mode) {
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

    let event_rx = pipeline.subscribe_events();
    let config_dir = AppConfig::default_config_dir().expect("application config directory");

    // Create app state
    let app_state = AppState {
        pipeline: Arc::new(Mutex::new(pipeline)),
        store: storage::AppStore::new(config_dir),
        hotkey_updates: Arc::new(Mutex::new(())),
        prompts,
    };
    app_state.store.history.set_enabled(config.save_history);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            start_pipeline,
            stop_pipeline,
            toggle_recording,
            is_recording,
            get_pipeline_state,
            get_config,
            save_config,
            get_diagnostic_logs,
            clear_diagnostic_logs,
            set_stt_provider,
            save_api_key,
            get_stt_providers,
            get_llm_processors,
            set_llm_processor,
            set_llm_model,
            set_custom_llm_endpoint,
            set_custom_stt_endpoint,
            set_output_mode,
            set_save_history,
            set_chinese_conversion,
            set_hotkey,
            get_dictionary,
            add_dictionary_entry,
            update_dictionary_entry,
            delete_dictionary_entry,
            search_dictionary,
            get_prompts,
            set_prompt,
            reset_prompt,
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
            set_apple_stt_locale,
            get_elevenlabs_languages,
            set_elevenlabs_language
        ])
        .setup(move |app| {
            app.manage(events::spawn(
                app.handle().clone(),
                event_rx,
                app.state::<AppState>().store.history.clone(),
            ));
            // Set up system tray - embed icon at compile time to avoid runtime path issues
            let icon_png_bytes = include_bytes!("../icons/tray-icon.png");
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
                MenuItemBuilder::with_id("toggle_recording", "Start Recording").build(app)?;
            let settings_item =
                MenuItemBuilder::with_id("open_settings", "Open Settings").build(app)?;
            let history_item = MenuItemBuilder::with_id("open_history", "History").build(app)?;
            let update_item =
                MenuItemBuilder::with_id("check_updates", "Check for Updates").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&toggle_item)
                .item(&settings_item)
                .item(&history_item)
                .separator()
                .item(&update_item)
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
                                if let Err(message) =
                                    toggle_recording(app_handle.clone(), state).await
                                {
                                    tracing::warn!("Tray action failed: {message}");
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
                        "check_updates" => {
                            let handle = app_handle.clone();
                            tauri::async_runtime::spawn(async move {
                                let was_open = handle.get_webview_window("settings").is_some();
                                if let Err(e) = ensure_settings_window_open(
                                    &handle,
                                    "view=settings&action=check-update",
                                ) {
                                    tracing::warn!("Failed to open settings window: {}", e);
                                    return;
                                }
                                if was_open {
                                    // URL query is ignored when the window already exists;
                                    // signal the frontend to switch to About and check.
                                    let _ = handle.emit("open-about-and-check", ());
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

            // Background update check (delayed to avoid slowing startup)
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                let updater = match update_handle.updater() {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::debug!("Updater not configured: {e}");
                        return;
                    }
                };
                match updater.check().await {
                    Ok(Some(update)) => {
                        tracing::info!(
                            "Update available: v{} → v{}",
                            update.current_version,
                            update.version
                        );
                        let _ = update_handle.emit(
                            "update-available",
                            serde_json::json!({
                                "version": update.version,
                                "body": update.body
                            }),
                        );
                    }
                    Ok(None) => {
                        tracing::info!("App is up to date");
                    }
                    Err(e) => {
                        tracing::debug!("Update check failed (expected if no releases yet): {e}");
                    }
                }
            });

            if let Err(error) = startup_hotkey
                .parse::<Shortcut>()
                .map_err(|error| error.to_string())
                .and_then(|shortcut| register_recording_shortcut(app.handle(), shortcut))
            {
                tracing::warn!("Failed to set up shortcut handler: {error}");
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(processor: LlmProcessorType) -> AppConfig {
        AppConfig {
            llm_processor: processor,
            ..AppConfig::default()
        }
    }

    #[test]
    fn a_provider_counts_as_configured_once_its_key_is_present() {
        let mut config = AppConfig::default();
        for provider in SttProviderType::ALL {
            let Some(key) = provider.api_key_name() else {
                continue;
            };
            assert!(!stt_provider_configured(provider, &config), "{provider:?}");
            config.api_keys.insert(key.into(), "k".into());
            assert!(stt_provider_configured(provider, &config), "{provider:?}");
        }
    }

    #[test]
    fn the_custom_endpoints_hinge_on_the_base_url_not_the_key() {
        let mut config = AppConfig::default();
        config.api_keys.insert("custom_stt".into(), "k".into());
        config.api_keys.insert("custom_llm".into(), "k".into());
        assert!(!stt_provider_configured(
            SttProviderType::CustomStt,
            &config
        ));
        assert!(!llm_processor_configured(
            LlmProcessorType::CustomApi,
            &config
        ));

        config.http_stt_config.custom_base_url = Some("http://127.0.0.1/v1".into());
        config.http_llm_config.custom_base_url = Some("http://127.0.0.1/v1".into());
        assert!(stt_provider_configured(SttProviderType::CustomStt, &config));
        assert!(llm_processor_configured(
            LlmProcessorType::CustomApi,
            &config
        ));
    }

    #[test]
    fn processors_that_need_no_key_are_configured_out_of_the_box() {
        let config = AppConfig::default();
        for processor in [
            LlmProcessorType::Gemini,
            LlmProcessorType::Copilot,
            LlmProcessorType::AppleLlm,
        ] {
            assert!(
                llm_processor_configured(processor, &config),
                "{processor:?}"
            );
        }
    }

    #[test]
    fn every_processor_declares_a_default_model() {
        for processor in LlmProcessorType::ALL {
            if processor.is_macos_only() && !cfg!(target_os = "macos") {
                continue;
            }
            assert!(
                !lt_llm::default_model(processor).is_empty(),
                "{processor:?} has no default model"
            );
        }
    }

    #[test]
    fn http_processors_report_a_missing_api_key_instead_of_sending_an_empty_one() {
        for (processor, label) in [
            (LlmProcessorType::OpenAiApi, "OpenAI"),
            (LlmProcessorType::ClaudeApi, "Claude"),
            (LlmProcessorType::GeminiApi, "Gemini"),
        ] {
            let config = config_with(processor);
            let error = create_llm_processor(&processor, None, &config, &PromptManager::new())
                .err()
                .expect("an unconfigured provider must not build");
            assert!(error.starts_with(label), "{error}");
            assert!(error.contains("API key not configured"), "{error}");
        }
    }

    #[test]
    fn a_blank_api_key_counts_as_missing() {
        let mut config = config_with(LlmProcessorType::OpenAiApi);
        config.api_keys.insert("openai".into(), "   ".into());
        assert!(create_llm_processor(
            &LlmProcessorType::OpenAiApi,
            None,
            &config,
            &PromptManager::new()
        )
        .is_err());
    }

    #[test]
    fn a_custom_endpoint_without_a_base_url_is_rejected_rather_than_defaulted() {
        // A fabricated fallback address would post the transcript to whatever
        // happens to be listening there.
        let config = config_with(LlmProcessorType::CustomApi);
        let error = create_llm_processor(
            &LlmProcessorType::CustomApi,
            None,
            &config,
            &PromptManager::new(),
        )
        .err()
        .expect("a custom endpoint without a base URL must not build");
        assert!(error.contains("base URL"), "{error}");
    }

    #[test]
    fn a_custom_endpoint_needs_no_api_key() {
        let mut config = config_with(LlmProcessorType::CustomApi);
        config.http_llm_config.custom_base_url = Some("http://127.0.0.1:1234/v1".into());
        assert!(create_llm_processor(
            &LlmProcessorType::CustomApi,
            None,
            &config,
            &PromptManager::new()
        )
        .is_ok());
    }

    #[test]
    fn cli_processors_need_no_configuration() {
        for processor in [LlmProcessorType::Gemini, LlmProcessorType::Copilot] {
            let config = config_with(processor);
            assert!(
                create_llm_processor(&processor, None, &config, &PromptManager::new()).is_ok(),
                "{processor:?} must build without configuration"
            );
        }
    }
}
