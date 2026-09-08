use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
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
    pub async fn execute(&self, program: &str, args: &[&str]) -> Result<CliOutput, std::io::Error> {
        self.execute_with_timeout(program, args, Duration::from_secs(self.timeout_secs))
            .await
    }

    async fn execute_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        duration: Duration,
    ) -> Result<CliOutput, std::io::Error> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // A dedicated owner can kill AND reap the child even when the caller
        // drops its future. kill_on_drop also covers runtime shutdown.
        let (mut tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let result = tokio::select! {
                biased;
                _ = tx.closed() => None,
                result = timeout(duration, collect_output(&mut child)) => Some(match result {
                    Ok(result) => result,
                    Err(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("Command timed out after {} seconds", duration.as_secs()),
                    )),
                }),
            };
            if child.id().is_some() {
                let _ = child.kill().await;
            }
            if let Some(result) = result {
                let _ = tx.send(result);
            }
        });

        rx.await.map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "CLI execution task stopped",
            )
        })?
    }

    /// Check if a CLI tool is available in PATH
    pub async fn is_available(&self, program: &str) -> bool {
        self.execute_with_timeout(
            program,
            &["--version"],
            Duration::from_secs(self.timeout_secs.min(5)),
        )
        .await
        .is_ok_and(|output| output.exit_code == 0)
    }
}

async fn collect_output(child: &mut Child) -> Result<CliOutput, std::io::Error> {
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    // Both pipes must drain while the child runs, otherwise a full stderr
    // pipe can prevent it from ever closing stdout.
    let (_, _, status) = tokio::try_join!(
        stdout.read_to_end(&mut stdout_bytes),
        stderr.read_to_end(&mut stderr_bytes),
        child.wait(),
    )?;
    Ok(CliOutput {
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        exit_code: status.code().unwrap_or(-1),
    })
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
