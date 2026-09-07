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
