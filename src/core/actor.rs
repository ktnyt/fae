//! Actor implementation for notification-based communication
//!
//! This module contains the core Actor struct and related types for handling
//! bidirectional notification messages in a lightweight actor system.

use crate::core::message::{Message, MessageHandler};
use std::marker::PhantomData;
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};

/// A lightweight actor that handles bidirectional notification messages.
///
/// The Actor manages a message loop that receives messages from external sources
/// and processes them using a provided MessageHandler. It follows the same
/// self-managed lifecycle pattern as JsonRpcEngine with automatic startup
/// and graceful shutdown.
pub struct Actor<T: Send + Sync + 'static, H: MessageHandler<T> + Send + Sync + 'static> {
    /// Channel for sending messages to external recipients
    sender: mpsc::UnboundedSender<Message<T>>,
    /// Controller for external use
    controller: ActorController<T>,
    /// Channel for sending shutdown signal
    shutdown_sender: Option<oneshot::Sender<()>>,
    /// Handle to the message processing thread
    thread_handle: Option<JoinHandle<()>>,
    /// Phantom data to hold the handler and message types
    _phantom: PhantomData<(T, H)>,
}

impl<T: Send + Sync + 'static, H: MessageHandler<T> + Send + Sync + 'static> Actor<T, H> {
    /// Create a new Actor with the specified channels and handler.
    ///
    /// # Arguments
    /// * `receiver` - Channel for receiving external messages
    /// * `sender` - Channel for sending messages to external recipients
    /// * `handler` - Message handler implementation
    pub fn new(
        receiver: mpsc::UnboundedReceiver<Message<T>>,
        sender: mpsc::UnboundedSender<Message<T>>,
        handler: H,
    ) -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        let sender_clone = sender.clone();

        // Create controller
        let controller = ActorController::new(sender.clone());

        // Start the message processing loop in a separate thread
        let thread_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(Self::run_message_loop(
                receiver,
                sender_clone,
                handler,
                shutdown_receiver,
            ));
        });

        Self {
            sender,
            controller,
            shutdown_sender: Some(shutdown_sender),
            thread_handle: Some(thread_handle),
            _phantom: PhantomData,
        }
    }

    /// Send a message to an external recipient.
    pub async fn send(&self, message: Message<T>) -> Result<(), ActorSendError> {
        self.sender
            .send(message)
            .map_err(|_| ActorSendError::ChannelClosed)
    }

    /// Send a message to another actor or external system.
    pub async fn send_message(
        &self,
        method: impl Into<String>,
        payload: T,
    ) -> Result<(), ActorSendError> {
        let message = Message::new(method, payload);
        self.send(message).await
    }

    /// Get a reference to the sender channel.
    pub fn sender(&self) -> &mpsc::UnboundedSender<Message<T>> {
        &self.sender
    }

    /// Get a reference to the controller.
    pub fn controller(&self) -> &ActorController<T> {
        &self.controller
    }

    /// Main message processing loop.
    async fn run_message_loop(
        mut receiver: mpsc::UnboundedReceiver<Message<T>>,
        sender: mpsc::UnboundedSender<Message<T>>,
        mut handler: H,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        // Create a controller for the handler to use for sending messages
        let controller = ActorController::new(sender.clone());

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_receiver => {
                    log::debug!("Received shutdown signal, stopping message loop");
                    break;
                }
                // Handle messages
                message = receiver.recv() => {
                    match message {
                        Some(message) => {
                            log::trace!("Received message: method={}", message.method);
                            handler.on_message(message, &controller).await;
                        }
                        None => {
                            log::debug!("Receiver channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Manually shutdown the actor.
    /// This will be called automatically on Drop if not called explicitly.
    pub fn shutdown(&mut self) {
        log::info!("Manual shutdown requested for Actor");

        // Send shutdown signal
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            log::debug!("Sending shutdown signal");
            let _ = shutdown_sender.send(());
        }

        // Wait for thread to finish (graceful shutdown)
        if let Some(thread_handle) = self.thread_handle.take() {
            log::debug!("Waiting for message loop thread to finish");
            let _ = thread_handle.join();
        }

        log::info!("Actor shutdown completed");
    }
}

/// Error type for Actor message sending operations.
#[derive(Debug)]
pub enum ActorSendError {
    ChannelClosed,
}

impl std::fmt::Display for ActorSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorSendError::ChannelClosed => write!(f, "Actor channel is closed"),
        }
    }
}

