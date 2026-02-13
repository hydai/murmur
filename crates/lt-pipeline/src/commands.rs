use lt_core::llm::ProcessingTask;

/// Voice command detection result
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDetection {
    /// The detected processing task
    pub task: ProcessingTask,
    /// The actual content after the command prefix
    pub content: String,
    /// The command name detected (e.g., "shorten", "formal", "reply")
    pub command_name: Option<String>,
}

/// Detect voice commands in transcribed text
///
/// Supported commands:
/// - "shorten this:" / "shorten:" → ProcessingTask::Shorten
/// - "make it formal:" / "formalize:" → ProcessingTask::ChangeTone (formal)
/// - "make it casual:" / "casualize:" → ProcessingTask::ChangeTone (casual)
/// - "reply to:" / "generate reply:" → ProcessingTask::GenerateReply
/// - "translate to [language]:" → ProcessingTask::Translate (with target language)
/// - No command prefix → ProcessingTask::PostProcess (default cleanup)
pub fn detect_command(text: &str, dictionary_terms: Vec<String>) -> CommandDetection {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    // Shorten command
    if lower.starts_with("shorten this:") || lower.starts_with("shorten:") {
        let prefix_len = if lower.starts_with("shorten this:") {
            13
        } else {
            8
        };
        let content = trimmed[prefix_len..].trim().to_string();
        return CommandDetection {
            task: ProcessingTask::Shorten {
                text: content.clone(),
            },
            content,
            command_name: Some("shorten".to_string()),
        };
    }

    // Make it formal command
    if lower.starts_with("make it formal:") || lower.starts_with("formalize:") {
        let prefix_len = if lower.starts_with("make it formal:") {
            15
        } else {
            10
        };
        let content = trimmed[prefix_len..].trim().to_string();
        return CommandDetection {
            task: ProcessingTask::ChangeTone {
                text: content.clone(),
                target_tone: "formal".to_string(),
            },
            content,
            command_name: Some("formalize".to_string()),
        };
    }

    // Make it casual command
    if lower.starts_with("make it casual:") || lower.starts_with("casualize:") {
        let prefix_len = if lower.starts_with("make it casual:") {
            15
        } else {
            10
        };
        let content = trimmed[prefix_len..].trim().to_string();
        return CommandDetection {
            task: ProcessingTask::ChangeTone {
                text: content.clone(),
                target_tone: "casual".to_string(),
            },
            content,
            command_name: Some("casualize".to_string()),
        };
    }

    // Reply to command
    if lower.starts_with("reply to:") || lower.starts_with("generate reply:") {
        let prefix_len = if lower.starts_with("generate reply:") {
            15
        } else {
            9
        };
        let content = trimmed[prefix_len..].trim().to_string();
        return CommandDetection {
            task: ProcessingTask::GenerateReply {
                context: content.clone(),
            },
            content,
            command_name: Some("reply".to_string()),
        };
    }

    // Translate to [language] command
    if lower.starts_with("translate to ") {
        // Extract the target language and content
        // Format: "translate to [language]: [content]"
        let after_prefix = &trimmed[13..]; // "translate to ".len() = 13

        if let Some(colon_pos) = after_prefix.find(':') {
            let language = after_prefix[..colon_pos].trim().to_string();
            let content = after_prefix[colon_pos + 1..].trim().to_string();

            return CommandDetection {
                task: ProcessingTask::Translate {
                    text: content.clone(),
                    target_language: language.clone(),
                },
                content,
                command_name: Some(format!("translate to {}", language)),
            };
        }
    }

    // No command detected - default to post-processing
    CommandDetection {
        task: ProcessingTask::PostProcess {
            text: trimmed.to_string(),
            dictionary_terms,
        },
        content: trimmed.to_string(),
        command_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_this_command() {
        let text = "shorten this: I would like to inform you that the quarterly financial report has been completed";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::Shorten { .. }));
        assert_eq!(result.command_name, Some("shorten".to_string()));
        assert!(result.content.contains("quarterly financial report"));
    }

    #[test]
    fn test_shorten_command() {
        let text = "shorten: This is a very long sentence that needs to be shortened";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::Shorten { .. }));
        assert_eq!(result.command_name, Some("shorten".to_string()));
    }

    #[test]
    fn test_make_it_formal_command() {
        let text = "make it formal: hey can we chat about the project tomorrow";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::ChangeTone { target_tone, .. } = result.task {
            assert_eq!(target_tone, "formal");
        } else {
            panic!("Expected ChangeTone task");
        }
        assert_eq!(result.command_name, Some("formalize".to_string()));
    }

    #[test]
    fn test_formalize_command() {
        let text = "formalize: thanks for the help";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::ChangeTone { target_tone, .. } = result.task {
            assert_eq!(target_tone, "formal");
        } else {
            panic!("Expected ChangeTone task");
        }
    }

    #[test]
    fn test_reply_to_command() {
        let text = "reply to: Can you attend the meeting at 3pm? Yes I'll be there";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::GenerateReply { .. }));
        assert_eq!(result.command_name, Some("reply".to_string()));
        assert!(result.content.contains("meeting at 3pm"));
    }

    #[test]
    fn test_generate_reply_command() {
        let text = "generate reply: What's the status of the project?";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::GenerateReply { .. }));
    }

    #[test]
    fn test_translate_command() {
        let text = "translate to Chinese: Hello world";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Chinese");
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(
            result.command_name,
            Some("translate to Chinese".to_string())
        );
    }

    #[test]
    fn test_translate_to_japanese() {
        let text = "translate to Japanese: thank you very much";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Japanese");
            assert_eq!(text, "thank you very much");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(
            result.command_name,
            Some("translate to Japanese".to_string())
        );
    }

    #[test]
    fn test_translate_to_spanish() {
        let text = "translate to Spanish: the meeting is at 3pm";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Spanish");
            assert_eq!(text, "the meeting is at 3pm");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(
            result.command_name,
            Some("translate to Spanish".to_string())
        );
    }

    #[test]
    fn test_translate_to_french() {
        let text = "translate to French: good morning everyone";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "French");
            assert_eq!(text, "good morning everyone");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(result.command_name, Some("translate to French".to_string()));
    }

    #[test]
    fn test_translate_to_german() {
        let text = "translate to German: how are you today";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "German");
            assert_eq!(text, "how are you today");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(result.command_name, Some("translate to German".to_string()));
    }

    #[test]
    fn test_translate_to_korean() {
        let text = "translate to Korean: nice to meet you";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Korean");
            assert_eq!(text, "nice to meet you");
        } else {
            panic!("Expected Translate task");
        }
        assert_eq!(result.command_name, Some("translate to Korean".to_string()));
    }

    #[test]
    fn test_translate_case_insensitive() {
        let text = "TRANSLATE TO CHINESE: HELLO WORLD";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language, ..
        } = result.task
        {
            assert_eq!(target_language, "CHINESE");
        } else {
            panic!("Expected Translate task");
        }
    }

    #[test]
    fn test_translate_with_extra_whitespace() {
        let text = "  translate to   Spanish  :   hello   ";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Spanish");
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Translate task");
        }
    }

    #[test]
    fn test_translate_multiword_language() {
        let text = "translate to Traditional Chinese: hello world";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Traditional Chinese");
            assert_eq!(text, "hello world");
        } else {
            panic!("Expected Translate task");
        }
    }

    #[test]
    fn test_translate_complex_content() {
        let text = "translate to Chinese: Hello world, how are you today? The meeting is at 3pm.";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::Translate {
            target_language,
            text,
        } = result.task
        {
            assert_eq!(target_language, "Chinese");
            assert_eq!(
                text,
                "Hello world, how are you today? The meeting is at 3pm."
            );
        } else {
            panic!("Expected Translate task");
        }
    }

    #[test]
    fn test_no_command_post_process() {
        let text = "um so like hello world you know";
        let dict_terms = vec!["API".to_string()];
        let result = detect_command(text, dict_terms.clone());

        if let ProcessingTask::PostProcess {
            dictionary_terms, ..
        } = result.task
        {
            assert_eq!(dictionary_terms, dict_terms);
        } else {
            panic!("Expected PostProcess task");
        }
        assert_eq!(result.command_name, None);
    }

    #[test]
    fn test_case_insensitive() {
        let text = "SHORTEN THIS: LOUD TEXT";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::Shorten { .. }));
    }

    #[test]
    fn test_whitespace_handling() {
        let text = "  shorten:   text with spaces  ";
        let result = detect_command(text, vec![]);

        assert!(matches!(result.task, ProcessingTask::Shorten { .. }));
        assert_eq!(result.content, "text with spaces");
    }

    #[test]
    fn test_make_it_casual() {
        let text = "make it casual: We respectfully request your presence at the formal gathering";
        let result = detect_command(text, vec![]);

        if let ProcessingTask::ChangeTone { target_tone, .. } = result.task {
            assert_eq!(target_tone, "casual");
        } else {
            panic!("Expected ChangeTone task");
        }
        assert_eq!(result.command_name, Some("casualize".to_string()));
    }
}
