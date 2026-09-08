use crate::cli::{cli_processor, CliSpec};

pub const DEFAULT_MODEL: &str = "gemini-3-flash-preview";

static SPEC: CliSpec = CliSpec {
    binary: "gemini",
    display_name: "Gemini CLI",
    default_model: DEFAULT_MODEL,
    install_hint: "Please install gemini-cli: https://github.com/google/generative-ai-cli",
    args: |prompt, model| {
        vec![
            "-p".into(),
            prompt.into(),
            "--output-format".into(),
            "json".into(),
            "-m".into(),
            model.into(),
        ]
    },
    parse: parse_json_output,
};

/// Pull the reply out of `--output-format json`, tolerating the plain text the
/// CLI still returns for some responses.
fn parse_json_output(output: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
        for field in ["text", "content", "response"] {
            if let Some(text) = json.get(field).and_then(|value| value.as_str()) {
                return text.to_string();
            }
        }
        if let Some(text) = json.as_str() {
            return text.to_string();
        }
    }
    output.trim().to_string()
}

cli_processor!(GeminiProcessor, SPEC);

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::llm::LlmProcessor;

    #[tokio::test]
    #[ignore = "requires a locally installed Gemini CLI"]
    async fn test_gemini_health_check() {
        let processor = GeminiProcessor::new();
        // This will return false if gemini is not installed, which is expected
        let _ = processor.health_check().await;
    }

    #[test]
    fn test_parse_json_output() {
        assert_eq!(
            parse_json_output(r#"{"text": "Hello world"}"#),
            "Hello world"
        );
        assert_eq!(
            parse_json_output(r#"{"content": "Hello world"}"#),
            "Hello world"
        );
        assert_eq!(
            parse_json_output(r#"{"response": "Hello world"}"#),
            "Hello world"
        );
        assert_eq!(parse_json_output("  Hello world  "), "Hello world");
    }

    #[test]
    fn the_configured_model_falls_back_to_the_default() {
        assert_eq!(
            GeminiProcessor::with_model(Some(String::new()))
                .inner
                .model(),
            DEFAULT_MODEL
        );
        assert_eq!(
            GeminiProcessor::with_model(Some("gemini-x".into()))
                .inner
                .model(),
            "gemini-x"
        );
    }

    #[test]
    fn arguments_carry_the_prompt_and_model() {
        let args = (SPEC.args)("say hi", "gemini-x");
        assert_eq!(
            args,
            ["-p", "say hi", "--output-format", "json", "-m", "gemini-x"]
        );
    }
}
