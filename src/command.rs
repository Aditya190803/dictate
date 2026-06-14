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

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_capture_success() {
        let _lock = ENV_MUTEX.lock().await;
        let command_args = vec!["printf".to_string(), "hello".to_string()];

        let result = execute_capture(&command_args).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_capture_failure() {
        let _lock = ENV_MUTEX.lock().await;
        let command_args = vec!["false".to_string()];

        let result = execute_capture(&command_args).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_success() {
        let _lock = ENV_MUTEX.lock().await;

        // Test with 'cat' command which should echo input to stdout
        let command_args = vec!["cat".to_string()];
        let input = "Hello, World!";

        let result = execute_with_input(&command_args, input).await;

        // cat should succeed with exit code 0
        assert!(result.is_ok());
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
    async fn test_execute_with_input_command_with_args() {
        let _lock = ENV_MUTEX.lock().await;

        // Test with 'head -n 1' to demonstrate argument handling
        let command_args = vec!["head".to_string(), "-n".to_string(), "1".to_string()];
        let input = "line1\nline2\nline3";

        let result = execute_with_input(&command_args, input).await;

        // head should succeed with exit code 0
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_execute_with_input_command_failure() {
        let _lock = ENV_MUTEX.lock().await;

        // Test with a command that reads stdin and exits with code 1
        let command_args = vec![
            "sh".to_string(),
            "-c".to_string(),
            "cat > /dev/null; exit 1".to_string(),
        ];
        let input = "test";

        let result = execute_with_input(&command_args, input).await;

        // Command should execute but return exit code 1
        assert!(
            result.is_ok(),
            "Command should execute successfully: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 1);
    }
}
