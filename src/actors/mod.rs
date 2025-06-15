//! Actor implementations for fae
//!
//! This module contains concrete actor implementations that extend
//! the core actor system with specific functionality.

pub mod rg;

pub use rg::{RipgrepActor, SearchMessage, SearchResult};