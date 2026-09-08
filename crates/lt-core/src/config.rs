use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{MurmurError, Result};
use crate::output::OutputMode;

/// STT provider type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SttProviderType {
    #[default]
    ElevenLabs,
    OpenAI,
    Groq,
    #[serde(rename = "apple_stt")]
    AppleStt,
    #[serde(rename = "custom_stt")]
    CustomStt,
}

/// How the settings UI groups a provider. Serialized as the `provider_type`
/// field the frontend filters on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Streams partial transcripts over a socket.
    Streaming,
    /// Uploads finished audio to a REST endpoint.
    Batch,
    /// A command-line tool on the user's machine.
    Cli,
    /// Runs on the device with no network call.
    Local,
    /// A cloud HTTP API.
    Http,
    /// A user-supplied endpoint.
    Custom,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Streaming => "streaming",
            Self::Batch => "batch",
            Self::Cli => "cli",
            Self::Local => "local",
            Self::Http => "http",
            Self::Custom => "custom",
        }
    }
}

impl SttProviderType {
    /// Every provider, in the order the settings page lists them.
    pub const ALL: [Self; 5] = [
        Self::ElevenLabs,
        Self::OpenAI,
        Self::Groq,
        Self::AppleStt,
        Self::CustomStt,
    ];

    /// Stable identifier shared by the config file, the IPC surface and the UI.
    /// Kept in step with the serde representation by a test.
    pub fn id(self) -> &'static str {
        match self {
            Self::ElevenLabs => "elevenlabs",
            Self::OpenAI => "openai",
            Self::Groq => "groq",
            Self::AppleStt => "apple_stt",
            Self::CustomStt => "custom_stt",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::ElevenLabs => "ElevenLabs Scribe",
            Self::OpenAI => "OpenAI Whisper",
            Self::Groq => "Groq Whisper Turbo",
            Self::AppleStt => "Apple Speech",
            Self::CustomStt => "Custom Endpoint",
        }
    }

    /// Key into [`AppConfig::api_keys`], or `None` when no key is needed.
    pub fn api_key_name(self) -> Option<&'static str> {
        match self {
            Self::ElevenLabs => Some("elevenlabs"),
            Self::OpenAI => Some("openai"),
            Self::Groq => Some("groq"),
            // On-device.
            Self::AppleStt => None,
            // Optional: a local Whisper server usually needs no auth.
            Self::CustomStt => None,
        }
    }

    pub fn kind(self) -> ProviderKind {
        match self {
            Self::ElevenLabs => ProviderKind::Streaming,
            Self::OpenAI | Self::Groq => ProviderKind::Batch,
            Self::AppleStt => ProviderKind::Local,
            Self::CustomStt => ProviderKind::Batch,
        }
    }

    /// Only available on macOS, so the settings list omits it elsewhere.
    pub fn is_macos_only(self) -> bool {
        matches!(self, Self::AppleStt)
    }
}

impl std::str::FromStr for SttProviderType {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let value = value.to_lowercase();
        Self::ALL
            .into_iter()
            .find(|provider| provider.id() == value)
            .ok_or_else(|| format!("Unknown STT provider: {value}"))
    }
}

/// LLM processor type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LlmProcessorType {
    #[default]
    Gemini,
    Copilot,
    #[serde(rename = "apple_llm")]
    AppleLlm,
    #[serde(rename = "openai_api")]
    OpenAiApi,
    #[serde(rename = "claude_api")]
    ClaudeApi,
    #[serde(rename = "gemini_api")]
    GeminiApi,
    #[serde(rename = "custom_api")]
    CustomApi,
}

impl LlmProcessorType {
    /// Every processor, in the order the settings page lists them.
    pub const ALL: [Self; 7] = [
        Self::Gemini,
        Self::Copilot,
        Self::AppleLlm,
        Self::OpenAiApi,
        Self::ClaudeApi,
        Self::GeminiApi,
        Self::CustomApi,
    ];

