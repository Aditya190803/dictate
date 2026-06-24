use crate::command;
use anyhow::Result;
use async_trait::async_trait;
use std::io::Write;
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait TypingBackend: Send + Sync {
    async fn type_text(&self, text: &str) -> Result<()>;
    async fn backspace(&self, count: usize) -> Result<()>;
}

pub struct OutputBackend {
    pipe_command: Option<Vec<String>>,
}

impl OutputBackend {
    pub fn new(pipe_command: Option<Vec<String>>) -> Self {
        Self { pipe_command }
    }
}

#[async_trait]
impl TypingBackend for OutputBackend {
    async fn type_text(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        if let Some(cmd) = &self.pipe_command {
            command::execute_with_input(cmd, text).await?;
        } else {
            print!("{}", text);
            std::io::stdout().flush().ok();
        }
        Ok(())
    }

    async fn backspace(&self, count: usize) -> Result<()> {
        if count == 0 {
            return Ok(());
        }

        if let Some(cmd) = &self.pipe_command {
            if cmd.first().is_some_and(|name| name.ends_with("ydotool")) {
                let mut key_cmd = vec!["ydotool".to_string(), "key".to_string()];
                for _ in 0..count {
                    key_cmd.push("14:1".to_string());
                    key_cmd.push("14:0".to_string());
                }
                command::execute_with_input(&key_cmd, "").await?;
            } else {
                let backspaces = "\u{0008}".repeat(count);
                command::execute_with_input(cmd, &backspaces).await?;
            }
        } else {
            print!("{}", "\u{0008}".repeat(count));
            std::io::stdout().flush().ok();
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub struct MockTypingBackend {
    operations: Arc<Mutex<Vec<TypingOperation>>>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypingOperation {
    Type(String),
    Backspace(usize),
}

#[cfg(test)]
impl MockTypingBackend {
    pub fn operations(&self) -> Vec<TypingOperation> {
        self.operations.lock().unwrap().clone()
    }
}

#[cfg(test)]
#[async_trait]
impl TypingBackend for MockTypingBackend {
    async fn type_text(&self, text: &str) -> Result<()> {
        self.operations
            .lock()
            .unwrap()
            .push(TypingOperation::Type(text.to_string()));
        Ok(())
    }

    async fn backspace(&self, count: usize) -> Result<()> {
        self.operations
            .lock()
            .unwrap()
            .push(TypingOperation::Backspace(count));
        Ok(())
    }
}
