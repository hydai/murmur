use lt_core::llm::ProcessingTask;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Compile-time embedded prompt templates.
/// Using `include_str!()` eliminates the working-directory dependency that
/// caused "No such file or directory" errors when running the built app.
/// These remain the fallback when no user override exists on disk.
const POST_PROCESS_TEMPLATE: &str = include_str!("../../../prompts/post_process.md");
const SHORTEN_TEMPLATE: &str = include_str!("../../../prompts/shorten.md");
const CHANGE_TONE_TEMPLATE: &str = include_str!("../../../prompts/change_tone.md");
const GENERATE_REPLY_TEMPLATE: &str = include_str!("../../../prompts/generate_reply.md");
const TRANSLATE_TEMPLATE: &str = include_str!("../../../prompts/translate.md");

/// Stable identifier for a prompt template. Serialized as snake_case, used as
/// the override filename stem and the IPC parameter name.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptName {
    PostProcess,
    Shorten,
    ChangeTone,
    GenerateReply,
    Translate,
}

impl PromptName {
    pub const ALL: [PromptName; 5] = [
        Self::PostProcess,
        Self::Shorten,
        Self::ChangeTone,
        Self::GenerateReply,
        Self::Translate,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PostProcess => "post_process",
            Self::Shorten => "shorten",
            Self::ChangeTone => "change_tone",
            Self::GenerateReply => "generate_reply",
            Self::Translate => "translate",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.as_str() == s)
    }

    pub fn default_template(self) -> &'static str {
        match self {
            Self::PostProcess => POST_PROCESS_TEMPLATE,
            Self::Shorten => SHORTEN_TEMPLATE,
            Self::ChangeTone => CHANGE_TONE_TEMPLATE,
            Self::GenerateReply => GENERATE_REPLY_TEMPLATE,
            Self::Translate => TRANSLATE_TEMPLATE,
        }
    }

    pub fn required_placeholders(self) -> &'static [&'static str] {
        match self {
            Self::PostProcess => &["{dictionary_terms}", "{raw_text}"],
            Self::Shorten => &["{text}"],
            Self::ChangeTone => &["{tone}", "{text}"],
            Self::GenerateReply => &["{context}"],
            Self::Translate => &["{language}", "{text}"],
        }
    }

    pub fn display_title(self) -> &'static str {
        match self {
            Self::PostProcess => "Post-Process Transcription",
            Self::Shorten => "Shorten",
            Self::ChangeTone => "Change Tone",
            Self::GenerateReply => "Generate Reply",
            Self::Translate => "Translate",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::PostProcess => {
                "Cleans raw STT output: removes fillers, fixes grammar, applies the personal dictionary. Runs when no voice command prefix is detected."
            }
            Self::Shorten => "Condenses the transcription while preserving meaning.",
            Self::ChangeTone => "Rewrites the transcription in a target tone (formal / casual).",
            Self::GenerateReply => "Generates a response given the transcription as context.",
            Self::Translate => "Translates the transcription into a target language.",
        }
    }

    pub fn task_variant_label(self) -> &'static str {
        match self {
            Self::PostProcess => "PostProcess",
            Self::Shorten => "Shorten",
            Self::ChangeTone => "ChangeTone",
            Self::GenerateReply => "GenerateReply",
            Self::Translate => "Translate",
        }
    }
}

/// In-memory collection of optional per-prompt overrides. Missing entries fall
/// back to the compile-time default via `PromptName::default_template()`.
#[derive(Debug, Clone, Default)]
pub struct PromptSet {
    post_process: Option<String>,
    shorten: Option<String>,
    change_tone: Option<String>,
    generate_reply: Option<String>,
    translate: Option<String>,
}

impl PromptSet {
    fn slot(&self, name: PromptName) -> &Option<String> {
        match name {
            PromptName::PostProcess => &self.post_process,
            PromptName::Shorten => &self.shorten,
            PromptName::ChangeTone => &self.change_tone,
            PromptName::GenerateReply => &self.generate_reply,
            PromptName::Translate => &self.translate,
        }
    }

    fn slot_mut(&mut self, name: PromptName) -> &mut Option<String> {
        match name {
            PromptName::PostProcess => &mut self.post_process,
            PromptName::Shorten => &mut self.shorten,
            PromptName::ChangeTone => &mut self.change_tone,
            PromptName::GenerateReply => &mut self.generate_reply,
            PromptName::Translate => &mut self.translate,
        }
    }

    pub fn get(&self, name: PromptName) -> &str {
        self.slot(name)
            .as_deref()
            .unwrap_or_else(|| name.default_template())
    }

    pub fn set_override(&mut self, name: PromptName, content: String) {
        *self.slot_mut(name) = Some(content);
    }

    pub fn clear_override(&mut self, name: PromptName) {
        *self.slot_mut(name) = None;
    }

    pub fn has_override(&self, name: PromptName) -> bool {
        self.slot(name).is_some()
    }
}

