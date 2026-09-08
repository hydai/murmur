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
///
/// A command with no content after the colon, or a translate whose language is
/// longer than three words, is treated as ordinary speech.
pub fn detect_command(text: &str, dictionary_terms: Vec<String>) -> CommandDetection {
    let trimmed = text.trim();

    if let Some(content) = content_after(trimmed, &["shorten this:", "shorten:"]) {
        return CommandDetection {
            task: ProcessingTask::Shorten {
                text: content.clone(),
            },
            content,
            command_name: Some("shorten".to_string()),
        };
    }

    if let Some(content) = content_after(trimmed, &["make it formal:", "formalize:"]) {
        return CommandDetection {
            task: ProcessingTask::ChangeTone {
                text: content.clone(),
                target_tone: "formal".to_string(),
            },
            content,
            command_name: Some("formalize".to_string()),
        };
    }

    if let Some(content) = content_after(trimmed, &["make it casual:", "casualize:"]) {
        return CommandDetection {
            task: ProcessingTask::ChangeTone {
                text: content.clone(),
                target_tone: "casual".to_string(),
            },
            content,
            command_name: Some("casualize".to_string()),
        };
    }

    if let Some(content) = content_after(trimmed, &["reply to:", "generate reply:"]) {
        return CommandDetection {
            task: ProcessingTask::GenerateReply {
                context: content.clone(),
            },
            content,
            command_name: Some("reply".to_string()),
        };
    }

    // "translate to [language]: [content]" — the language is the short phrase
    // before the first colon; anything longer is ordinary speech.
    if let Some(rest) = strip_prefix_ignore_ascii_case(trimmed, "translate to ") {
        if let Some((language, content)) = rest.split_once(':') {
            let language = language.trim();
            let content = content.trim();
            if is_language_name(language) && !content.is_empty() {
                return CommandDetection {
                    task: ProcessingTask::Translate {
                        text: content.to_string(),
                        target_language: language.to_string(),
                    },
                    content: content.to_string(),
                    command_name: Some(format!("translate to {language}")),
                };
            }
        }
    }

    // No usable command: clean up the whole utterance instead.
    CommandDetection {
        task: ProcessingTask::PostProcess {
            text: trimmed.to_string(),
            dictionary_terms,
        },
        content: trimmed.to_string(),
        command_name: None,
    }
}

/// Case-insensitive ASCII prefix match on a char boundary. The remainder is
/// taken from the original text, never from a lowercased copy whose byte
/// offsets can differ.
fn strip_prefix_ignore_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// The non-empty content after the first matching prefix.
fn content_after(text: &str, prefixes: &[&str]) -> Option<String> {
    prefixes.iter().find_map(|prefix| {
        let content = strip_prefix_ignore_ascii_case(text, prefix)?.trim();
        (!content.is_empty()).then(|| content.to_string())
    })
}

/// A spoken language name: one to three words of letters, hyphens, or
/// parentheses, such as "Japanese" or "Traditional Chinese (Taiwan)".
fn is_language_name(language: &str) -> bool {
    let words: Vec<&str> = language.split_whitespace().collect();
    !words.is_empty()
        && words.len() <= 3
        && words.iter().all(|word| {
            word.chars()
                .all(|c| c.is_alphabetic() || matches!(c, '-' | '(' | ')'))
        })
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

    fn is_post_process_of_whole_text(result: &CommandDetection, text: &str) -> bool {
        result.command_name.is_none()
            && result.content == text.trim()
            && matches!(&result.task, ProcessingTask::PostProcess { text: t, .. } if t == text.trim())
    }

    #[test]
    fn translate_requires_a_short_language_before_the_colon() {
        let text = "translate to my friend what I said yesterday: hello there";
        let result = detect_command(text, vec![]);
        assert!(is_post_process_of_whole_text(&result, text), "{result:?}");

        let text = "translate to Traditional Chinese (Taiwan): hello";
        let result = detect_command(text, vec![]);
        assert_eq!(
            result.command_name.as_deref(),
            Some("translate to Traditional Chinese (Taiwan)")
        );
    }

    #[test]
    fn empty_command_content_falls_back_to_post_processing() {
        for text in [
            "translate to French:",
            "shorten:",
            "make it formal:  ",
            "reply to:",
        ] {
            let result = detect_command(text, vec![]);
            assert!(
                is_post_process_of_whole_text(&result, text),
                "{text:?} -> {result:?}"
            );
        }
    }

    #[test]
    fn non_ascii_lookalikes_do_not_corrupt_the_content() {
        // U+212A KELVIN SIGN lowercases to ASCII "k", so byte offsets taken
        // from the lowercased text do not apply to the original.
        let text = "ma\u{212A}e it formal: hello";
        let result = detect_command(text, vec![]);
        assert!(is_post_process_of_whole_text(&result, text), "{result:?}");
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
