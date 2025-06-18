//! Channel integration and multiplexing utilities
//!
//! This module provides utilities for integrating multiple channels into one
//! and multiplexing one channel into multiple channels for improved Actor
//! system communication patterns.

use std::collections::HashMap;
use std::hash::Hash;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Integrates multiple senders into a single receiver channel
///
/// Takes multiple UnboundedSender instances and provides a single
/// UnboundedReceiver that receives messages from all senders.
pub struct ChannelIntegrator<T> {
    receiver: mpsc::UnboundedReceiver<T>,
    _handles: Vec<JoinHandle<()>>,
}

impl<T> ChannelIntegrator<T>
where
    T: Send + 'static,
{
    /// Create a new ChannelIntegrator from multiple senders
    ///
    /// Returns a ChannelIntegrator that will receive messages from all provided senders.
    /// The original senders can still be used to send messages.
    pub fn new(mut senders: Vec<mpsc::UnboundedReceiver<T>>) -> Self {
        let (integrated_tx, integrated_rx) = mpsc::unbounded_channel();
        let mut handles = Vec::new();

        for mut receiver in senders.drain(..) {
            let tx_clone = integrated_tx.clone();
            let handle = tokio::spawn(async move {
                while let Some(message) = receiver.recv().await {
                    if tx_clone.send(message).is_err() {
                        // Integrated receiver was dropped, exit this task
                        break;
                    }
                }
            });
            handles.push(handle);
        }

        Self {
            receiver: integrated_rx,
            _handles: handles,
        }
    }

    /// Receive a message from any of the integrated senders
    pub async fn recv(&mut self) -> Option<T> {
        self.receiver.recv().await
    }

    /// Try to receive a message without blocking
    pub fn try_recv(&mut self) -> Result<T, mpsc::error::TryRecvError> {
        self.receiver.try_recv()
    }

    /// Close the integrated receiver
    pub fn close(&mut self) {
        self.receiver.close();
    }
}

/// Multiplexes a single receiver into multiple senders based on a routing key
///
/// Takes a single UnboundedReceiver and routes messages to different
/// UnboundedSenders based on a routing function.
pub struct ChannelMultiplexer<T, K>
where
    K: Eq + Hash + Clone,
{
    _handle: JoinHandle<()>,
    senders: std::sync::Arc<std::sync::Mutex<HashMap<K, mpsc::UnboundedSender<T>>>>,
}

impl<T, K> ChannelMultiplexer<T, K>
where
    T: Send + 'static,
    K: Eq + Hash + Clone + Send + 'static,
{
    /// Create a new ChannelMultiplexer
    ///
    /// Takes a receiver and a routing function that determines which sender
    /// should receive each message based on the message content.
    pub fn new<F>(mut receiver: mpsc::UnboundedReceiver<T>, router: F) -> Self
    where
        F: Fn(&T) -> K + Send + 'static,
    {
        let senders = std::sync::Arc::new(std::sync::Mutex::new(HashMap::<
            K,
            mpsc::UnboundedSender<T>,
        >::new()));
        let senders_clone = senders.clone();

        let handle = tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                let route_key = router(&message);

                // Get a copy of the sender for this route
                let sender_option = {
                    let senders_guard = senders_clone.lock().unwrap();
                    senders_guard.get(&route_key).cloned()
                };

                // Send message if we have a sender for this route
                if let Some(sender) = sender_option {
                    if sender.send(message).is_err() {
                        // Receiver was dropped, remove this sender
                        let mut senders_guard = senders_clone.lock().unwrap();
                        senders_guard.remove(&route_key);
                    }
                }
                // If no sender exists for this route, message is dropped
            }
        });

        Self {
            _handle: handle,
            senders,
        }
    }

    /// Add a new receiver for a specific routing key
    ///
    /// Returns an UnboundedReceiver that will receive messages routed to the specified key.
    pub fn add_receiver(&mut self, key: K) -> mpsc::UnboundedReceiver<T> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut senders_guard = self.senders.lock().unwrap();
        senders_guard.insert(key, tx);
        rx
    }

    /// Remove a receiver for a specific routing key
    pub fn remove_receiver(&mut self, key: &K) -> bool {
        let mut senders_guard = self.senders.lock().unwrap();
        senders_guard.remove(key).is_some()
    }
}

