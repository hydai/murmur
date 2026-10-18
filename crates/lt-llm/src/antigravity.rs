use crate::cli::{cli_processor, CliSpec};
use lt_core::error::{MurmurError, Result};
use serde::Deserialize;

// Antigravity uses model slugs rather than Gemini API IDs (`agy models`).
pub const DEFAULT_MODEL: &str = "gemini-3.8-flash-medium";
pub const INSTALL_HINT: &str =
    "Install Antigravity CLI: https://antigravity.google/docs/getting-started/; then run agy once to sign in.";

static SPEC: CliSpec = CliSpec {
    binary: "agy",
    fallback_binary: Some(".local/bin/agy"),
    display_name: "Antigravity CLI",
    default_model: DEFAULT_MODEL,
    install_hint: INSTALL_HINT,
    args: |prompt, model| {
        vec![
            "--print".into(),
            prompt.into(),
            "--model".into(),
            model.into(),
            "--output-format".into(),
            "json".into(),
            // Leave time for the CLI to report an error before our 30s deadline.
            "--print-timeout".into(),
            "25s".into(),
        ]
    },
    parse: parse_json_output,
};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RunStatus {
    Success,
    Error,
    Canceled,
    Interrupted,
    Invalid,
    Waiting,
    Running,
}

#[derive(Deserialize)]
struct PrintResponse {
    status: RunStatus,
    response: Option<String>,
}

fn parse_json_output(stdout: &str) -> Result<String> {
    let result: PrintResponse = serde_json::from_str(stdout).map_err(|_| {
        MurmurError::Llm("Antigravity CLI returned invalid JSON output. Update agy if your version does not support --output-format json.".into())
    })?;
    if result.status != RunStatus::Success {
        return Err(MurmurError::Llm(format!(
            "Antigravity CLI did not complete successfully ({:?})",
            result.status
        )));
    }
    result
        .response
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| MurmurError::Llm("Antigravity CLI returned an empty response".into()))
}

cli_processor!(AntigravityProcessor, SPEC);

#[cfg(test)]
mod tests {
    use super::*;
    use lt_core::llm::{LlmProcessor, ProcessingTask};
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn print_mode_returns_only_the_response_and_passes_the_model() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agy");
        std::fs::write(
            &path,
            r#"#!/bin/sh
[ "$#" = 8 ] && [ "$1" = --print ] && [ -n "$2" ] &&
[ "$3" = --model ] && [ "$4" = gemini-3.8-flash-medium ] &&
[ "$5" = --output-format ] && [ "$6" = json ] &&
[ "$7" = --print-timeout ] && [ "$8" = 25s ] || exit 2
printf 'progress and usage\n' >&2
printf '{"status":"SUCCESS","response":"處理完成。","usage":{"total_tokens":123}}\n'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let spec = Box::leak(Box::new(CliSpec {
            binary: Box::leak(path.to_str().unwrap().to_owned().into_boxed_str()),
            ..SPEC
        }));
        let processor = crate::cli::CliLlmProcessor::with_model_and_prompts(
            spec,
            None,
            crate::prompts::PromptManager::new(),
        );
        let output = processor
            .process(ProcessingTask::Shorten {
                text: "測試文字".into(),
            })
            .await
            .unwrap();
        assert_eq!(output.text, "處理完成。");
    }

    #[test]
    fn model_overrides_are_passed_without_rewriting() {
        let processor = AntigravityProcessor::with_model(Some("custom-model".into()));
        assert_eq!(
            (SPEC.args)("say hi", processor.inner.model()),
            [
                "--print",
                "say hi",
                "--model",
                "custom-model",
                "--output-format",
                "json",
                "--print-timeout",
                "25s"
            ]
        );
        assert_eq!(
            AntigravityProcessor::with_model(Some(String::new()))
                .inner
                .model(),
            DEFAULT_MODEL
        );
    }

    #[test]
    fn unsuccessful_or_malformed_output_cannot_be_pasted_as_a_transcript() {
        for status in [
            "ERROR",
            "CANCELED",
            "INTERRUPTED",
            "INVALID",
            "WAITING",
            "RUNNING",
            "UNKNOWN",
        ] {
            let json = serde_json::json!({"status": status, "response": "private-transcript", "error": "private-transcript"});
            let error = parse_json_output(&json.to_string())
                .unwrap_err()
                .to_string();
            assert!(!error.contains("private-transcript"));
        }
        for output in [
            "private-transcript",
            "{}",
            r#"{"status":"SUCCESS"}"#,
            r#"{"status":"SUCCESS","response":"  "}"#,
        ] {
            assert!(parse_json_output(output).is_err());
        }
    }
}
