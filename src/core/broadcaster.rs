//! Broadcaster implementation for transparent actor communication
//!
//! This module provides a broadcaster that automatically relays messages between
//! multiple actors. When any actor sends a message using the shared sender,
//! it gets broadcast to all registered actors automatically.

use crate::core::message::Message;
use std::marker::PhantomData;
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};

pub struct Multiplexer<T: Clone + Send + 'static> {
    receivers: Vec<mpsc::UnboundedReceiver<Message<T>>>,
    senders: Vec<mpsc::UnboundedSender<Message<T>>>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl<T: Clone + Send + 'static> Multiplexer<T> {
    pub fn new(senders: Vec<mpsc::UnboundedSender<Message<T>>>) -> Self {
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
        Self {
            receivers: Vec::new(),
            senders,
            shutdown_sender: Some(shutdown_sender),
            thread_handle: None,
        }
    }

    pub fn register(
        &mut self,
        target: mpsc::UnboundedReceiver<Message<T>>,
    ) -> mpsc::UnboundedReceiver<Message<T>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.receivers.push(target);
        self.senders.push(sender);
        receiver
    }

    pub fn run(&mut self) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::new();

        // Move shutdown_receiver out of self for use in the spawned task
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        self.shutdown_sender = Some(shutdown_sender);

        let senders = self.senders.clone();
        let receivers = std::mem::take(&mut self.receivers);

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                Self::run_multiplexer_loop(receivers, senders, shutdown_receiver).await;
            });
        });

        self.thread_handle = Some(handle);
        handles.push(self.thread_handle.take().unwrap());
        handles
    }

    async fn run_multiplexer_loop(
        receivers: Vec<mpsc::UnboundedReceiver<Message<T>>>,
        senders: Vec<mpsc::UnboundedSender<Message<T>>>,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        // Spawn a task for each receiver
        let mut tasks = Vec::new();
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<Message<T>>();

        for mut receiver in receivers {
            let tx = message_tx.clone();
            let task = tokio::spawn(async move {
                while let Some(message) = receiver.recv().await {
                    if tx.send(message).is_err() {
                        break;
                    }
                }
            });
            tasks.push(task);
        }

        // Drop the original sender to allow proper shutdown detection
        drop(message_tx);

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_receiver => {
                    log::debug!("Received shutdown signal for Multiplexer");
                    break;
                }

                // Handle messages from any receiver
                message = message_rx.recv() => {
                    match message {
                        Some(msg) => {
                            log::trace!("Multiplexer broadcasting message: {}", msg.method);
                            Self::broadcast_to_all(&senders, msg).await;
                        }
                        None => {
                            log::debug!("All receivers closed, shutting down multiplexer");
                            break;
                        }
                    }
                }
            }
        }

        // Clean up tasks
        for task in tasks {
            task.abort();
        }
    }

    async fn broadcast_to_all(senders: &[mpsc::UnboundedSender<Message<T>>], message: Message<T>) {
        for sender in senders.iter() {
            if let Err(e) = sender.send(message.clone()) {
                log::warn!("Failed to broadcast message to target: {}", e);
            }
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }

        if let Some(thread_handle) = self.thread_handle.take() {
            let _ = thread_handle.join();
        }
    }
}

/// A broadcaster that transparently relays messages between multiple actors
pub struct Broadcaster<T: Clone + Send + 'static> {
    /// Handle to the broadcast loop thread
    thread_handle: Option<JoinHandle<()>>,
    /// Channel for sending shutdown signal
    shutdown_sender: Option<oneshot::Sender<()>>,
    /// Phantom data to hold the message type
    _phantom: PhantomData<T>,
}

