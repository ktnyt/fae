//! Command execution actor for running external processes
//!
//! This module provides a CommandActor that can execute external commands
//! asynchronously and stream their stdout output line by line through the
//! actor messaging system. It supports both simple string output and custom
//! type conversion through the CommandHandler trait.

use crate::core::{
    message::{Message, MessageHandler, CommandController},
    ActorSender,
};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

/// Trait for handling command output lines with full control over messaging.
///
/// This trait allows for flexible processing of stdout lines from executed commands.
/// Handlers have direct access to the ActorSender to send messages as needed,
/// enabling complex processing, filtering, and multiple message generation.
#[async_trait]
pub trait CommandHandler<T>: Send + Sync {
    /// Process a single line of stdout and handle messaging directly.
    ///
    /// # Arguments
    /// * `line` - A single line of stdout from the executed command
    /// * `sender` - Direct access to send messages to the actor system
    ///
    /// The handler can:
    /// - Send processed data as "output" messages
    /// - Send multiple messages per line
    /// - Filter out unwanted lines (send nothing)
    /// - Send error/warning messages for malformed data
    /// - Perform logging or other side effects
    ///
    /// # Example
    /// ```
    /// async fn process_line(&self, line: String, sender: &ActorSender<String>) {
    ///     let uppercase = line.to_uppercase();
    ///     let _ = sender.send_message("output", uppercase).await;
    /// }
    /// ```
    async fn process_line(&self, line: String, sender: &ActorSender<T>);
}

/// Simple pass-through handler for String output.
///
/// This handler sends each line as-is as an "output" message.
/// It's the default behavior equivalent to the original CommandActor.
#[derive(Debug, Clone)]
pub struct StringHandler;

#[async_trait]
impl CommandHandler<String> for StringHandler {
    async fn process_line(&self, line: String, sender: &ActorSender<String>) {
        let _ = sender.send_message("output", line).await;
    }
}

/// Actor that executes external commands and streams their output.
///
/// The CommandActor takes a command string in its constructor and executes it
/// when it receives messages. The stdout of the command is read line by line
/// and each line is sent as a separate message through the actor system.
/// Generic command actor that supports custom output type conversion and cancellation.
///
/// This actor is more flexible than the original CommandActor, supporting:
/// - Custom type conversion through CommandHandler trait
/// - Command cancellation through CancellationToken
/// - Process lifecycle management
///
/// Note: The actor still sends string messages for system events (cancelled, completed, error)
/// but uses the custom type T for processed stdout lines.
#[derive(Debug)]
pub struct CommandActor<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    /// The command to execute (e.g., "echo hello\nworld")
    command: String,
    /// Handler for processing stdout lines
    handler: H,
    /// Cancellation token for stopping execution
    cancellation_token: CancellationToken,
    /// Phantom data for type T
    _phantom: std::marker::PhantomData<T>,
}

impl<T, H> CommandActor<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    /// Create a new GenericCommandActor with the specified command and handler.
    ///
    /// # Arguments
    /// * `command` - The shell command to execute when triggered
    /// * `handler` - The handler for processing stdout lines
    ///
    /// # Example
    /// ```
    /// let handler = StringHandler;
    /// let actor = GenericCommandActor::new("echo 'Hello World'", handler);
    /// ```
    pub fn new(command: impl Into<String>, handler: H) -> Self {
        Self {
            command: command.into(),
            handler,
            cancellation_token: CancellationToken::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Execute the command with cancellation support and custom type conversion.
    ///
    /// This method runs the command asynchronously and sends each processed line
    /// as a separate message. The execution can be cancelled through the "cancel"
    /// message method.
    async fn execute_command(&mut self, sender: &ActorSender<T>) -> Result<(), CommandError> {
        log::debug!("Executing command: {}", self.command);

        // Create the command process
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // Ignore stderr for now
            .spawn()
            .map_err(CommandError::SpawnFailed)?;

        // Get stdout handle
        let stdout = child
            .stdout
            .take()
            .ok_or(CommandError::StdoutNotAvailable)?;

        // Create a buffered reader for line-by-line reading
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // Read lines and send each as a message (with cancellation support)
        loop {
            tokio::select! {
                // Check for cancellation
                _ = self.cancellation_token.cancelled() => {
                    log::info!("Command execution cancelled");

                    // Kill the child process
                    if let Err(e) = child.kill().await {
                        log::warn!("Failed to kill child process: {}", e);
                    }

                    // Note: System messages are not sent in the new design
                    // Handlers can implement their own cancellation logic if needed
                    log::info!("Command cancelled - no system message sent");
                    return Err(CommandError::Cancelled);
                }

                // Read next line
                line_result = lines.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            log::trace!("Command output line: {}", line);

                            // Process the line through the handler (handler manages messaging)
                            self.handler.process_line(line, sender).await;
                        }
                        Ok(None) => {
                            // End of stream
                            break;
                        }
                        Err(e) => {
                            log::error!("Error reading line: {}", e);
                            return Err(CommandError::ReadFailed(e));
                        }
                    }
                }
            }
        }

        // Wait for the command to complete
        let status = child.wait().await.map_err(CommandError::WaitFailed)?;

        log::debug!("Command completed with status: {}", status);

        // Send completion message
        let completion_message = if status.success() {
            "Command completed successfully".to_string()
        } else {
            format!("Command failed with exit code: {:?}", status.code())
        };

        // Note: System messages are not sent in the new design
        // Handlers can implement their own completion logic if needed
        log::info!("Command completed: {}", completion_message);

        Ok(())
    }
}

