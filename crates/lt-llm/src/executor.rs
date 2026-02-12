use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// CLI execution result
#[derive(Debug)]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// CLI executor for spawning subprocess and capturing output
pub struct CliExecutor {
    timeout_secs: u64,
}

impl CliExecutor {
    /// Create a new CLI executor with default timeout (30 seconds)
    pub fn new() -> Self {
        Self { timeout_secs: 30 }
    }

    /// Create a new CLI executor with custom timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Execute a CLI command and capture output
    pub async fn execute(
        &self,
        program: &str,
        args: &[&str],
    ) -> Result<CliOutput, std::io::Error> {
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        // Wait for process to complete with timeout
        let result = timeout(timeout_duration, async {
            let stdout = if let Some(mut stdout) = child.stdout.take() {
                let mut buf = String::new();
                stdout.read_to_string(&mut buf).await?;
                buf
            } else {
                String::new()
            };

            let stderr = if let Some(mut stderr) = child.stderr.take() {
                let mut buf = String::new();
                stderr.read_to_string(&mut buf).await?;
                buf
            } else {
                String::new()
            };

            let status = child.wait().await?;

            Ok::<CliOutput, std::io::Error>(CliOutput {
                stdout,
                stderr,
                exit_code: status.code().unwrap_or(-1),
            })
        })
        .await;

        match result {
            Ok(output) => output,
            Err(_) => {
                // Kill the process on timeout
                let _ = child.kill().await;
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Command timed out after {} seconds", self.timeout_secs),
                ))
            }
        }
    }

    /// Check if a CLI tool is available in PATH
    pub async fn is_available(&self, program: &str) -> bool {
        let result = Command::new(program)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        result.is_ok()
    }
}

impl Default for CliExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_success() {
        let executor = CliExecutor::new();
        let output = executor.execute("echo", &["hello"]).await.unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_executor_failure() {
        let executor = CliExecutor::new();
        let result = executor.execute("false", &[]).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_ne!(output.exit_code, 0);
    }

    #[tokio::test]
    async fn test_is_available() {
        let executor = CliExecutor::new();
        // Most systems should have 'echo'
        assert!(executor.is_available("echo").await);
        // This command should not exist
        assert!(!executor.is_available("nonexistent-command-xyz").await);
    }
}