impl<T: Clone + Send + 'static> Broadcaster<T> {
    /// Create a new broadcaster with the given actor senders
    ///
    /// Returns the broadcaster instance and a shared sender that all actors should use
    pub fn new(
        actor_senders: Vec<mpsc::UnboundedSender<Message<T>>>,
    ) -> (Self, mpsc::UnboundedSender<Message<T>>) {
        let (shared_sender, broadcast_receiver) = mpsc::unbounded_channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        // Start the broadcast loop in a separate thread
        let thread_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(Self::run_broadcast_loop(
                broadcast_receiver,
                actor_senders,
                shutdown_receiver,
            ));
        });

        let broadcaster = Self {
            thread_handle: Some(thread_handle),
            shutdown_sender: Some(shutdown_sender),
            _phantom: PhantomData,
        };

        (broadcaster, shared_sender)
    }

    /// Main broadcast loop that relays messages to all actors
    async fn run_broadcast_loop(
        mut broadcast_receiver: mpsc::UnboundedReceiver<Message<T>>,
        actor_senders: Vec<mpsc::UnboundedSender<Message<T>>>,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = &mut shutdown_receiver => {
                    log::debug!("Received shutdown signal, stopping broadcast loop");
                    break;
                }

                // Handle messages from the shared broadcast channel
                message = broadcast_receiver.recv() => {
                    match message {
                        Some(message) => {
                            log::trace!("Broadcasting message to {} actors: method={}",
                                       actor_senders.len(), message.method);
                            Self::broadcast_to_all(&actor_senders, message).await;
                        }
                        None => {
                            log::debug!("Broadcast receiver channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Broadcast a message to all registered actors
    async fn broadcast_to_all(
        actor_senders: &[mpsc::UnboundedSender<Message<T>>],
        message: Message<T>,
    ) {
        for (i, sender) in actor_senders.iter().enumerate() {
            match sender.send(message.clone()) {
                Ok(_) => {
                    log::trace!("Message sent to actor {}", i);
                }
                Err(_) => {
                    log::warn!("Failed to send message to actor {} (channel closed)", i);
                }
            }
        }
    }

    /// Manually shutdown the broadcaster
    pub fn shutdown(&mut self) {
        log::info!("Manual shutdown requested for Broadcaster");

        // Send shutdown signal
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            log::debug!("Sending shutdown signal");
            let _ = shutdown_sender.send(());
        }

        // Wait for thread to finish
        if let Some(thread_handle) = self.thread_handle.take() {
            log::debug!("Waiting for broadcast loop thread to finish");
            let _ = thread_handle.join();
        }

        log::info!("Broadcaster shutdown completed");
    }
}

impl<T: Clone + Send + 'static> Drop for Broadcaster<T> {
    fn drop(&mut self) {
        if self.shutdown_sender.is_some() || self.thread_handle.is_some() {
            log::debug!("Broadcaster dropped without explicit shutdown, performing cleanup");
            self.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Actor;
    use crate::core::{Message, MessageHandler};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;
    use tokio::time::{sleep, Duration};

    // Test message type
    #[derive(Debug, Clone, PartialEq)]
    struct TestMessage {
        content: String,
        number: i32,
    }

    // Test handler that collects received messages
    #[derive(Clone)]
    struct TestHandler {
        received_messages: Arc<Mutex<Vec<Message<TestMessage>>>>,
        actor_id: String,
    }

    impl TestHandler {
        fn new(actor_id: impl Into<String>) -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
                actor_id: actor_id.into(),
            }
        }

        fn get_messages(&self) -> Vec<Message<TestMessage>> {
            self.received_messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl MessageHandler<TestMessage> for TestHandler {
        async fn on_message(
            &mut self,
            message: Message<TestMessage>,
            _controller: &crate::core::ActorController<TestMessage>,
        ) {
            log::debug!("Actor {} received message: {:?}", self.actor_id, message);
            let mut messages = self.received_messages.lock().unwrap();
            messages.push(message);
        }
    }

    #[tokio::test]
    async fn test_multiplexer_creation() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx1, _) = mpsc::unbounded_channel::<Message<TestMessage>>();
        let (tx2, _) = mpsc::unbounded_channel::<Message<TestMessage>>();

        let _multiplexer = Multiplexer::new(vec![tx1, tx2]);
        // Test that we can create the multiplexer without issues
        sleep(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn test_multiplexer_broadcast() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // Create output receivers to capture broadcasts
        let (output_tx1, mut output_rx1) = mpsc::unbounded_channel::<Message<TestMessage>>();
        let (output_tx2, mut output_rx2) = mpsc::unbounded_channel::<Message<TestMessage>>();

        // Create multiplexer with output senders
        let mut multiplexer = Multiplexer::new(vec![output_tx1, output_tx2]);

        // Create input channels and register them
        let (input_tx1, input_rx1) = mpsc::unbounded_channel::<Message<TestMessage>>();
        let (input_tx2, input_rx2) = mpsc::unbounded_channel::<Message<TestMessage>>();

        let _managed_rx1 = multiplexer.register(input_rx1);
        let _managed_rx2 = multiplexer.register(input_rx2);

        // Start the multiplexer
        let _handles = multiplexer.run();

        sleep(Duration::from_millis(10)).await;

        // Send test messages to input channels
        let test_message1 = TestMessage {
            content: "test1".to_string(),
            number: 1,
        };
        let test_message2 = TestMessage {
            content: "test2".to_string(),
            number: 2,
        };

        input_tx1
            .send(Message::new("method1", test_message1.clone()))
            .unwrap();
        input_tx2
            .send(Message::new("method2", test_message2.clone()))
            .unwrap();

        sleep(Duration::from_millis(20)).await;

        // Check that both output receivers got both messages
        let mut messages1 = Vec::new();
        let mut messages2 = Vec::new();

        while let Ok(msg) = output_rx1.try_recv() {
            messages1.push(msg);
        }
        while let Ok(msg) = output_rx2.try_recv() {
            messages2.push(msg);
        }

        assert_eq!(messages1.len(), 2, "Output 1 should receive 2 messages");
        assert_eq!(messages2.len(), 2, "Output 2 should receive 2 messages");

        // Verify message content
        assert_eq!(messages1[0].payload.content, "test1");
        assert_eq!(messages1[1].payload.content, "test2");
        assert_eq!(messages2[0].payload.content, "test1");
        assert_eq!(messages2[1].payload.content, "test2");

        multiplexer.shutdown();
    }

    #[tokio::test]
    async fn test_broadcaster_creation() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // Create some dummy senders for testing
        let (tx1, _) = mpsc::unbounded_channel::<Message<TestMessage>>();
        let (tx2, _) = mpsc::unbounded_channel::<Message<TestMessage>>();

        let (_broadcaster, _shared_sender) = Broadcaster::new(vec![tx1, tx2]);

        // Test that we can create the broadcaster without issues
        sleep(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn test_broadcaster_with_actors() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // Create actor receivers
        let (actor1_tx, actor1_rx) = mpsc::unbounded_channel();
        let (actor2_tx, actor2_rx) = mpsc::unbounded_channel();
        let (actor3_tx, actor3_rx) = mpsc::unbounded_channel();

        // Create broadcaster with actor senders
        let (_broadcaster, shared_sender) = Broadcaster::new(vec![actor1_tx, actor2_tx, actor3_tx]);

        // Create actors using the shared sender
        let handler1 = TestHandler::new("actor1");
        let handler2 = TestHandler::new("actor2");
        let handler3 = TestHandler::new("actor3");

        let handler1_clone = handler1.clone();
        let handler2_clone = handler2.clone();
        let handler3_clone = handler3.clone();

        let actor1: Actor<TestMessage, TestHandler> =
            Actor::new(actor1_rx, shared_sender.clone(), handler1);
        let actor2: Actor<TestMessage, TestHandler> =
            Actor::new(actor2_rx, shared_sender.clone(), handler2);
        let actor3: Actor<TestMessage, TestHandler> =
            Actor::new(actor3_rx, shared_sender.clone(), handler3);

        // Give time for actors to start
        sleep(Duration::from_millis(10)).await;

        // Send a message from actor1 - should be broadcast to all actors
        let test_message = TestMessage {
            content: "broadcast test".to_string(),
            number: 42,
        };

        actor1
            .send_message("test_method", test_message.clone())
            .await
            .unwrap();

        // Give time for message processing
        sleep(Duration::from_millis(20)).await;

        // Verify all actors received the message
        let messages1 = handler1_clone.get_messages();
        let messages2 = handler2_clone.get_messages();
        let messages3 = handler3_clone.get_messages();

        assert_eq!(messages1.len(), 1);
        assert_eq!(messages2.len(), 1);
        assert_eq!(messages3.len(), 1);

        assert_eq!(messages1[0].method, "test_method");
        assert_eq!(messages1[0].payload.content, test_message.content);
        assert_eq!(messages1[0].payload.number, test_message.number);

        assert_eq!(messages2[0].method, "test_method");
        assert_eq!(messages2[0].payload.content, test_message.content);
        assert_eq!(messages2[0].payload.number, test_message.number);

        assert_eq!(messages3[0].method, "test_method");
        assert_eq!(messages3[0].payload.content, test_message.content);
        assert_eq!(messages3[0].payload.number, test_message.number);

        // Cleanup
        drop(actor1);
        drop(actor2);
        drop(actor3);
        sleep(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn test_multiple_messages_broadcast() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // Create actor receivers
        let (actor1_tx, actor1_rx) = mpsc::unbounded_channel();
        let (actor2_tx, actor2_rx) = mpsc::unbounded_channel();

        // Create broadcaster
        let (_broadcaster, shared_sender) = Broadcaster::new(vec![actor1_tx, actor2_tx]);

        // Create actors
        let handler1 = TestHandler::new("actor1");
        let handler2 = TestHandler::new("actor2");

        let handler1_clone = handler1.clone();
        let handler2_clone = handler2.clone();

        let actor1: Actor<TestMessage, TestHandler> =
            Actor::new(actor1_rx, shared_sender.clone(), handler1);
        let actor2: Actor<TestMessage, TestHandler> =
            Actor::new(actor2_rx, shared_sender.clone(), handler2);

        sleep(Duration::from_millis(10)).await;

        // Send multiple messages from different actors
        actor1
            .send_message(
                "message1",
                TestMessage {
                    content: "from actor1".to_string(),
                    number: 1,
                },
            )
            .await
            .unwrap();

        actor2
            .send_message(
                "message2",
                TestMessage {
                    content: "from actor2".to_string(),
                    number: 2,
                },
            )
            .await
            .unwrap();

        sleep(Duration::from_millis(20)).await;

        // Verify both actors received both messages
        let messages1 = handler1_clone.get_messages();
        let messages2 = handler2_clone.get_messages();

        assert_eq!(messages1.len(), 2);
        assert_eq!(messages2.len(), 2);

        // Check message order and content
        assert_eq!(messages1[0].method, "message1");
        assert_eq!(messages1[0].payload.content, "from actor1");
        assert_eq!(messages1[1].method, "message2");
        assert_eq!(messages1[1].payload.content, "from actor2");

        assert_eq!(messages2[0].method, "message1");
        assert_eq!(messages2[0].payload.content, "from actor1");
        assert_eq!(messages2[1].method, "message2");
        assert_eq!(messages2[1].payload.content, "from actor2");

        // Cleanup
        drop(actor1);
        drop(actor2);
        sleep(Duration::from_millis(10)).await;
    }
}
