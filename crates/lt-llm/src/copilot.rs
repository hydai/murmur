use crate::cli::{cli_processor, CliSpec};

pub const DEFAULT_MODEL: &str = "gpt-5-mini";

static SPEC: CliSpec = CliSpec {
    binary: "copilot",
    display_name: "Copilot CLI",
    default_model: DEFAULT_MODEL,
    install_hint: "Please install copilot-cli.",
    args: |prompt, model| {
        vec![
            "--prompt".into(),
            prompt.into(),
            "--model".into(),
            model.into(),
        ]
    },
    parse: |stdout| stdout.trim().to_string(),
};

cli_processor!(CopilotProcessor, SPEC);

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::llm::LlmProcessor;

    #[tokio::test]
    #[ignore = "requires a locally installed Copilot CLI"]
    async fn test_copilot_health_check() {
        let processor = CopilotProcessor::new();
        // This will return false if copilot is not installed, which is expected
        let _ = processor.health_check().await;
    }

    #[test]
    fn the_configured_model_falls_back_to_the_default() {
        assert_eq!(
            CopilotProcessor::with_model(None).inner.model(),
            DEFAULT_MODEL
        );
        assert_eq!(
            CopilotProcessor::with_model(Some("gpt-x".into()))
                .inner
                .model(),
            "gpt-x"
        );
    }

    #[test]
    fn arguments_always_pass_a_model() {
        assert_eq!(
            (SPEC.args)("say hi", "gpt-x"),
            ["--prompt", "say hi", "--model", "gpt-x"]
        );
    }
}
