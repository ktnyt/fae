//! Core module for notification-based communication
//!
//! This module provides a lightweight actor system based on notification messages.
//! Unlike the JsonRpc system which supports request/response patterns, this core
//! system focuses solely on bidirectional notification passing.

pub mod actor;
pub mod broadcaster;
pub mod command;
pub mod message;

// Re-exports for convenience
pub use actor::{Actor, ActorController, ActorSendError};
pub use broadcaster::Broadcaster;
pub use command::{CommandActor, CommandController, CommandFactory, CommandHandler};
pub use message::{Message, MessageHandler};
