//! Message types and handler traits for the Actor system.

use async_trait::async_trait;

/// A generic message in the Actor system.
/// T represents the payload type, which can be any type that satisfies the required bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct Message<T> {
    /// The method name for this message
    pub method: String,
    /// The message payload
    pub payload: T,
}

impl<T> Message<T> {
    /// Create a new Message with the specified method and payload.
    pub fn new(method: impl Into<String>, payload: T) -> Self {
        Self {
            method: method.into(),
            payload,
        }
    }
}

/// Common message types that can be used with the Actor system
pub mod types {
    use super::Message;
    
    /// A message with no payload (method-only)
    pub type MethodOnlyMessage = Message<()>;
    
    /// A message with string payload
    pub type StringMessage = Message<String>;
    
    /// A message with integer payload
    pub type IntMessage = Message<i32>;
    
    /// A message with boolean payload
    pub type BoolMessage = Message<bool>;
    
    impl MethodOnlyMessage {
        pub fn method_only(method: impl Into<String>) -> Self {
            Message::new(method, ())
        }
    }
    
    impl StringMessage {
        pub fn with_string(method: impl Into<String>, payload: String) -> Self {
            Message::new(method, payload)
        }
    }
    
    impl IntMessage {
        pub fn with_int(method: impl Into<String>, payload: i32) -> Self {
            Message::new(method, payload)
        }
    }
    
    impl BoolMessage {
        pub fn with_bool(method: impl Into<String>, payload: bool) -> Self {
            Message::new(method, payload)
        }
    }
}

/// Extended controller interface for command actors.
/// 
/// This trait extends basic message sending with command lifecycle control.
/// It allows message handlers to spawn and shutdown commands as needed.
#[async_trait]
pub trait CommandController<T>: Send + Sync {
    /// Send a message to external recipients.
    async fn send_message(&self, method: impl Into<String> + Send, payload: T) -> Result<(), crate::core::ActorSendError>;
    
    /// Spawn a new command execution.
    async fn spawn(&self, command: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    
    /// Shutdown the currently running command.
    async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Trait for handling messages specifically in command actors.
/// 
/// This trait provides access to CommandController for lifecycle management.
/// It's separate from MessageHandler to avoid breaking existing Actor implementations.
#[async_trait]
pub trait CommandMessageHandler<T>: Send + Sync {
    /// Handle an incoming message with command control capabilities.
    /// 
    /// # Arguments
    /// * `message` - The incoming message to process
    /// * `controller` - Controller for message sending and command lifecycle management
    async fn on_message(&mut self, message: Message<T>, controller: &dyn CommandController<T>);
}

/// Trait for handling messages in the Actor system.
/// 
/// This trait is generic over the message payload type T, allowing for type-safe
/// message handling without JSON serialization overhead.
/// Implementors define how to process incoming messages and can send responses or
/// additional messages using the provided ActorSender.
#[async_trait]
pub trait MessageHandler<T> {
    /// Handle an incoming message.
    /// 
    /// # Arguments
    /// * `message` - The incoming message to process
    /// * `sender` - Sender for outgoing messages (responses, forwarding, etc.)
    async fn on_message(&mut self, message: Message<T>, sender: &crate::core::ActorSender<T>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::types::*;
    
    #[test]
    fn test_generic_message_creation() {
        // Test method-only message
        let msg1 = MethodOnlyMessage::method_only("ping");
        assert_eq!(msg1.method, "ping");
        assert_eq!(msg1.payload, ());
        
        // Test string message
        let msg2 = StringMessage::with_string("echo", "hello world".to_string());
        assert_eq!(msg2.method, "echo");
        assert_eq!(msg2.payload, "hello world");
        
        // Test integer message
        let msg3 = IntMessage::with_int("count", 42);
        assert_eq!(msg3.method, "count");
        assert_eq!(msg3.payload, 42);
        
        // Test boolean message
        let msg4 = BoolMessage::with_bool("status", true);
        assert_eq!(msg4.method, "status");
        assert_eq!(msg4.payload, true);
    }
    
    #[test]
    fn test_direct_message_creation() {
        // Test direct generic constructor
        let msg1 = Message::new("test", "data".to_string());
        assert_eq!(msg1.method, "test");
        assert_eq!(msg1.payload, "data");
        
        let msg2 = Message::new("number", 123i32);
        assert_eq!(msg2.method, "number");
        assert_eq!(msg2.payload, 123);
    }
    
    #[test]
    fn test_message_clone_and_equality() {
        let original = StringMessage::new("test", "data".to_string());
        let cloned = original.clone();
        
        assert_eq!(original, cloned);
        assert_eq!(original.method, cloned.method);
        assert_eq!(original.payload, cloned.payload);
    }
}