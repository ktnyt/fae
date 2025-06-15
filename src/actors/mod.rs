//! Actor implementations for fae
//!
//! This module contains concrete actor implementations that extend
//! the core actor system with specific functionality.

pub mod messages;
pub mod ripgrep;

pub use ripgrep::RipgrepActor;