    /// Stable identifier shared by the config file, the IPC surface and the UI.
    /// Kept in step with the serde representation by a test.
    pub fn id(self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::Copilot => "copilot",
            Self::AppleLlm => "apple_llm",
            Self::OpenAiApi => "openai_api",
            Self::ClaudeApi => "claude_api",
            Self::GeminiApi => "gemini_api",
            Self::CustomApi => "custom_api",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Gemini => "Gemini CLI",
            Self::Copilot => "Copilot CLI",
            Self::AppleLlm => "Apple Intelligence",
            Self::OpenAiApi => "OpenAI API",
            Self::ClaudeApi => "Claude API",
            Self::GeminiApi => "Gemini API",
            Self::CustomApi => "Custom Endpoint",
        }
    }

    /// Key into [`AppConfig::api_keys`], or `None` when no key is needed.
    pub fn api_key_name(self) -> Option<&'static str> {
        match self {
            // The CLIs and the on-device model carry their own auth.
            Self::Gemini | Self::Copilot | Self::AppleLlm => None,
            // Shared with the OpenAI STT provider.
            Self::OpenAiApi => Some("openai"),
            Self::ClaudeApi => Some("anthropic"),
            Self::GeminiApi => Some("google_ai"),
            // Optional: a local server usually needs no auth.
            Self::CustomApi => Some("custom_llm"),
        }
    }

    /// Whether the processor cannot run at all without its key.
    pub fn requires_api_key(self) -> bool {
        matches!(self, Self::OpenAiApi | Self::ClaudeApi | Self::GeminiApi)
    }

    pub fn kind(self) -> ProviderKind {
        match self {
            Self::Gemini | Self::Copilot => ProviderKind::Cli,
            Self::AppleLlm => ProviderKind::Local,
            Self::OpenAiApi | Self::ClaudeApi | Self::GeminiApi => ProviderKind::Http,
            Self::CustomApi => ProviderKind::Custom,
        }
    }

    /// Only available on macOS, so the settings list omits it elsewhere.
    pub fn is_macos_only(self) -> bool {
        matches!(self, Self::AppleLlm)
    }
}

impl std::str::FromStr for LlmProcessorType {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let value = value.to_lowercase();
        Self::ALL
            .into_iter()
            .find(|processor| processor.id() == value)
            .ok_or_else(|| format!("Unknown LLM processor: {value}"))
    }
}

/// HTTP LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpLlmConfig {
    /// Custom base URL (only for CustomApi)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_base_url: Option<String>,
    /// Display name for custom endpoint in UI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_display_name: Option<String>,
}

/// HTTP STT provider configuration (for custom_stt)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpSttConfig {
    /// Custom base URL for OpenAI-compatible STT endpoint
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_base_url: Option<String>,
    /// Display name for custom endpoint in UI
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_display_name: Option<String>,
    /// Model name (defaults to "whisper-1")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_model: Option<String>,
    /// Language hint (ISO-639-1 code, e.g. "en", "zh", "ja")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// UI preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPreferences {
    /// Window opacity (0.0 - 1.0)
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Show waveform indicator
    #[serde(default = "default_show_waveform")]
    pub show_waveform: bool,
    /// Theme (light/dark)
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            opacity: default_opacity(),
            show_waveform: default_show_waveform(),
            theme: default_theme(),
        }
    }
}

/// How Chinese text in the final output is converted before delivery.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ChineseConversion {
    /// Convert Simplified Chinese to Traditional Chinese with Taiwan phrasing.
    #[default]
    Traditional,
    /// Deliver the text as transcribed.
    None,
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Selected STT provider
    #[serde(default)]
    pub stt_provider: SttProviderType,

    /// API keys (provider_name -> api_key)
    #[serde(default)]
    pub api_keys: HashMap<String, String>,

    /// Global hotkey (e.g., "Cmd+Shift+L")
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    /// Selected LLM processor
    #[serde(default)]
    pub llm_processor: LlmProcessorType,

    /// Optional LLM model name override (None = use provider default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_model: Option<String>,

    /// Output mode
    #[serde(default)]
    pub output_mode: OutputMode,

    /// UI preferences
    #[serde(default)]
    pub ui_preferences: UiPreferences,

    /// Apple STT locale ("auto" = detect system locale, or e.g. "en_US", "ja_JP")
    #[serde(default = "default_apple_stt_locale")]
    pub apple_stt_locale: String,

    /// ElevenLabs STT language ("auto" = automatic detection, or ISO 639-3 code)
    #[serde(default = "default_elevenlabs_language")]
    pub elevenlabs_language: String,

    /// HTTP LLM provider configuration
    #[serde(default)]
    pub http_llm_config: HttpLlmConfig,

    /// HTTP STT provider configuration (for custom_stt)
    #[serde(default)]
    pub http_stt_config: HttpSttConfig,

    /// Whether finished transcriptions are written to history.json
    #[serde(default = "default_save_history")]
    pub save_history: bool,

    /// Chinese conversion applied to the final output
    #[serde(default)]
    pub chinese_conversion: ChineseConversion,
}

fn default_save_history() -> bool {
    true
}

fn default_hotkey() -> String {
    "Ctrl+`".to_string()
}

fn default_opacity() -> f32 {
    0.9
}

fn default_show_waveform() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_apple_stt_locale() -> String {
    "auto".to_string()
}