/// Cheaply-clonable handle backed by a shared `Arc<RwLock<PromptSet>>`.
/// All LLM processors clone the same handle, so one write propagates to every
/// consumer's next `build_prompt` call without recreating processors.
#[derive(Clone)]
pub struct PromptManager {
    inner: Arc<RwLock<PromptSet>>,
}

impl PromptManager {
    pub fn new() -> Self {
        Self::from_set(PromptSet::default())
    }

    pub fn from_set(set: PromptSet) -> Self {
        Self {
            inner: Arc::new(RwLock::new(set)),
        }
    }

    /// Expose the inner `Arc` so IPC handlers can mutate the shared set.
    pub fn shared(&self) -> Arc<RwLock<PromptSet>> {
        self.inner.clone()
    }

    pub async fn build_prompt(&self, task: &ProcessingTask) -> String {
        let set = self.inner.read().await;
        match task {
            ProcessingTask::PostProcess {
                text,
                dictionary_terms,
            } => {
                let dict_terms_str = if dictionary_terms.is_empty() {
                    "No custom terms defined.".to_string()
                } else {
                    dictionary_terms.join(", ")
                };
                set.get(PromptName::PostProcess)
                    .replace("{dictionary_terms}", &dict_terms_str)
                    .replace("{raw_text}", text)
            }
            ProcessingTask::Shorten { text } => {
                set.get(PromptName::Shorten).replace("{text}", text)
            }
            ProcessingTask::ChangeTone { text, target_tone } => set
                .get(PromptName::ChangeTone)
                .replace("{text}", text)
                .replace("{tone}", target_tone),
            ProcessingTask::GenerateReply { context } => set
                .get(PromptName::GenerateReply)
                .replace("{context}", context),
            ProcessingTask::Translate {
                text,
                target_language,
            } => set
                .get(PromptName::Translate)
                .replace("{text}", text)
                .replace("{language}", target_language),
        }
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prompt_manager_creation() {
        let _manager = PromptManager::new();
    }

    #[tokio::test]
    async fn test_build_post_process_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::PostProcess {
            text: "um so like hello".to_string(),
            dictionary_terms: vec!["API".to_string(), "STT".to_string()],
        };

        let prompt = manager.build_prompt(&task).await;
        assert!(prompt.contains("um so like hello"));
        assert!(prompt.contains("API, STT"));
    }

    #[tokio::test]
    async fn test_build_shorten_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Shorten {
            text: "This is a long text".to_string(),
        };

        let prompt = manager.build_prompt(&task).await;
        assert!(prompt.contains("This is a long text"));
    }

    #[tokio::test]
    async fn test_build_translate_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Translate {
            text: "Hello world".to_string(),
            target_language: "Chinese".to_string(),
        };

        let prompt = manager.build_prompt(&task).await;
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Chinese"));
    }

    #[tokio::test]
    async fn test_build_change_tone_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::ChangeTone {
            text: "hey there".to_string(),
            target_tone: "formal".to_string(),
        };

        let prompt = manager.build_prompt(&task).await;
        assert!(prompt.contains("hey there"));
        assert!(prompt.contains("formal"));
    }

    #[tokio::test]
    async fn test_build_generate_reply_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::GenerateReply {
            context: "Can you attend the meeting?".to_string(),
        };

        let prompt = manager.build_prompt(&task).await;
        assert!(prompt.contains("Can you attend the meeting?"));
    }

    #[tokio::test]
    async fn test_prompt_set_override_fallback() {
        let mut set = PromptSet::default();
        assert!(!set.has_override(PromptName::Shorten));
        assert_eq!(set.get(PromptName::Shorten), SHORTEN_TEMPLATE);

        set.set_override(PromptName::Shorten, "CUSTOM".to_string());
        assert!(set.has_override(PromptName::Shorten));
        assert_eq!(set.get(PromptName::Shorten), "CUSTOM");

        set.clear_override(PromptName::Shorten);
        assert!(!set.has_override(PromptName::Shorten));
        assert_eq!(set.get(PromptName::Shorten), SHORTEN_TEMPLATE);
    }

    #[tokio::test]
    async fn test_prompt_manager_hot_swap_via_shared() {
        let manager = PromptManager::new();
        let shared = manager.shared();

        shared
            .write()
            .await
            .set_override(PromptName::Shorten, "SHORTEN-OVERRIDE {text}".to_string());

        let task = ProcessingTask::Shorten {
            text: "hello".to_string(),
        };
        let prompt = manager.build_prompt(&task).await;
        assert_eq!(prompt, "SHORTEN-OVERRIDE hello");
    }

    #[test]
    fn test_prompt_name_round_trip() {
        for name in PromptName::ALL {
            assert_eq!(PromptName::from_key(name.as_str()), Some(name));
        }
        assert_eq!(PromptName::from_key("nonsense"), None);
    }
}
