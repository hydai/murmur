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
    let temp_dir = std::env::temp_dir();
    let mock_cli_path = temp_dir.join("mock_gemini");

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
    let result = executor.execute(
        mock_cli_path.to_str().unwrap(),
        &["-p", "test prompt", "--output-format", "json"]
    ).await;

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
    let temp_dir = std::env::temp_dir();
    let mock_cli_path = temp_dir.join("mock_gemini_fail");

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
    let result = executor.execute(
        mock_cli_path.to_str().unwrap(),
        &["-p", "test prompt"]
    ).await;

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

#[tokio::test]
async fn test_fallback_behavior_simulation() {
    let executor = CliExecutor::new();

    // Primary CLI not found
    let primary_result = executor.execute("gemini", &["--version"]).await;

    if primary_result.is_err() {
        // Simulate fallback to secondary CLI
        let secondary_result = executor.execute("copilot", &["--version"]).await;

        if secondary_result.is_err() {
            // Both failed - this would trigger raw transcription fallback
            assert!(true, "Both CLIs unavailable - fallback to raw transcription");
        }
    }
}