fn default_elevenlabs_language() -> String {
    "auto".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            stt_provider: SttProviderType::default(),
            api_keys: HashMap::new(),
            hotkey: default_hotkey(),
            llm_processor: LlmProcessorType::default(),
            llm_model: None,
            output_mode: OutputMode::default(),
            ui_preferences: UiPreferences::default(),
            apple_stt_locale: default_apple_stt_locale(),
            elevenlabs_language: default_elevenlabs_language(),
            http_llm_config: HttpLlmConfig::default(),
            http_stt_config: HttpSttConfig::default(),
            save_history: default_save_history(),
            chinese_conversion: ChineseConversion::default(),
        }
    }
}

/// toml's Display echoes the offending line, which in a config file can be an
/// API key; keep only the location and the message.
fn describe_toml_error(content: &str, error: &toml::de::Error) -> String {
    match error.span() {
        Some(span) => {
            // The span is a byte offset; counting newline bytes avoids slicing
            // the str on what might not be a char boundary.
            let offset = span.start.min(content.len());
            let line = content.as_bytes()[..offset]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count()
                + 1;
            format!("TOML parse error at line {line}: {}", error.message())
        }
        None => format!("TOML parse error: {}", error.message()),
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
        toml::from_str(&content)
            .map_err(|error| MurmurError::Config(describe_toml_error(&content, &error)))
    }

    /// Save config to TOML file
    /// Copy for the webview: every setting except secrets.
    pub fn redacted(&self) -> Self {
        let mut copy = self.clone();
        copy.api_keys.clear();
        copy
    }

    /// Replace the settings with a redacted copy while keeping the stored
    /// secrets, so a round-tripped config can never erase or inject API keys.
    pub fn apply_redacted(&mut self, incoming: AppConfig) {
        let secrets = std::mem::take(&mut self.api_keys);
        *self = incoming;
        self.api_keys = secrets;
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| MurmurError::Config(format!("Failed to serialize config: {}", e)))?;

        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        crate::persistence::atomic_write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_secret() -> AppConfig {
        AppConfig {
            hotkey: "Ctrl+Shift+Space".into(),
            api_keys: HashMap::from([("openai".to_string(), "sk-secret".to_string())]),
            ..AppConfig::default()
        }
    }

    #[test]
    fn redacted_config_drops_api_keys_only() {
        let config = config_with_secret();
        let redacted = config.redacted();
        assert!(redacted.api_keys.is_empty());
        assert_eq!(redacted.hotkey, config.hotkey);
        assert_eq!(redacted.stt_provider, config.stt_provider);
        assert_eq!(redacted.output_mode, config.output_mode);
    }

    #[test]
    fn applying_a_redacted_config_keeps_stored_api_keys() {
        let mut stored = config_with_secret();
        let mut incoming = stored.redacted();
        incoming.hotkey = "Ctrl+Alt+M".into();
        incoming
            .api_keys
            .insert("groq".into(), "must-not-be-written".into());
        stored.apply_redacted(incoming);
        assert_eq!(stored.hotkey, "Ctrl+Alt+M");
        assert_eq!(
            stored.api_keys.get("openai").map(String::as_str),
            Some("sk-secret")
        );
        assert!(!stored.api_keys.contains_key("groq"));
    }

    #[test]
    fn save_history_defaults_to_true_for_existing_config_files() {
        let written = toml::to_string(&AppConfig::default()).unwrap();
        let legacy: String = written
            .lines()
            .filter(|line| !line.starts_with("save_history"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!legacy.contains("save_history"));
        let parsed: AppConfig = toml::from_str(&legacy).unwrap();
        assert!(parsed.save_history);
    }

    #[test]
    fn chinese_conversion_defaults_to_traditional_for_existing_config_files() {
        assert_eq!(
            AppConfig::default().chinese_conversion,
            ChineseConversion::Traditional
        );
        let written = toml::to_string(&AppConfig::default()).unwrap();
        let legacy: String = written
            .lines()
            .filter(|line| !line.starts_with("chinese_conversion"))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed: AppConfig = toml::from_str(&legacy).unwrap();
        assert_eq!(parsed.chinese_conversion, ChineseConversion::Traditional);
        // Top-level keys must precede the tables in the written document.
        let none: AppConfig =
            toml::from_str(&format!("chinese_conversion = \"none\"\n{legacy}")).unwrap();
        assert_eq!(none.chinese_conversion, ChineseConversion::None);
    }

    #[test]
    fn provider_ids_match_their_serde_representation() {
        // id() is what the IPC surface, the UI and the config file all use, so
        // it has to stay in step with rename_all/rename rather than being a
        // second, hand-maintained spelling of the same thing.
        for provider in SttProviderType::ALL {
            let encoded = serde_json::to_string(&provider).unwrap();
            assert_eq!(encoded, format!("\"{}\"", provider.id()), "{provider:?}");
        }
        for processor in LlmProcessorType::ALL {
            let encoded = serde_json::to_string(&processor).unwrap();
            assert_eq!(encoded, format!("\"{}\"", processor.id()), "{processor:?}");
        }
    }

    #[test]
    fn every_variant_appears_exactly_once_in_all() {
        // A variant missing from ALL would silently disappear from settings.
        let stt: std::collections::HashSet<_> =
            SttProviderType::ALL.iter().map(|p| p.id()).collect();
        assert_eq!(stt.len(), SttProviderType::ALL.len());
        let llm: std::collections::HashSet<_> =
            LlmProcessorType::ALL.iter().map(|p| p.id()).collect();
        assert_eq!(llm.len(), LlmProcessorType::ALL.len());
    }

    #[test]
    fn provider_ids_round_trip_through_from_str() {
        use std::str::FromStr;
        for provider in SttProviderType::ALL {
            assert_eq!(SttProviderType::from_str(provider.id()), Ok(provider));
            // The IPC layer used to lowercase before matching.
            assert_eq!(
                SttProviderType::from_str(&provider.id().to_uppercase()),
                Ok(provider)
            );
        }
        for processor in LlmProcessorType::ALL {
            assert_eq!(LlmProcessorType::from_str(processor.id()), Ok(processor));
            assert_eq!(
                LlmProcessorType::from_str(&processor.id().to_uppercase()),
                Ok(processor)
            );
        }
        assert!(SttProviderType::from_str("nope").is_err());
        assert!(LlmProcessorType::from_str("nope").is_err());
    }

    #[test]
    fn api_key_names_are_declared_for_everything_that_requires_one() {
        for processor in LlmProcessorType::ALL {
            if processor.requires_api_key() {
                assert!(
                    processor.api_key_name().is_some(),
                    "{processor:?} requires a key but names none"
                );
            }
        }
    }

    #[test]
    fn an_empty_config_file_loads_as_the_defaults() {
        // Every field must be optional: a config.toml that fails to parse makes
        // `FileStore::load` error, which breaks every settings IPC with no
        // in-app way to rewrite the file.
        let parsed: AppConfig = toml::from_str("").unwrap();
        let defaults = AppConfig::default();
        assert_eq!(parsed.stt_provider, defaults.stt_provider);
        assert_eq!(parsed.llm_processor, defaults.llm_processor);
        assert_eq!(parsed.output_mode, defaults.output_mode);
        assert_eq!(parsed.hotkey, defaults.hotkey);
        assert!(parsed.api_keys.is_empty());
        assert_eq!(parsed.ui_preferences.theme, defaults.ui_preferences.theme);
    }

    #[test]
    fn dropping_any_single_key_still_loads() {
        // Mirrors a user hand-editing config.toml from the documented template.
        let written = toml::to_string(&AppConfig::default()).unwrap();
        for (index, line) in written.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('[') {
                continue;
            }
            let without: String = written
                .lines()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, l)| l)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                toml::from_str::<AppConfig>(&without).is_ok(),
                "dropping `{line}` must not make the config unloadable"
            );
        }
    }

    #[test]
    fn dropping_the_ui_preferences_table_still_loads() {
        // The table is documented in config/default.toml but nothing reads it.
        let written = toml::to_string(&AppConfig::default()).unwrap();
        assert!(written.contains("[ui_preferences]"));
        let without: String = written
            .lines()
            .take_while(|line| !line.starts_with("[ui_preferences]"))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed: AppConfig = toml::from_str(&without).unwrap();
        assert_eq!(parsed.ui_preferences.opacity, 0.9);
    }

    #[test]
    fn load_errors_count_lines_correctly_after_multibyte_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "hotkey = \"Ctrl+A\" # 快捷鍵\n# 設定檔\n[api_keys]\nopenai = \"sk-top-secret-token\n",
        )
        .unwrap();
        let error = AppConfig::load_from_file(&path).unwrap_err().to_string();
        assert!(
            !error.contains("sk-top-secret-token"),
            "secret echoed: {error}"
        );
        assert!(error.contains("line 4"), "{error}");
    }

    #[test]
    fn load_errors_locate_the_problem_without_echoing_the_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "hotkey = \"Ctrl+A\"\n[api_keys]\nopenai = \"sk-top-secret-token\n",
        )
        .unwrap();
        let error = AppConfig::load_from_file(&path).unwrap_err().to_string();
        assert!(
            !error.contains("sk-top-secret-token"),
            "secret echoed: {error}"
        );
        assert!(error.contains("line 3"), "{error}");
    }
}
