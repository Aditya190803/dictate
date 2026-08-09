use crate::platform;
use anyhow::{anyhow, Result};
use log::debug;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Execute a command and capture stdout as UTF-8 text.
pub async fn execute_capture(command_args: &[String]) -> Result<String> {
    if command_args.is_empty() {
        return Err(anyhow!("No command provided"));
    }

    if platform::is_internal(command_args) {
        return platform::capture_internal(&command_args[1..]).await;
    }

    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new(&command_args[0])
            .args(&command_args[1..])
            .output(),
    )
    .await
    .map_err(|_| anyhow!("Command '{}' timed out after 30s", command_args[0]))?
    .map_err(|e| anyhow!("Failed to execute command '{}': {}", command_args[0], e))?;

    if !output.status.success() {
        return Err(anyhow!(
            "Command '{}' exited with code {}",
            command_args[0],
            output.status.code().unwrap_or(-1)
        ));
    }

    String::from_utf8(output.stdout).map_err(|e| anyhow!("Command output was not UTF-8: {}", e))
}

/// Execute a command with the given arguments, piping the provided input to its stdin
pub async fn execute_with_input(command_args: &[String], input: &str) -> Result<i32> {
    if command_args.is_empty() {
        return Err(anyhow!("No command provided"));
    }

    if platform::is_internal(command_args) {
        debug!("Internal sink: {:?}", &command_args[1..]);
        return platform::run_internal_sink(&command_args[1..], input).await;
    }

    let command_name = &command_args[0];
    let args = &command_args[1..];

    debug!("Executing command: {} {:?}", command_name, args);
    debug!("Input length: {} characters", input.len());

    let mut child = Command::new(command_name)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow!("Failed to execute command '{}': {}", command_name, e))?;

    // Get stdin handle and write input
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| anyhow!("Failed to write to command stdin: {}", e))?;

        // Close stdin to signal EOF
        stdin
            .shutdown()
            .await
            .map_err(|e| anyhow!("Failed to close stdin: {}", e))?;
    } else {
        return Err(anyhow!("Failed to get stdin handle for command"));
    }

    // Wait for the command to complete
    let output = child
        .wait()
        .await
        .map_err(|e| anyhow!("Failed to wait for command completion: {}", e))?;

    let exit_code = output.code().unwrap_or(-1);
    debug!("Command completed with exit code: {}", exit_code);

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_MUTEX;

    /// Command that writes `text` to stdout. Trailing newlines vary by shell,
    /// so callers compare trimmed output.
    fn echo(text: &str) -> Vec<String> {
        #[cfg(unix)]
        return vec!["printf".to_string(), text.to_string()];
        #[cfg(windows)]
        return vec!["cmd".to_string(), "/c".to_string(), format!("echo {text}")];
    }

    /// Command that exits non-zero.
    fn fails() -> Vec<String> {
        #[cfg(unix)]
        return vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()];
        #[cfg(windows)]
        return vec!["cmd".to_string(), "/c".to_string(), "exit 1".to_string()];
    }

    /// Command that reads stdin to EOF and succeeds.
    fn consumes_stdin() -> Vec<String> {
        #[cfg(unix)]
        return vec!["cat".to_string()];
        #[cfg(windows)]
        return vec!["cmd".to_string(), "/c".to_string(), "sort".to_string()];
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_capture_success() {
        let _lock = ENV_MUTEX.lock().await;

        let result = execute_capture(&echo("hello")).await;

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_capture_failure() {
        let _lock = ENV_MUTEX.lock().await;

        let result = execute_capture(&fails()).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_success() {
        let _lock = ENV_MUTEX.lock().await;

        let result = execute_with_input(&consumes_stdin(), "Hello, World!").await;

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_empty_command() {
        let _lock = ENV_MUTEX.lock().await;

        let command_args = vec![];
        let input = "test";

        let result = execute_with_input(&command_args, input).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No command provided"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_nonexistent_command() {
        let _lock = ENV_MUTEX.lock().await;

        let command_args = vec!["nonexistent_command_12345".to_string()];
        let input = "test";

        let result = execute_with_input(&command_args, input).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to execute command"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_command_failure() {
        let _lock = ENV_MUTEX.lock().await;

        let result = execute_with_input(&fails(), "test").await;

        // The command runs; only its exit status reports the failure.
        assert!(
            result.is_ok(),
            "Command should execute successfully: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn internal_sinks_are_not_spawned_as_processes() {
        let _lock = ENV_MUTEX.lock().await;

        // `@dictate` never exists on PATH, so reaching a spawn would report
        // "Failed to execute command" instead of an unknown-sink error.
        let sink = vec![
            crate::platform::INTERNAL_CMD.to_string(),
            "definitely-not-a-sink".to_string(),
        ];

        let error = execute_with_input(&sink, "text")
            .await
            .expect_err("unknown sink should fail");
        assert!(
            !error.to_string().contains("Failed to execute command"),
            "internal sink leaked to process spawning: {error}"
        );
    }
}