impl std::error::Error for ActorSendError {}

/// Controller for sending messages in the Actor system.
/// This provides a concrete implementation for message sending operations.
pub struct ActorController<T> {
    sender: mpsc::UnboundedSender<Message<T>>,
}

impl<T: Send + Sync + 'static> ActorController<T> {
    /// Create a new ActorController with the given sender.
    pub fn new(sender: mpsc::UnboundedSender<Message<T>>) -> Self {
        Self { sender }
    }

    /// Send a message to external recipients.
    pub async fn send_message(&self, method: String, payload: T) -> Result<(), ActorSendError> {
        let message = Message::new(method, payload);
        self.sender
            .send(message)
            .map_err(|_| ActorSendError::ChannelClosed)
    }
}

impl<T> Clone for ActorController<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T: Send + Sync + 'static, H: MessageHandler<T> + Send + Sync + 'static> Drop for Actor<T, H> {
    fn drop(&mut self) {
        // Perform cleanup if shutdown hasn't been called explicitly
        if self.shutdown_sender.is_some() || self.thread_handle.is_some() {
            log::debug!("Actor dropped without explicit shutdown, performing cleanup");
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::message::{Message, MessageHandler};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    // Test structures for custom payload types
    #[derive(Debug, Clone, PartialEq)]
    struct UserData {
        id: u64,
        name: String,
        active: bool,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct TaskInfo {
        task_id: String,
        priority: u8,
        tags: Vec<String>,
        metadata: HashMap<String, String>,
    }

    // Test handler that records received string messages
    #[derive(Clone)]
    struct StringTestHandler {
        received_messages: Arc<Mutex<Vec<Message<String>>>>,
    }

    impl StringTestHandler {
        fn new() -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<Message<String>> {
            self.received_messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MessageHandler<String> for StringTestHandler {
        async fn on_message(
            &mut self,
            message: Message<String>,
            _controller: &crate::core::ActorController<String>,
        ) {
            let mut messages = self.received_messages.lock().unwrap();
            messages.push(message);
        }
    }

    // Test handler for integer messages
    #[derive(Clone)]
    struct IntTestHandler {
        received_messages: Arc<Mutex<Vec<Message<i32>>>>,
    }

    impl IntTestHandler {
        fn new() -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<Message<i32>> {
            self.received_messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MessageHandler<i32> for IntTestHandler {
        async fn on_message(
            &mut self,
            message: Message<i32>,
            _controller: &crate::core::ActorController<i32>,
        ) {
            let mut messages = self.received_messages.lock().unwrap();
            messages.push(message);
        }
    }

    // Test handler for UserData struct messages
    #[derive(Clone)]
    struct UserDataHandler {
        received_messages: Arc<Mutex<Vec<Message<UserData>>>>,
    }

    impl UserDataHandler {
        fn new() -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<Message<UserData>> {
            self.received_messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MessageHandler<UserData> for UserDataHandler {
        async fn on_message(
            &mut self,
            message: Message<UserData>,
            controller: &crate::core::ActorController<UserData>,
        ) {
            // Echo back a modified user (demonstrate struct manipulation)
            if message.method == "update_user" {
                let mut updated_user = message.payload.clone();
                updated_user.active = !updated_user.active; // Toggle active status
                let _ = controller
                    .send_message("user_updated".to_string(), updated_user)
                    .await;
            }

            // Push to messages after async operation
            let mut messages = self.received_messages.lock().unwrap();
            messages.push(message);
        }
    }

    // Test handler for TaskInfo struct messages
    #[derive(Clone)]
    struct TaskInfoHandler {
        received_messages: Arc<Mutex<Vec<Message<TaskInfo>>>>,
    }

    impl TaskInfoHandler {
        fn new() -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<Message<TaskInfo>> {
            self.received_messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MessageHandler<TaskInfo> for TaskInfoHandler {
        async fn on_message(
            &mut self,
            message: Message<TaskInfo>,
            controller: &crate::core::ActorController<TaskInfo>,
        ) {
            // Demonstrate complex struct processing
            if message.method == "process_task" {
                let mut processed_task = message.payload.clone();
                processed_task.priority = processed_task.priority.saturating_add(1);
                processed_task.tags.push("processed".to_string());
                processed_task
                    .metadata
                    .insert("status".to_string(), "completed".to_string());

                let _ = controller
                    .send_message("task_processed".to_string(), processed_task)
                    .await;
            }

            // Push to messages after async operation
            let mut messages = self.received_messages.lock().unwrap();
            messages.push(message);
        }
    }

    #[tokio::test]
    async fn test_string_actor_creation_and_shutdown() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = StringTestHandler::new();
        let actor: Actor<String, StringTestHandler> = Actor::new(actor_rx, tx, handler);

        // Give the actor a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Actor should clean up gracefully when dropped
        drop(actor);

        // Give cleanup time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_string_actor_message_handling() {
        let (tx, mut _rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = StringTestHandler::new();
        let handler_clone = handler.clone();
        let actor: Actor<String, StringTestHandler> = Actor::new(actor_rx, tx, handler);

        // Send a message to the actor
        let test_message = Message::new("test_method", "hello world".to_string());

        actor_tx.send(test_message.clone()).unwrap();

        // Give the actor time to process the message
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the handler received the message
        let received_messages = handler_clone.get_messages();
        assert_eq!(received_messages.len(), 1);
        assert_eq!(received_messages[0].method, test_message.method);
        assert_eq!(received_messages[0].payload, test_message.payload);

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_string_actor_send_message() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = StringTestHandler::new();
        let actor: Actor<String, StringTestHandler> = Actor::new(actor_rx, tx, handler);

        // Send a message through the actor's send_message method
        actor
            .send_message("ping", "test data".to_string())
            .await
            .unwrap();

        // Verify the message was sent to the external channel
        let received = rx.recv().await.unwrap();
        assert_eq!(received.method, "ping");
        assert_eq!(received.payload, "test data");

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_int_actor_message_handling() {
        let (tx, mut _rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = IntTestHandler::new();
        let handler_clone = handler.clone();
        let actor: Actor<i32, IntTestHandler> = Actor::new(actor_rx, tx, handler);

        // Send a message to the actor
        let test_message = Message::new("count", 42);

        actor_tx.send(test_message.clone()).unwrap();

        // Give the actor time to process the message
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the handler received the message
        let received_messages = handler_clone.get_messages();
        assert_eq!(received_messages.len(), 1);
        assert_eq!(received_messages[0].method, test_message.method);
        assert_eq!(received_messages[0].payload, test_message.payload);

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_userdata_struct_actor() {
        let (tx, mut _rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = UserDataHandler::new();
        let handler_clone = handler.clone();
        let actor: Actor<UserData, UserDataHandler> = Actor::new(actor_rx, tx, handler);

        // Create test user data
        let user_data = UserData {
            id: 123,
            name: "Alice".to_string(),
            active: true,
        };

        // Send a message with struct payload
        let test_message = Message::new("create_user", user_data.clone());
        actor_tx.send(test_message.clone()).unwrap();

        // Give the actor time to process the message
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the handler received the message
        let received_messages = handler_clone.get_messages();
        assert_eq!(received_messages.len(), 1);
        assert_eq!(received_messages[0].method, "create_user");
        assert_eq!(received_messages[0].payload, user_data);

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_userdata_struct_echo_behavior() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = UserDataHandler::new();
        let handler_clone = handler.clone();
        let actor: Actor<UserData, UserDataHandler> = Actor::new(actor_rx, tx, handler);

        // Create test user data
        let user_data = UserData {
            id: 456,
            name: "Bob".to_string(),
            active: false,
        };

        // Send update_user message to trigger echo behavior
        let update_message = Message::new("update_user", user_data.clone());
        actor_tx.send(update_message).unwrap();

        // Give the actor time to process and echo back
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the echo message was sent back
        let echoed_message = rx.recv().await.unwrap();
        assert_eq!(echoed_message.method, "user_updated");
        assert_eq!(echoed_message.payload.id, user_data.id);
        assert_eq!(echoed_message.payload.name, user_data.name);
        assert_eq!(echoed_message.payload.active, !user_data.active); // Should be toggled

        // Verify the handler recorded the original message
        let received_messages = handler_clone.get_messages();
        assert_eq!(received_messages.len(), 1);
        assert_eq!(received_messages[0].payload, user_data);

        // Clean up
        drop(actor);
    }

    #[tokio::test]
    async fn test_complex_taskinfo_struct() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = TaskInfoHandler::new();
        let handler_clone = handler.clone();
        let actor: Actor<TaskInfo, TaskInfoHandler> = Actor::new(actor_rx, tx, handler);

        // Create complex task info with Vec and HashMap
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "Charlie".to_string());
        metadata.insert("deadline".to_string(), "2024-12-31".to_string());

        let task_info = TaskInfo {
            task_id: "task-789".to_string(),
            priority: 5,
            tags: vec!["urgent".to_string(), "backend".to_string()],
            metadata,
        };

        // Send process_task message to trigger complex processing
        let process_message = Message::new("process_task", task_info.clone());
        actor_tx.send(process_message).unwrap();

        // Give the actor time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Verify the processed message was sent back
        let processed_message = rx.recv().await.unwrap();
        assert_eq!(processed_message.method, "task_processed");
        assert_eq!(processed_message.payload.task_id, task_info.task_id);
        assert_eq!(processed_message.payload.priority, task_info.priority + 1); // Should be incremented

        // Verify new tag was added
        assert!(processed_message
            .payload
            .tags
            .contains(&"processed".to_string()));
        assert_eq!(
            processed_message.payload.tags.len(),
            task_info.tags.len() + 1
        );

        // Verify metadata was updated
        assert_eq!(
            processed_message.payload.metadata.get("status"),
            Some(&"completed".to_string())
        );

        // Verify the handler recorded the original message
        let received_messages = handler_clone.get_messages();
        assert_eq!(received_messages.len(), 1);
        assert_eq!(received_messages[0].payload.task_id, task_info.task_id);

        // Clean up
        drop(actor);
    }

    // Echo handler that responds to string messages
    struct StringEchoHandler;

    #[async_trait]
    impl MessageHandler<String> for StringEchoHandler {
        async fn on_message(
            &mut self,
            message: Message<String>,
            controller: &crate::core::ActorController<String>,
        ) {
            log::debug!("StringEchoHandler received message: {}", message.method);

            // Echo the message back with a modified method name and payload
            let echo_method = format!("echo_{}", message.method);
            let echo_payload = format!("echo: {}", message.payload);
            let _ = controller.send_message(echo_method, echo_payload).await;
        }
    }

    #[tokio::test]
    async fn test_string_echo_handler() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let handler = StringEchoHandler;
        let actor: Actor<String, StringEchoHandler> = Actor::new(actor_rx, tx, handler);

        // Send a message to the actor
        let test_message = Message::new("hello", "world".to_string());

        actor_tx.send(test_message).unwrap();

        // Receive the echoed message
        let echoed = rx.recv().await.unwrap();
        assert_eq!(echoed.method, "echo_hello");
        assert_eq!(echoed.payload, "echo: world");

        // Clean up
        drop(actor);
    }
}