#[async_trait]
impl<T, H> MessageHandler<T> for CommandActor<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    /// Handle incoming messages to execute or cancel the command.
    ///
    /// - "cancel" method: Cancels the currently running command
    /// - Any other method: Executes the configured command
    async fn on_message(&mut self, message: Message<T>, sender: &ActorSender<T>) {
        log::debug!(
            "GenericCommandActor received message: method={}",
            message.method
        );

        match message.method.as_str() {
            "cancel" => {
                log::info!("Cancellation requested");
                self.cancellation_token.cancel();
            }
            _ => {
                // Reset cancellation token for new execution
                self.cancellation_token = CancellationToken::new();

                // Execute the command
                if let Err(e) = self.execute_command(sender).await {
                    log::error!("Command execution failed: {:?}", e);

                    // Send error message
                    let error_msg = format!("Command execution failed: {:?}", e);
                    // Note: System messages are not sent in the new design
                    // Handlers can implement their own error logic if needed
                    log::error!("Command execution error: {}", error_msg);
                }
            }
        }
    }
}

/// Type alias for backward compatibility.
/// Creates a CommandActor that processes strings with StringHandler.
pub type StringCommandActor = CommandActor<String, StringHandler>;

impl StringCommandActor {
    /// Create a new StringCommandActor with the specified command.
    /// This provides the same API as the original CommandActor.
    pub fn new_string(command: impl Into<String>) -> Self {
        Self::new(command, StringHandler)
    }
}

/// Error types for command execution
#[derive(Debug)]
pub enum CommandError {
    /// Failed to spawn the command process
    SpawnFailed(std::io::Error),
    /// Stdout pipe was not available
    StdoutNotAvailable,
    /// Failed to wait for command completion
    WaitFailed(std::io::Error),
    /// Failed to read from stdout
    ReadFailed(std::io::Error),
    /// Command execution was cancelled
    Cancelled,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::SpawnFailed(e) => write!(f, "Failed to spawn command: {}", e),
            CommandError::StdoutNotAvailable => write!(f, "Stdout pipe not available"),
            CommandError::WaitFailed(e) => write!(f, "Failed to wait for command: {}", e),
            CommandError::ReadFailed(e) => write!(f, "Failed to read from stdout: {}", e),
            CommandError::Cancelled => write!(f, "Command execution was cancelled"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Command controller implementation for CommandActor.
/// 
/// This struct implements CommandController to provide both message sending
/// and command lifecycle control to MessageHandler implementations.
pub struct CommandActorController<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    actor_sender: ActorSender<T>,
    command_actor: std::sync::Arc<tokio::sync::Mutex<CommandActor<T, H>>>,
}

impl<T, H> CommandActorController<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    pub fn new(
        actor_sender: ActorSender<T>,
        command_actor: std::sync::Arc<tokio::sync::Mutex<CommandActor<T, H>>>,
    ) -> Self {
        Self {
            actor_sender,
            command_actor,
        }
    }
}

