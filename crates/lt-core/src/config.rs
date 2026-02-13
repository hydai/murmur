use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{MurmurError, Result};
use crate::output::OutputMode;

/// STT provider type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SttProviderType {
    ElevenLabs,
    OpenAI,
    Groq,
}

impl Default for SttProviderType {
    fn default() -> Self {
        Self::ElevenLabs
    }
}

/// LLM processor type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProcessorType {
    Gemini,
    Copilot,
}

impl Default for LlmProcessorType {
    fn default() -> Self {
        Self::Gemini
    }
}

/// UI preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPreferences {
    /// Window opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Show waveform indicator
    pub show_waveform: bool,
    /// Theme (light/dark)
    pub theme: String,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            opacity: 0.9,
            show_waveform: true,
            theme: "dark".to_string(),
        }
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Selected STT provider
    pub stt_provider: SttProviderType,

    /// API keys (provider_name -> api_key)
    pub api_keys: HashMap<String, String>,

    /// Global hotkey (e.g., "Cmd+Shift+L")
    pub hotkey: String,

    /// Selected LLM processor
    pub llm_processor: LlmProcessorType,

    /// Output mode
    pub output_mode: OutputMode,

    /// UI preferences
    pub ui_preferences: UiPreferences,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            stt_provider: SttProviderType::default(),
            api_keys: HashMap::new(),
            hotkey: "Ctrl+`".to_string(),
            llm_processor: LlmProcessorType::default(),
            output_mode: OutputMode::default(),
            ui_preferences: UiPreferences::default(),
        }
    }
}

impl AppConfig {
    /// Get the default config directory path
    pub fn default_config_dir() -> Result<PathBuf> {
        directories::ProjectDirs::from("com", "hydai", "Murmur")
            .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
            .ok_or_else(|| MurmurError::Config("Failed to get config directory".to_string()))
    }

    /// Get the default config file path
    pub fn default_config_file() -> Result<PathBuf> {
        Ok(Self::default_config_dir()?.join("config.toml"))
    }

    /// Load config from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| MurmurError::Config(format!("Failed to serialize config: {}", e)))?;

        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, content)?;
        Ok(())
    }
}