/// Builder for ChannelIntegrator to provide more ergonomic API
pub struct ChannelIntegratorBuilder<T> {
    receivers: Vec<mpsc::UnboundedReceiver<T>>,
}

impl<T> ChannelIntegratorBuilder<T>
where
    T: Send + 'static,
{
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            receivers: Vec::new(),
        }
    }

    /// Add a receiver to be integrated
    pub fn add_receiver(mut self, receiver: mpsc::UnboundedReceiver<T>) -> Self {
        self.receivers.push(receiver);
        self
    }

    /// Build the ChannelIntegrator
    pub fn build(self) -> ChannelIntegrator<T> {
        ChannelIntegrator::new(self.receivers)
    }
}

impl<T> Default for ChannelIntegratorBuilder<T>
where
    T: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_channel_integrator_basic() {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();

        let mut integrator = ChannelIntegrator::new(vec![rx1, rx2]);

        // Send messages from both senders
        tx1.send("message1").unwrap();
        tx2.send("message2").unwrap();

        // Should receive both messages
        let msg1 = integrator.recv().await.unwrap();
        let msg2 = integrator.recv().await.unwrap();

        // Messages can arrive in any order
        assert!(msg1 == "message1" || msg1 == "message2");
        assert!(msg2 == "message1" || msg2 == "message2");
        assert_ne!(msg1, msg2);
    }

    #[tokio::test]
    async fn test_channel_integrator_builder() {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();

        let mut integrator = ChannelIntegratorBuilder::new()
            .add_receiver(rx1)
            .add_receiver(rx2)
            .build();

        tx1.send(42).unwrap();
        tx2.send(84).unwrap();

        let received = integrator.recv().await.unwrap();
        assert!(received == 42 || received == 84);
    }

    #[tokio::test]
    async fn test_channel_multiplexer_basic() {
        let (tx, rx) = mpsc::unbounded_channel();

        // Router function that routes based on message content
        let mut multiplexer =
            ChannelMultiplexer::new(rx, |msg: &i32| if *msg % 2 == 0 { "even" } else { "odd" });

        let mut even_rx = multiplexer.add_receiver("even");
        let mut odd_rx = multiplexer.add_receiver("odd");

        // Send some messages
        tx.send(2).unwrap(); // even
        tx.send(3).unwrap(); // odd
        tx.send(4).unwrap(); // even

        // Give the multiplexer time to process
        sleep(Duration::from_millis(10)).await;

        // Check even receiver
        assert_eq!(even_rx.try_recv().unwrap(), 2);
        assert_eq!(even_rx.try_recv().unwrap(), 4);

        // Check odd receiver
        assert_eq!(odd_rx.try_recv().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_channel_integrator_sender_drop() {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();

        let mut integrator = ChannelIntegrator::new(vec![rx1, rx2]);

        tx1.send("message1").unwrap();
        drop(tx1); // Drop one sender

        tx2.send("message2").unwrap();

        // Should still receive both messages
        let msg1 = integrator.recv().await.unwrap();
        let msg2 = integrator.recv().await.unwrap();

        assert!(msg1 == "message1" || msg1 == "message2");
        assert!(msg2 == "message1" || msg2 == "message2");
        assert_ne!(msg1, msg2);
    }

    #[tokio::test]
    async fn test_channel_multiplexer_receiver_drop() {
        let (tx, rx) = mpsc::unbounded_channel();

        let mut multiplexer =
            ChannelMultiplexer::new(rx, |msg: &String| msg.chars().next().unwrap_or('?'));

        let mut a_rx = multiplexer.add_receiver('a');
        let b_rx = multiplexer.add_receiver('b');

        tx.send("apple".to_string()).unwrap();
        tx.send("banana".to_string()).unwrap();

        drop(b_rx); // Drop the 'b' receiver

        tx.send("apricot".to_string()).unwrap();

        sleep(Duration::from_millis(10)).await;

        // 'a' receiver should get both 'a' messages
        assert_eq!(a_rx.try_recv().unwrap(), "apple");
        assert_eq!(a_rx.try_recv().unwrap(), "apricot");
    }
}