#[async_trait]
impl<T, H> CommandController<T> for CommandActorController<T, H>
where
    T: Send + 'static,
    H: CommandHandler<T> + Send + 'static,
{
    async fn send_message(&self, method: impl Into<String> + Send, payload: T) -> Result<(), crate::core::ActorSendError> {
        self.actor_sender.send_message(method, payload).await
    }
    
    async fn spawn(&self, command: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut actor = self.command_actor.lock().await;
        actor.command = command;
        actor.cancellation_token = CancellationToken::new();
        
        // Execute command in background
        let sender_clone = self.actor_sender.clone();
        let result = actor.execute_command(&sender_clone).await;
        
        result.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
    
    async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let actor = self.command_actor.lock().await;
        actor.cancellation_token.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Actor;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_command_actor_echo_single_line() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create StringCommandActor with simple echo command
        let handler = StringCommandActor::new_string("echo 'Hello World'");
        let actor: Actor<String, StringCommandActor> = Actor::new(actor_rx, tx, handler);

        // Send trigger message
        let trigger_message = Message::new("execute", "".to_string());
        actor_tx.send(trigger_message).unwrap();

        // Wait for output message
        let output_msg = rx.recv().await.unwrap();
        assert_eq!(output_msg.method, "output");
        assert_eq!(output_msg.payload, "Hello World");

        // In the new design, no completion message is sent
        // The command execution is tracked through logging

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_command_actor_echo_multiple_lines() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create StringCommandActor with multi-line echo command (using printf for cross-platform compatibility)
        let handler = StringCommandActor::new_string("printf 'Hello\\nWorld\\nFrom\\nCommand\\n'");
        let actor: Actor<String, StringCommandActor> = Actor::new(actor_rx, tx, handler);

        // Send trigger message
        let trigger_message = Message::new("run", "test".to_string());
        actor_tx.send(trigger_message).unwrap();

        // Collect output messages
        let mut output_lines = Vec::new();

        // Read output messages (expect 4 lines, no completion in new design)
        for _ in 0..4 {
            let msg = rx.recv().await.unwrap();
            if msg.method == "output" {
                output_lines.push(msg.payload);
            }
        }

        // Verify we got all expected lines
        assert_eq!(output_lines.len(), 4);
        assert_eq!(output_lines[0], "Hello");
        assert_eq!(output_lines[1], "World");
        assert_eq!(output_lines[2], "From");
        assert_eq!(output_lines[3], "Command");

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_command_actor_ls_command() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create StringCommandActor with ls command
        let handler = StringCommandActor::new_string("ls -la /tmp | head -3");
        let actor: Actor<String, StringCommandActor> = Actor::new(actor_rx, tx, handler);

        // Send trigger message
        let trigger_message = Message::new("list", "".to_string());
        actor_tx.send(trigger_message).unwrap();

        // Wait for at least one output message
        let first_msg = rx.recv().await.unwrap();
        assert_eq!(first_msg.method, "output");
        assert!(!first_msg.payload.is_empty());

        // In the new design, no completion message is sent
        // Just wait a bit to ensure command finishes
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_command_actor_failing_command() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create StringCommandActor with a command that should fail
        let handler = StringCommandActor::new_string("false"); // 'false' command always exits with code 1
        let actor: Actor<String, StringCommandActor> = Actor::new(actor_rx, tx, handler);

        // Send trigger message
        let trigger_message = Message::new("fail", "".to_string());
        actor_tx.send(trigger_message).unwrap();

        // In the new design, no completion message is sent
        // Error handling is done through logging
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean up
        drop(actor);
    }

    // Tests for GenericCommandActor and CommandHandler trait

    /// Custom handler that converts lines to uppercase
    #[derive(Debug, Clone)]
    struct UppercaseHandler;

    #[async_trait]
    impl CommandHandler<String> for UppercaseHandler {
        async fn process_line(&self, line: String, sender: &ActorSender<String>) {
            let uppercase = line.to_uppercase();
            let _ = sender.send_message("output", uppercase).await;
        }
    }

    /// Custom handler that parses numbers and ignores non-numeric lines
    #[derive(Debug, Clone)]
    struct NumberHandler;

    #[async_trait]
    impl CommandHandler<i32> for NumberHandler {
        async fn process_line(&self, line: String, sender: &ActorSender<i32>) {
            // Only send valid numbers, ignore non-numeric lines
            if let Ok(number) = line.trim().parse::<i32>() {
                let _ = sender.send_message("output", number).await;
            }
            // Invalid lines are simply ignored (no message sent)
        }
    }

    #[tokio::test]
    async fn test_generic_command_actor_uppercase_handler() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create GenericCommandActor with uppercase handler
        let handler = UppercaseHandler;
        let mut actor_handler = CommandActor::new("printf 'hello\\nworld\\n'", handler);
        let actor: Actor<String, CommandActor<String, UppercaseHandler>> =
            Actor::new(actor_rx, tx, actor_handler);

        // Send trigger message
        let trigger_message = Message::new("execute", "".to_string());
        actor_tx.send(trigger_message).unwrap();

        // Collect output messages
        let mut output_lines = Vec::new();

        // Read output messages (expect 2 lines, no completion message in new design)
        for _ in 0..2 {
            let msg = rx.recv().await.unwrap();
            if msg.method == "output" {
                output_lines.push(msg.payload);
            }
        }

        // Verify we got all expected lines in uppercase
        assert_eq!(output_lines.len(), 2);
        assert_eq!(output_lines[0], "HELLO");
        assert_eq!(output_lines[1], "WORLD");

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_generic_command_actor_number_handler() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create GenericCommandActor with number handler
        let handler = NumberHandler;
        let mut actor_handler = CommandActor::new("printf '42\\nnot-a-number\\n123\\n'", handler);
        let actor: Actor<i32, CommandActor<i32, NumberHandler>> =
            Actor::new(actor_rx, tx, actor_handler);

        // Send trigger message
        let trigger_message = Message::new("execute", 0);
        actor_tx.send(trigger_message).unwrap();

        // Collect output messages
        let mut output_numbers = Vec::new();

        // Read output messages (expect 2 valid numbers, no completion message in new design)
        for _ in 0..2 {
            let msg = rx.recv().await.unwrap();
            if msg.method == "output" {
                output_numbers.push(msg.payload);
            }
        }

        // Verify we got the valid numbers only (non-numeric line ignored)
        assert_eq!(output_numbers.len(), 2);
        assert_eq!(output_numbers[0], 42);
        assert_eq!(output_numbers[1], 123);

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_generic_command_actor_cancellation_token_reset() {
        // Note: This test verifies that the cancellation token is properly reset
        // between executions. In the new design, no system messages are sent.
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create GenericCommandActor with simple command
        let handler = StringHandler;
        let actor_handler = CommandActor::new("echo 'test'", handler);
        let actor: Actor<String, CommandActor<String, StringHandler>> =
            Actor::new(actor_rx, tx, actor_handler);

        // Send cancel message first (should do nothing since no command is running)
        let cancel_message = Message::new("cancel", "".to_string());
        actor_tx.send(cancel_message).unwrap();

        // Send execute message - should work normally despite previous cancel
        let execute_message = Message::new("execute", "".to_string());
        actor_tx.send(execute_message).unwrap();

        // Should receive normal output (no completion message in new design)
        let output_msg = rx.recv().await.unwrap();
        assert_eq!(output_msg.method, "output");
        assert_eq!(output_msg.payload, "test");

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_generic_command_actor_multiple_executions() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        // Create GenericCommandActor with simple echo
        let handler = StringHandler;
        let mut actor_handler = CommandActor::new("echo 'test'", handler);
        let actor: Actor<String, CommandActor<String, StringHandler>> =
            Actor::new(actor_rx, tx, actor_handler);

        // Send first execute message
        let execute1 = Message::new("execute", "".to_string());
        actor_tx.send(execute1).unwrap();

        // Wait for first output (no completion message in new design)
        let output1 = rx.recv().await.unwrap();
        assert_eq!(output1.method, "output");
        assert_eq!(output1.payload, "test");

        // Send second execute message
        let execute2 = Message::new("execute", "".to_string());
        actor_tx.send(execute2).unwrap();

        // Wait for second output
        let output2 = rx.recv().await.unwrap();
        assert_eq!(output2.method, "output");
        assert_eq!(output2.payload, "test");

        // Clean up
        drop(actor);
    }
}
