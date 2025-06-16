//! Actor implementations for various functionalities

pub mod ag;
pub mod filepath;
pub mod messages;
pub mod native;
pub mod ripgrep;
pub mod symbol_extractor;
pub mod symbol_index;
pub mod types;
pub mod watch;

#[cfg(test)]
pub mod integration_tests;

pub use ag::{create_ag_command_factory, AgActor, AgHandler};
pub use filepath::{FilepathSearchActor, FilepathSearchHandler};
pub use native::{NativeSearchActor, NativeSearchHandler};
pub use ripgrep::{create_ripgrep_command_factory, RipgrepActor, RipgrepHandler};
pub use symbol_extractor::SymbolExtractor;
pub use symbol_index::{SymbolIndexActor, SymbolIndexHandler};
pub use watch::{WatchActor, WatchHandler};
