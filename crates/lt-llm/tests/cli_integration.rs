use lt_llm::executor::CliExecutor;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn test_cli_timeout_handling() {
    let executor = CliExecutor::with_timeout(1);

    // This command should timeout (sleeps for 5 seconds)
    let result = executor.execute("sleep", &["5"]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn test_cli_not_found_handling() {
    let executor = CliExecutor::new();

    // This command should not exist
    let result = executor.execute("nonexistent-cli-tool-xyz123", &[]).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[tokio::test]
async fn test_cli_exit_code_handling() {
    let executor = CliExecutor::new();

    // false command exits with code 1
    let result = executor.execute("false", &[]).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_ne!(output.exit_code, 0);
}

#[tokio::test]
async fn test_mock_gemini_cli_success() {
    // Create a mock gemini CLI script
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mock_cli_path = temp_dir.path().join("mock_gemini");

    // Write mock script
    let script = r#"#!/bin/bash
echo '{"text": "Processed successfully"}'
exit 0
"#;
    fs::write(&mock_cli_path, script).unwrap();

    // Make executable
    let mut perms = fs::metadata(&mock_cli_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_cli_path, perms).unwrap();

    let executor = CliExecutor::new();
    let result = executor
        .execute(
            mock_cli_path.to_str().unwrap(),
            &["-p", "test prompt", "--output-format", "json"],
        )
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.contains("Processed successfully"));

    // Cleanup
    fs::remove_file(mock_cli_path).unwrap();
}

#[tokio::test]
async fn test_mock_gemini_cli_failure() {
    // Create a mock gemini CLI script that fails
    let temp_dir = tempfile::TempDir::new().unwrap();
    let mock_cli_path = temp_dir.path().join("mock_gemini_fail");

    // Write mock script that fails
    let script = r#"#!/bin/bash
echo "Error: API key invalid" >&2
exit 1
"#;
    fs::write(&mock_cli_path, script).unwrap();

    // Make executable
    let mut perms = fs::metadata(&mock_cli_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_cli_path, perms).unwrap();

    let executor = CliExecutor::new();
    let result = executor
        .execute(mock_cli_path.to_str().unwrap(), &["-p", "test prompt"])
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("Error"));

    // Cleanup
    fs::remove_file(mock_cli_path).unwrap();
}

#[tokio::test]
async fn test_is_available() {
    let executor = CliExecutor::new();

    // Test with a command that should exist on all Unix systems
    assert!(executor.is_available("echo").await);

    // Test with a command that should not exist
    assert!(!executor.is_available("nonexistent-command-xyz123").await);
    assert!(!executor.is_available("false").await);
}

#[tokio::test]
async fn drains_large_stderr_while_waiting_for_stdout() {
    let output = CliExecutor::with_timeout(3)
        .execute(
            "/bin/sh",
            &["-c", "head -c 1048576 /dev/zero >&2; printf complete"],
        )
        .await
        .unwrap();
    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, "complete");
    assert_eq!(output.stderr.len(), 1_048_576);
}

#[tokio::test]
async fn child_stdin_is_closed() {
    let output = CliExecutor::with_timeout(1)
        .execute("cat", &[])
        .await
        .unwrap();
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.is_empty());
}

async fn process_exists(pid: &str) -> bool {
    tokio::process::Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .unwrap()
        .success()
}

