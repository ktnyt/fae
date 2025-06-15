//! Core module for notification-based communication
//!
//! This module provides a lightweight actor system based on notification messages.
//! Unlike the JsonRpc system which supports request/response patterns, this core
//! system focuses solely on bidirectional notification passing.

pub mod actor;
pub mod broadcaster;
pub mod message;

// Re-exports for convenience
pub use actor::{Actor, ActorSendError, ActorSender};
pub use broadcaster::Broadcaster;
pub use message::{Message, MessageHandler};
