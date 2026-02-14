use lt_core::llm::ProcessingTask;

/// Compile-time embedded prompt templates.
/// Using `include_str!()` eliminates the working-directory dependency that
/// caused "No such file or directory" errors when running the built app.
const POST_PROCESS_TEMPLATE: &str = include_str!("../../../prompts/post_process.md");
const SHORTEN_TEMPLATE: &str = include_str!("../../../prompts/shorten.md");
const CHANGE_TONE_TEMPLATE: &str = include_str!("../../../prompts/change_tone.md");
const GENERATE_REPLY_TEMPLATE: &str = include_str!("../../../prompts/generate_reply.md");
const TRANSLATE_TEMPLATE: &str = include_str!("../../../prompts/translate.md");

/// Prompt template manager with compile-time embedded templates
pub struct PromptManager;

impl PromptManager {
    pub fn new() -> Self {
        Self
    }

    /// Build a prompt for the given task
    pub fn build_prompt(&self, task: &ProcessingTask) -> String {
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
                POST_PROCESS_TEMPLATE
                    .replace("{dictionary_terms}", &dict_terms_str)
                    .replace("{raw_text}", text)
            }
            ProcessingTask::Shorten { text } => SHORTEN_TEMPLATE.replace("{text}", text),
            ProcessingTask::ChangeTone { text, target_tone } => CHANGE_TONE_TEMPLATE
                .replace("{text}", text)
                .replace("{tone}", target_tone),
            ProcessingTask::GenerateReply { context } => {
                GENERATE_REPLY_TEMPLATE.replace("{context}", context)
            }
            ProcessingTask::Translate {
                text,
                target_language,
            } => TRANSLATE_TEMPLATE
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

    #[test]
    fn test_prompt_manager_creation() {
        let _manager = PromptManager::new();
    }

    #[test]
    fn test_build_post_process_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::PostProcess {
            text: "um so like hello".to_string(),
            dictionary_terms: vec!["API".to_string(), "STT".to_string()],
        };

        let prompt = manager.build_prompt(&task);
        assert!(prompt.contains("um so like hello"));
        assert!(prompt.contains("API, STT"));
    }

    #[test]
    fn test_build_shorten_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Shorten {
            text: "This is a long text".to_string(),
        };

        let prompt = manager.build_prompt(&task);
        assert!(prompt.contains("This is a long text"));
    }

    #[test]
    fn test_build_translate_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Translate {
            text: "Hello world".to_string(),
            target_language: "Chinese".to_string(),
        };

        let prompt = manager.build_prompt(&task);
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Chinese"));
    }

    #[test]
    fn test_build_change_tone_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::ChangeTone {
            text: "hey there".to_string(),
            target_tone: "formal".to_string(),
        };

        let prompt = manager.build_prompt(&task);
        assert!(prompt.contains("hey there"));
        assert!(prompt.contains("formal"));
    }

    #[test]
    fn test_build_generate_reply_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::GenerateReply {
            context: "Can you attend the meeting?".to_string(),
        };

        let prompt = manager.build_prompt(&task);
        assert!(prompt.contains("Can you attend the meeting?"));
    }
}
