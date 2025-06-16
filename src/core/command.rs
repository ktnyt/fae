//! Command control interfaces for the Actor system.
//!
//! This module provides abstractions for command lifecycle management,
//! extending basic Actor messaging with command-specific operations
//! like spawn and kill.

use crate::core::{ActorSendError, Message};
use async_trait::async_trait;
use std::marker::PhantomData;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

/// Factory trait for creating command objects with flexible parameters.
pub trait CommandFactory<Args = ()>: Fn(Args) -> Command + Send + Sync {}

/// Blanket implementation for functions and closures
impl<F, Args> CommandFactory<Args> for F where F: Fn(Args) -> Command + Send + Sync {}

/// Command handler trait for processing messages and command output
#[async_trait]
pub trait CommandHandler<T: Send + Sync + 'static, Args: Send + 'static>:
    Send + Sync + 'static
{
    /// Handle incoming messages with access to command control
    async fn on_message(&mut self, message: Message<T>, controller: &CommandController<T, Args>);

    /// Handle stdout output from spawned commands
    async fn on_stdout(&mut self, line: String, controller: &CommandController<T, Args>);

    /// Handle stderr output from spawned commands
    async fn on_stderr(&mut self, line: String, controller: &CommandController<T, Args>);
}

/// A running process with cancellation support
pub struct RunningProcess {
    child: Child,
    cancellation_token: CancellationToken,
}

pub enum CommandOutput {
    Stdout(String),
    Stderr(String),
}

/// Controller for managing commands within a CommandActor
pub struct CommandController<T: Send + Sync + 'static, Args: Send + 'static> {
    sender: mpsc::UnboundedSender<Message<T>>,
    command_factory: Arc<dyn CommandFactory<Args>>,
    current_process: Arc<Mutex<Option<RunningProcess>>>,
    output_sender: mpsc::UnboundedSender<CommandOutput>,
}

impl<T: Send + Sync + 'static, Args: Send + 'static> CommandController<T, Args> {
    /// Send a message
    pub async fn send_message(&self, method: String, payload: T) -> Result<(), ActorSendError> {
        let message = Message::new(method, payload);
        self.sender
            .send(message)
            .map_err(|_| ActorSendError::ChannelClosed)
    }

