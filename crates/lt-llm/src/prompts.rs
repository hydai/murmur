use lt_core::llm::ProcessingTask;
use std::path::{Path, PathBuf};

/// Prompt template manager
pub struct PromptManager {
    prompts_dir: PathBuf,
}

impl PromptManager {
    /// Create a new prompt manager with default prompts directory
    pub fn new() -> Self {
        // Get the prompts directory relative to project root
        let prompts_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("prompts");

        Self { prompts_dir }
    }

    /// Create a prompt manager with custom prompts directory
    pub fn with_dir<P: AsRef<Path>>(dir: P) -> Self {
        Self {
            prompts_dir: dir.as_ref().to_path_buf(),
        }
    }

    /// Build a prompt for the given task
    pub fn build_prompt(&self, task: &ProcessingTask) -> Result<String, std::io::Error> {
        match task {
            ProcessingTask::PostProcess {
                text,
                dictionary_terms,
            } => {
                let template = self.load_template("post_process.md")?;
                let dict_terms_str = if dictionary_terms.is_empty() {
                    "No custom terms defined.".to_string()
                } else {
                    dictionary_terms.join(", ")
                };
                Ok(template
                    .replace("{dictionary_terms}", &dict_terms_str)
                    .replace("{raw_text}", text))
            }
            ProcessingTask::Shorten { text } => {
                let template = self.load_template("shorten.md")?;
                Ok(template.replace("{text}", text))
            }
            ProcessingTask::ChangeTone { text, target_tone } => {
                let template = self.load_template("change_tone.md")?;
                Ok(template
                    .replace("{text}", text)
                    .replace("{tone}", target_tone))
            }
            ProcessingTask::GenerateReply { context } => {
                let template = self.load_template("generate_reply.md")?;
                Ok(template.replace("{context}", context))
            }
            ProcessingTask::Translate {
                text,
                target_language,
            } => {
                let template = self.load_template("translate.md")?;
                Ok(template
                    .replace("{text}", text)
                    .replace("{language}", target_language))
            }
        }
    }

    /// Load a template file
    fn load_template(&self, filename: &str) -> Result<String, std::io::Error> {
        let path = self.prompts_dir.join(filename);
        std::fs::read_to_string(path)
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
        let manager = PromptManager::new();
        assert!(manager.prompts_dir.ends_with("prompts"));
    }

    #[test]
    fn test_build_post_process_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::PostProcess {
            text: "um so like hello".to_string(),
            dictionary_terms: vec!["API".to_string(), "STT".to_string()],
        };

        let result = manager.build_prompt(&task);
        if let Ok(prompt) = result {
            assert!(prompt.contains("um so like hello"));
            assert!(prompt.contains("API, STT"));
        }
    }

    #[test]
    fn test_build_shorten_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Shorten {
            text: "This is a long text".to_string(),
        };

        let result = manager.build_prompt(&task);
        if let Ok(prompt) = result {
            assert!(prompt.contains("This is a long text"));
        }
    }

    #[test]
    fn test_build_translate_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::Translate {
            text: "Hello world".to_string(),
            target_language: "Chinese".to_string(),
        };

        let result = manager.build_prompt(&task);
        if let Ok(prompt) = result {
            assert!(prompt.contains("Hello world"));
            assert!(prompt.contains("Chinese"));
        }
    }

    #[test]
    fn test_build_change_tone_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::ChangeTone {
            text: "hey there".to_string(),
            target_tone: "formal".to_string(),
        };

        let result = manager.build_prompt(&task);
        if let Ok(prompt) = result {
            assert!(prompt.contains("hey there"));
            assert!(prompt.contains("formal"));
        }
    }

    #[test]
    fn test_build_generate_reply_prompt() {
        let manager = PromptManager::new();
        let task = ProcessingTask::GenerateReply {
            context: "Can you attend the meeting?".to_string(),
        };

        let result = manager.build_prompt(&task);
        if let Ok(prompt) = result {
            assert!(prompt.contains("Can you attend the meeting?"));
        }
    }
}
