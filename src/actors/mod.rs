//! Actor implementations for specific use cases
//!
//! This module contains concrete actor implementations that can be used
//! with the core actor system for various tasks like command execution,
//! file processing, and other background operations.

pub mod command;

pub use command::{CommandActor, StringCommandActor, CommandHandler, StringHandler};