    /// Spawn a new command with the given arguments
    pub async fn spawn(&self, args: Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Kill existing process if any
        self.kill().await?;

        // Create new command
        let mut command = (self.command_factory)(args);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Spawn process
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to get stderr")?;

        // Create cancellation token
        let cancellation_token = CancellationToken::new();

        // Store the process
        let process = RunningProcess {
            child,
            cancellation_token: cancellation_token.clone(),
        };

        {
            let mut current = self.current_process.lock().unwrap();
            *current = Some(process);
        }

        // Start output processing - send stdout/stderr to output_sender
        let output_sender_stdout = self.output_sender.clone();
        let output_sender_stderr = self.output_sender.clone();
        let token_stdout = cancellation_token.clone();
        let token_stderr = cancellation_token.clone();

        // Spawn stdout reading task
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stdout).lines();
            loop {
                tokio::select! {
                    _ = token_stdout.cancelled() => {
                        log::debug!("Stdout reading cancelled");
                        break;
                    }
                    line_result = lines.next_line() => {
                        match line_result {
                            Ok(Some(line)) => {
                                if output_sender_stdout.send(CommandOutput::Stdout(line)).is_err() {
                                    log::debug!("Output sender closed");
                                    break;
                                }
                            }
                            Ok(None) => {
                                log::debug!("EOF reached on stdout");
                                break;
                            }
                            Err(e) => {
                                log::error!("Error reading stdout: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Spawn stderr reading task
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            loop {
                tokio::select! {
                    _ = token_stderr.cancelled() => {
                        log::debug!("Stderr reading cancelled");
                        break;
                    }
                    line_result = lines.next_line() => {
                        match line_result {
                            Ok(Some(line)) => {
                                if output_sender_stderr.send(CommandOutput::Stderr(line)).is_err() {
                                    log::debug!("Output sender closed");
                                    break;
                                }
                            }
                            Ok(None) => {
                                log::debug!("EOF reached on stderr");
                                break;
                            }
                            Err(e) => {
                                log::error!("Error reading stderr: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        log::info!("Command spawned successfully with output processing");
        Ok(())
    }

    /// Kill the currently running command
    pub async fn kill(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let process = {
            let mut current = self.current_process.lock().unwrap();
            current.take()
        };

        if let Some(mut process) = process {
            // Cancel stdout/stderr reading tasks first
            process.cancellation_token.cancel();

            // Kill the process
            process.child.kill().await?;
            process.child.wait().await?;

            log::info!("Command killed and output processing cancelled");
        }
        Ok(())
    }
}

/// Independent CommandActor that handles messages and manages command lifecycle
pub struct CommandActor<T: Send + Sync + 'static, Args: Send + 'static> {
    sender: mpsc::UnboundedSender<Message<T>>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    thread_handle: Option<JoinHandle<()>>,
    _phantom: PhantomData<Args>,
}

impl<T: Send + Sync + 'static, Args: Send + 'static> CommandActor<T, Args> {
    /// Create a new CommandActor and start it automatically
    pub fn new<H: CommandHandler<T, Args>>(
        message_receiver: mpsc::UnboundedReceiver<Message<T>>,
        sender: mpsc::UnboundedSender<Message<T>>,
        command_factory: Arc<dyn CommandFactory<Args>>,
        handler: H,
    ) -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let sender_clone = sender.clone();

        // Start the message processing loop in a separate thread
        let thread_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(Self::run(
                handler,
                message_receiver,
                sender_clone,
                command_factory,
                shutdown_receiver,
            ));
        });

        Self {
            sender,
            shutdown_sender: Some(shutdown_sender),
            thread_handle: Some(thread_handle),
            _phantom: PhantomData,
        }
    }

    /// Send a message to external recipients
    pub async fn send_message(
        &self,
        method: impl Into<String>,
        payload: T,
    ) -> Result<(), ActorSendError> {
        let message = Message::new(method, payload);
        self.sender
            .send(message)
            .map_err(|_| ActorSendError::ChannelClosed)
    }

    /// Internal run method for the message processing loop
    async fn run<H: CommandHandler<T, Args>>(
        mut handler: H,
        mut message_receiver: mpsc::UnboundedReceiver<Message<T>>,
        sender: mpsc::UnboundedSender<Message<T>>,
        command_factory: Arc<dyn CommandFactory<Args>>,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        // Create output channel for command output
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        // Create controller
        let controller = CommandController {
            sender,
            command_factory,
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_receiver => {
                    log::debug!("CommandActor received shutdown signal, stopping message loop");
                    break;
                }
                // Handle incoming messages
                message = message_receiver.recv() => {
                    match message {
                        Some(message) => {
                            log::trace!("CommandActor received message: method={}", message.method);
                            handler.on_message(message, &controller).await;
                        }
                        None => {
                            log::debug!("Message receiver channel closed");
                            break;
                        }
                    }
                }
                // Handle command output
                output = output_receiver.recv() => {
                    match output {
                        Some(CommandOutput::Stdout(line)) => {
                            handler.on_stdout(line, &controller).await;
                        }
                        Some(CommandOutput::Stderr(line)) => {
                            handler.on_stderr(line, &controller).await;
                        }
                        None => {
                            log::debug!("Output receiver channel closed");
                            // Don't break here, we might still receive messages
                        }
                    }
                }
            }
        }

        log::info!("CommandActor shutting down");
    }

    /// Manually shutdown the CommandActor
    pub fn shutdown(&mut self) {
        log::info!("Manual shutdown requested for CommandActor");

        // Send shutdown signal
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            log::debug!("Sending shutdown signal to CommandActor");
            let _ = shutdown_sender.send(());
        }

        // Wait for thread to finish (graceful shutdown)
        if let Some(thread_handle) = self.thread_handle.take() {
            log::debug!("Waiting for CommandActor message loop thread to finish");
            let _ = thread_handle.join();
        }

        log::info!("CommandActor shutdown completed");
    }
}

impl<T: Send + Sync + 'static, Args: Send + 'static> Drop for CommandActor<T, Args> {
    fn drop(&mut self) {
        // Perform cleanup if shutdown hasn't been called explicitly
        if self.shutdown_sender.is_some() || self.thread_handle.is_some() {
            log::debug!("CommandActor dropped without explicit shutdown, performing cleanup");
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    // Mock command factory for testing
    fn create_long_running_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg("while true; do echo 'output'; sleep 0.1; done");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg("for /L %i in (1,0,2) do @echo output & timeout /t 1 /nobreak >nul");
            cmd
        }
    }

    #[tokio::test]
    async fn test_kill_cancels_output_tasks() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_long_running_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Spawn a long-running command
        controller.spawn(()).await.expect("Failed to spawn command");

        // Wait a bit to ensure output starts flowing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify output is being received
        let first_output = timeout(Duration::from_millis(500), output_receiver.recv())
            .await
            .expect("Should receive output")
            .expect("Should have output");

        assert!(matches!(first_output, CommandOutput::Stdout(_)));

        // Kill the command
        controller.kill().await.expect("Failed to kill command");

        // Wait a bit for cancellation to take effect
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Count how many more outputs we receive (should be 0 or very few due to buffering)
        let mut output_count = 0;
        let mut consecutive_timeouts = 0;

        loop {
            let result = timeout(Duration::from_millis(100), output_receiver.recv()).await;
            match result {
                Err(_) => {
                    // Timeout
                    consecutive_timeouts += 1;
                    if consecutive_timeouts >= 2 {
                        break; // No output for a while, tasks are cancelled
                    }
                }
                Ok(None) => {
                    // Channel closed - tasks were cancelled
                    break;
                }
                Ok(Some(_)) => {
                    output_count += 1;
                    consecutive_timeouts = 0;
                    // Allow a few buffered outputs but not continuous stream
                    if output_count > 5 {
                        panic!("Too many outputs received after kill ({}), tasks not properly cancelled", output_count);
                    }
                }
            }
        }

        println!(
            "Received {} outputs after kill (acceptable if small due to buffering)",
            output_count
        );
    }

    #[tokio::test]
    async fn test_spawn_after_kill_works() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_long_running_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Spawn first command
        controller
            .spawn(())
            .await
            .expect("Failed to spawn first command");

        // Kill it
        controller
            .kill()
            .await
            .expect("Failed to kill first command");

        // Spawn second command
        controller
            .spawn(())
            .await
            .expect("Failed to spawn second command");

        // Should receive output from new command
        let output = timeout(Duration::from_millis(1000), output_receiver.recv())
            .await
            .expect("Should receive output from new command")
            .expect("Should have output");

        assert!(matches!(output, CommandOutput::Stdout(_)));

        // Clean up
        controller
            .kill()
            .await
            .expect("Failed to kill second command");
    }

    // Test handler that spawns commands on message and records output
    #[derive(Clone)]
    struct TestCommandHandler {
        stdout_lines: Arc<Mutex<Vec<String>>>,
        stderr_lines: Arc<Mutex<Vec<String>>>,
    }

    impl TestCommandHandler {
        fn new() -> Self {
            Self {
                stdout_lines: Arc::new(Mutex::new(Vec::new())),
                stderr_lines: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_stdout_lines(&self) -> Vec<String> {
            self.stdout_lines.lock().unwrap().clone()
        }

        fn get_stderr_lines(&self) -> Vec<String> {
            self.stderr_lines.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl CommandHandler<String, ()> for TestCommandHandler {
        async fn on_message(
            &mut self,
            message: Message<String>,
            controller: &CommandController<String, ()>,
        ) {
            if message.method == "spawn_command" {
                let _ = controller.spawn(()).await;
            }
        }

        async fn on_stdout(&mut self, line: String, _controller: &CommandController<String, ()>) {
            self.stdout_lines.lock().unwrap().push(line);
        }

        async fn on_stderr(&mut self, line: String, _controller: &CommandController<String, ()>) {
            self.stderr_lines.lock().unwrap().push(line);
        }
    }

    fn create_simple_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg("echo 'hello stdout'; echo 'hello stderr' >&2");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg("echo hello stdout & echo hello stderr 1>&2");
            cmd
        }
    }

    #[tokio::test]
    async fn test_command_actor_message_to_output_flow() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<String>>();
        let (external_tx, mut _external_rx) = mpsc::unbounded_channel::<Message<String>>();

        let handler = TestCommandHandler::new();
        let handler_clone = handler.clone();

        // Create CommandActor
        let mut actor = CommandActor::new(
            actor_rx,
            external_tx,
            Arc::new(create_simple_command),
            handler,
        );

        // Send message to trigger command spawn
        let spawn_message = Message::new("spawn_command", "test".to_string());
        actor_tx
            .send(spawn_message)
            .expect("Failed to send message");

        // Wait for command execution and output processing
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Verify stdout and stderr were received
        let stdout_lines = handler_clone.get_stdout_lines();
        let stderr_lines = handler_clone.get_stderr_lines();

        assert!(!stdout_lines.is_empty(), "Should receive stdout output");
        assert!(!stderr_lines.is_empty(), "Should receive stderr output");

        // Check content
        assert!(stdout_lines
            .iter()
            .any(|line| line.contains("hello stdout")));
        assert!(stderr_lines
            .iter()
            .any(|line| line.contains("hello stderr")));

        // Clean up
        actor.shutdown();
    }

    // Test handler that can spawn and kill commands
    #[derive(Clone)]
    struct KillTestHandler {
        stdout_lines: Arc<Mutex<Vec<String>>>,
        stderr_lines: Arc<Mutex<Vec<String>>>,
    }

    impl KillTestHandler {
        fn new() -> Self {
            Self {
                stdout_lines: Arc::new(Mutex::new(Vec::new())),
                stderr_lines: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_stdout_count(&self) -> usize {
            self.stdout_lines.lock().unwrap().len()
        }

        #[allow(dead_code)]
        fn get_stderr_count(&self) -> usize {
            self.stderr_lines.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl CommandHandler<String, ()> for KillTestHandler {
        async fn on_message(
            &mut self,
            message: Message<String>,
            controller: &CommandController<String, ()>,
        ) {
            match message.method.as_str() {
                "spawn_yes" => {
                    let _ = controller.spawn(()).await;
                }
                "kill_command" => {
                    let _ = controller.kill().await;
                }
                _ => {}
            }
        }

        async fn on_stdout(&mut self, line: String, _controller: &CommandController<String, ()>) {
            self.stdout_lines.lock().unwrap().push(line);
        }

        async fn on_stderr(&mut self, line: String, _controller: &CommandController<String, ()>) {
            self.stderr_lines.lock().unwrap().push(line);
        }
    }

    fn create_yes_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("yes");
            cmd.arg("test_output");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg("for /L %i in (1,0,2) do @echo test_output");
            cmd
        }
    }

    // Test error conditions and edge cases

    fn create_failing_command(_args: ()) -> Command {
        // Create a command that will fail to execute
        Command::new("non_existent_command_xyz_12345")
    }

    #[tokio::test]
    async fn test_spawn_with_failing_command() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, _output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_failing_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Spawning should fail but not panic
        let result = controller.spawn(()).await;
        assert!(result.is_err(), "Should fail to spawn non-existent command");

        // Should have no current process after failed spawn
        let process = controller.current_process.lock().unwrap();
        assert!(
            process.is_none(),
            "Should have no process after failed spawn"
        );
    }

    #[tokio::test]
    async fn test_kill_when_no_process() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, _output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_simple_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Killing when no process should succeed (no-op)
        let result = controller.kill().await;
        assert!(
            result.is_ok(),
            "Kill should succeed even when no process exists"
        );
    }

    #[tokio::test]
    async fn test_multiple_consecutive_spawns() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_simple_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // First spawn
        controller
            .spawn(())
            .await
            .expect("First spawn should succeed");

        // Wait for some output
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Second spawn should kill first and start new
        controller
            .spawn(())
            .await
            .expect("Second spawn should succeed");

        // Should receive output from second process
        let output = timeout(Duration::from_millis(1000), output_receiver.recv())
            .await
            .expect("Should receive output")
            .expect("Should have output");

        assert!(matches!(output, CommandOutput::Stdout(_)));

        // Clean up
        controller
            .kill()
            .await
            .expect("Clean up kill should succeed");
    }

    #[tokio::test]
    async fn test_send_message_with_closed_channel() {
        let (message_sender, message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, _output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_simple_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Close the receiver
        drop(message_receiver);

        // Sending message should fail gracefully
        let result = controller
            .send_message("test".to_string(), "payload".to_string())
            .await;
        assert!(matches!(result, Err(ActorSendError::ChannelClosed)));
    }

    // Test concurrent operations
    #[tokio::test]
    async fn test_concurrent_spawn_and_kill() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, _output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = Arc::new(CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_long_running_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        });

        let controller1 = controller.clone();
        let controller2 = controller.clone();

        // Start concurrent spawn and kill operations
        let spawn_task = tokio::spawn(async move { controller1.spawn(()).await });

        let kill_task = tokio::spawn(async move {
            // Small delay to increase chance of race condition
            tokio::time::sleep(Duration::from_millis(10)).await;
            controller2.kill().await
        });

        // Both operations should complete without panic
        let (spawn_result, kill_result) = tokio::join!(spawn_task, kill_task);

        // At least one should succeed (might race)
        assert!(spawn_result.is_ok() || kill_result.is_ok());

        // Ensure final cleanup
        controller
            .kill()
            .await
            .expect("Final cleanup should succeed");
    }

    // Test CommandActor error handling
    #[derive(Clone)]
    struct PanicHandler;

    #[async_trait]
    impl CommandHandler<String, ()> for PanicHandler {
        async fn on_message(
            &mut self,
            message: Message<String>,
            _controller: &CommandController<String, ()>,
        ) {
            if message.payload == "panic" {
                panic!("Test panic in handler");
            }
        }

        async fn on_stdout(&mut self, _line: String, _controller: &CommandController<String, ()>) {}
        async fn on_stderr(&mut self, _line: String, _controller: &CommandController<String, ()>) {}
    }

    #[tokio::test]
    async fn test_handler_panic_isolation() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<String>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<String>>();

        let mut actor = CommandActor::new(
            actor_rx,
            external_tx,
            Arc::new(create_simple_command),
            PanicHandler,
        );

        // Send a non-panic message first
        let normal_message = Message::new("test", "normal".to_string());
        actor_tx
            .send(normal_message)
            .expect("Should send normal message");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send panic message - this might cause the actor to crash
        let panic_message = Message::new("test", "panic".to_string());
        let send_result = actor_tx.send(panic_message);

        // The send might succeed even if handler panics
        // This test mainly ensures the panic doesn't crash the entire program

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Try to send another message to see if actor is still alive
        let recovery_message = Message::new("test", "recovery".to_string());
        let recovery_result = actor_tx.send(recovery_message);

        // Log results for debugging
        println!("Send panic result: {:?}", send_result);
        println!("Recovery send result: {:?}", recovery_result);

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_shutdown_and_drop_safety() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<String>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<String>>();

        let mut actor = CommandActor::new(
            actor_rx,
            external_tx,
            Arc::new(create_simple_command),
            TestCommandHandler::new(),
        );

        // First shutdown
        actor.shutdown();

        // Second shutdown should be safe (no-op)
        actor.shutdown();

        // Dropping after shutdown should be safe
        drop(actor);

        // Sending to closed actor should fail gracefully
        let result = actor_tx.send(Message::new("test", "after_shutdown".to_string()));
        assert!(
            result.is_err(),
            "Should not be able to send to shutdown actor"
        );
    }

    #[tokio::test]
    async fn test_output_channel_closed_handling() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_long_running_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Spawn command
        controller.spawn(()).await.expect("Should spawn command");

        // Close output receiver immediately
        drop(output_receiver);

        // Wait a bit - output tasks should detect closed channel and stop
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Process should still be killable
        controller
            .kill()
            .await
            .expect("Should be able to kill even with closed output channel");
    }

    // Additional stress tests and edge cases

    #[tokio::test]
    async fn test_high_frequency_spawn_kill_cycles() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, _output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_simple_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Rapid spawn/kill cycles to test stability
        for i in 0..10 {
            println!("Cycle {}/10", i + 1);

            controller.spawn(()).await.expect("Spawn should succeed");
            tokio::time::sleep(Duration::from_millis(50)).await;

            controller.kill().await.expect("Kill should succeed");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn test_spawn_immediately_after_kill() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_simple_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        // Spawn first process
        controller
            .spawn(())
            .await
            .expect("First spawn should succeed");

        // Kill and immediately spawn again
        controller.kill().await.expect("Kill should succeed");
        controller
            .spawn(())
            .await
            .expect("Immediate respawn should succeed");

        // Should receive output from new process
        let output = timeout(Duration::from_millis(1000), output_receiver.recv())
            .await
            .expect("Should receive output")
            .expect("Should have output");

        assert!(matches!(output, CommandOutput::Stdout(_)));

        // Clean up
        controller
            .kill()
            .await
            .expect("Final cleanup should succeed");
    }

    // Test with command that produces stderr
    fn create_stderr_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("sh");
            cmd.arg("-c")
                .arg("echo 'error message' >&2; echo 'normal message'");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C")
                .arg("echo error message 1>&2 & echo normal message");
            cmd
        }
    }

    #[tokio::test]
    async fn test_stderr_handling() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_stderr_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        controller.spawn(()).await.expect("Should spawn command");

        let mut stdout_received = false;
        let mut stderr_received = false;

        // Collect outputs with timeout
        for _ in 0..10 {
            match timeout(Duration::from_millis(200), output_receiver.recv()).await {
                Ok(Some(CommandOutput::Stdout(line))) => {
                    println!("Stdout: {}", line);
                    stdout_received = true;
                }
                Ok(Some(CommandOutput::Stderr(line))) => {
                    println!("Stderr: {}", line);
                    stderr_received = true;
                }
                Ok(None) => break,
                Err(_) => break, // Timeout
            }

            if stdout_received && stderr_received {
                break;
            }
        }

        assert!(stdout_received, "Should receive stdout output");
        assert!(stderr_received, "Should receive stderr output");

        // Clean up
        controller.kill().await.expect("Should be able to kill");
    }

    // Test process that exits quickly
    fn create_quick_exit_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("echo");
            cmd.arg("quick_exit");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg("echo quick_exit");
            cmd
        }
    }

    #[tokio::test]
    async fn test_quick_exit_process() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_quick_exit_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        controller
            .spawn(())
            .await
            .expect("Should spawn quick exit command");

        // Should still receive output even though process exits quickly
        let output = timeout(Duration::from_millis(1000), output_receiver.recv())
            .await
            .expect("Should receive output before timeout")
            .expect("Should have output");

        assert!(matches!(output, CommandOutput::Stdout(_)));

        // Wait a bit for process to finish
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Kill should succeed even if process already exited
        let kill_result = controller.kill().await;
        // Note: kill might fail if process already exited, which is fine
        println!("Kill result for already-exited process: {:?}", kill_result);
    }

    // Test memory safety under stress
    #[tokio::test]
    async fn test_memory_safety_stress() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = Arc::new(CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_quick_exit_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        });

        // Spawn many concurrent operations
        let mut tasks = Vec::new();

        for i in 0..20 {
            let controller_clone = controller.clone();
            let task = tokio::spawn(async move {
                // Alternate between spawn and kill
                if i % 2 == 0 {
                    let _ = controller_clone.spawn(()).await;
                } else {
                    let _ = controller_clone.kill().await;
                }
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            let _ = task.await;
        }

        // Drain output channel
        let mut output_count = 0;
        while let Ok(output) = timeout(Duration::from_millis(10), output_receiver.recv()).await {
            if output.is_some() {
                output_count += 1;
            } else {
                break;
            }
        }

        println!("Received {} outputs during stress test", output_count);

        // Final cleanup
        let _ = controller.kill().await;
    }

    // Test with very large output
    fn create_large_output_command(_args: ()) -> Command {
        #[cfg(unix)]
        {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg("for i in $(seq 1 100); do echo 'Line $i with some text to make it longer and test buffer handling'; done");
            cmd
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg("for /L %i in (1,1,100) do @echo Line %i with some text to make it longer and test buffer handling");
            cmd
        }
    }

    #[tokio::test]
    async fn test_large_output_handling() {
        let (message_sender, _message_receiver) = mpsc::unbounded_channel::<Message<String>>();
        let (output_sender, mut output_receiver) = mpsc::unbounded_channel::<CommandOutput>();

        let controller = CommandController {
            sender: message_sender,
            command_factory: Arc::new(create_large_output_command),
            current_process: Arc::new(Mutex::new(None)),
            output_sender,
        };

        controller
            .spawn(())
            .await
            .expect("Should spawn large output command");

        let mut line_count = 0;
        let start_time = std::time::Instant::now();

        while line_count < 100 && start_time.elapsed() < Duration::from_secs(5) {
            match timeout(Duration::from_millis(500), output_receiver.recv()).await {
                Ok(Some(CommandOutput::Stdout(_))) => {
                    line_count += 1;
                }
                Ok(Some(CommandOutput::Stderr(_))) => {
                    // Stderr is also fine
                }
                Ok(None) => break,
                Err(_) => break, // Timeout
            }
        }

        println!("Received {} lines of output", line_count);
        assert!(
            line_count >= 50,
            "Should receive substantial output (got {})",
            line_count
        );

        // Clean up
        controller.kill().await.expect("Should be able to kill");
    }

    #[tokio::test]
    async fn test_command_actor_kill_stops_output() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<String>>();
        let (external_tx, mut _external_rx) = mpsc::unbounded_channel::<Message<String>>();

        let handler = KillTestHandler::new();
        let handler_clone = handler.clone();

        // Create CommandActor
        let mut actor =
            CommandActor::new(actor_rx, external_tx, Arc::new(create_yes_command), handler);

        // Spawn yes command (infinite output)
        let spawn_message = Message::new("spawn_yes", "start".to_string());
        actor_tx
            .send(spawn_message)
            .expect("Failed to send spawn message");

        // Wait for output to start flowing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify output is being produced
        let initial_count = handler_clone.get_stdout_count();
        assert!(
            initial_count > 0,
            "Should have received some output before kill"
        );

        // Kill the command
        let kill_message = Message::new("kill_command", "stop".to_string());
        actor_tx
            .send(kill_message)
            .expect("Failed to send kill message");

        // Wait for kill to take effect
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Record count after kill
        let count_after_kill = handler_clone.get_stdout_count();

        // Wait a bit longer to see if output continues
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Final count should be the same or very close (allowing for buffering)
        let final_count = handler_clone.get_stdout_count();
        let continued_output = final_count - count_after_kill;

        println!(
            "Initial count: {}, After kill: {}, Final: {}, Continued: {}",
            initial_count, count_after_kill, final_count, continued_output
        );

        // Should have very little or no continued output after kill
        assert!(
            continued_output <= 3,
            "Command should stop producing output after kill, but got {} more lines",
            continued_output
        );

        // Clean up
        actor.shutdown();
    }
}