#[tokio::test]
async fn cancellation_kills_and_reaps_child() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let pid_path = temp_dir.path().join("child.pid");
    let task_path = pid_path.clone();
    let task = tokio::spawn(async move {
        CliExecutor::with_timeout(10)
            .execute(
                "/bin/sh",
                &[
                    "-c",
                    "echo $$ > \"$1\"; exec sleep 20",
                    "child",
                    task_path.to_str().unwrap(),
                ],
            )
            .await
    });
    let pid = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Ok(pid) = fs::read_to_string(&pid_path) {
                if !pid.trim().is_empty() {
                    break pid.trim().to_owned();
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(process_exists(&pid).await);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while process_exists(&pid).await {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("canceled child must be killed and reaped");
}

#[tokio::test]
async fn health_check_has_deadline_and_reaps_child() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let cli = temp_dir.path().join("slow-cli");
    let pid_path = temp_dir.path().join("child.pid");
    fs::write(
        &cli,
        format!(
            "#!/bin/sh\necho $$ > '{}'\nexec sleep 20\n",
            pid_path.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
    let available = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        CliExecutor::with_timeout(1).is_available(cli.to_str().unwrap()),
    )
    .await
    .unwrap();
    assert!(!available);
    let pid = fs::read_to_string(pid_path).unwrap();
    assert!(!process_exists(pid.trim()).await);
}

#[tokio::test]
async fn test_cli_stdout_stderr_capture() {
    let executor = CliExecutor::new();

    // Test stdout capture
    let result = executor.execute("echo", &["hello world"]).await.unwrap();
    assert!(result.stdout.contains("hello world"));
    assert!(result.stderr.is_empty());
    assert_eq!(result.exit_code, 0);
}

// ---------------------------------------------------------------------------
// CliLlmProcessor: the behaviour both CLI processors now share.
// ---------------------------------------------------------------------------

use lt_core::llm::{LlmProcessor, ProcessingTask};
use lt_llm::cli::{CliLlmProcessor, CliSpec};
use lt_llm::prompts::PromptManager;

/// Build a spec pointing at a throwaway script. The leaks are bounded by the
/// test binary's lifetime and let the spec satisfy its 'static bound.
fn fake_cli(script: &str) -> (&'static CliSpec, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("fake-cli");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let binary: &'static str = Box::leak(path.to_str().unwrap().to_owned().into_boxed_str());
    let spec: &'static CliSpec = Box::leak(Box::new(CliSpec {
        binary,
        display_name: "Fake CLI",
        default_model: "fake-default",
        install_hint: "Please install fake-cli.",
        args: |prompt, model| {
            vec![
                "--prompt".into(),
                prompt.into(),
                "--model".into(),
                model.into(),
            ]
        },
        parse: |stdout| stdout.trim().to_string(),
    }));
    (spec, dir)
}

fn post_process() -> ProcessingTask {
    ProcessingTask::PostProcess {
        text: "hello".into(),
        dictionary_terms: Vec::new(),
    }
}

#[tokio::test]
async fn the_configured_model_reaches_the_command_line() {
    let (spec, _dir) = fake_cli("#!/bin/sh\nshift 2\necho \"model=$2\"\n");
    let processor =
        CliLlmProcessor::with_model_and_prompts(spec, Some("chosen".into()), PromptManager::new());
    let output = processor.process(post_process()).await.unwrap();
    assert_eq!(output.text, "model=chosen");
}

#[tokio::test]
async fn an_absent_model_override_falls_back_to_the_spec_default() {
    let (spec, _dir) = fake_cli("#!/bin/sh\nshift 2\necho \"model=$2\"\n");
    let processor = CliLlmProcessor::with_model_and_prompts(spec, None, PromptManager::new());
    assert_eq!(processor.model(), "fake-default");
    let output = processor.process(post_process()).await.unwrap();
    assert_eq!(output.text, "model=fake-default");
}

#[tokio::test]
async fn a_nonzero_exit_reports_the_tool_name_and_its_stderr() {
    let (spec, _dir) = fake_cli("#!/bin/sh\necho 'boom' >&2\nexit 3\n");
    let processor = CliLlmProcessor::with_model_and_prompts(spec, None, PromptManager::new());
    let error = processor
        .process(post_process())
        .await
        .expect_err("a failing tool must not look successful")
        .to_string();
    assert!(error.contains("Fake CLI failed"), "{error}");
    assert!(error.contains("boom"), "{error}");
}

#[tokio::test]
async fn a_missing_binary_reports_the_install_hint() {
    static MISSING: CliSpec = CliSpec {
        binary: "nonexistent-cli-tool-xyz123",
        display_name: "Fake CLI",
        default_model: "fake-default",
        install_hint: "Please install fake-cli.",
        args: |prompt, _| vec![prompt.into()],
        parse: |stdout| stdout.trim().to_string(),
    };
    let processor = CliLlmProcessor::with_model_and_prompts(&MISSING, None, PromptManager::new());
    let error = processor
        .process(post_process())
        .await
        .expect_err("a missing binary must be an error")
        .to_string();
    assert!(error.contains("Fake CLI not found"), "{error}");
    assert!(error.contains("Please install fake-cli."), "{error}");

    assert!(!processor.health_check().await.unwrap());
}
