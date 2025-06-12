//! # fae - Fast And Elegant code search
//! 
//! A fast, interactive code search tool with multi-mode search capabilities.
//! 
//! ## Features
//! 
//! - **Multi-mode search**: Content, Symbol (#), File (>), Regex (/)
//! - **Real-time results**: Instant search as you type
//! - **Smart caching**: Memory-efficient LRU cache with disk persistence
//! - **TUI interface**: Beautiful terminal interface with keyboard navigation
//! - **File watching**: Automatic updates when files change
//! 
//! ## Example
//! 
//! ```rust,no_run
//! use fae::cache_manager::CacheManager;
//! use std::path::Path;
//! 
//! let mut cache = CacheManager::new();
//! let symbols = cache.get_symbols(Path::new("src/main.rs")).unwrap();
//! 
//! for symbol in symbols {
//!     println!("{}: {} at line {}", symbol.symbol_type.icon(), symbol.name, symbol.line);
//! }
//! ```

pub mod types;
pub mod cache_manager;
pub mod display;
pub mod languages;
pub mod symbol_index;
pub mod tree_sitter;

// Re-export commonly used types
pub use types::{
    SearchMode, SearchResult, DisplayInfo, SymbolType,
    CachedFileInfo, CachedSymbol, FormattedResult,
};

pub use cache_manager::{CacheManager, CacheStats};
pub use display::DisplayFormatter;
pub use symbol_index::{SymbolIndex, MetadataStorage, SymbolMetadata, SearchHit};

// Tree-sitter integration (to be implemented)
pub use tree_sitter::extract_symbols_from_file;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default cache limits
pub mod defaults {
    /// Default maximum memory usage in MB
    pub const MAX_MEMORY_MB: usize = 100;
    
    /// Default maximum number of cached entries
    pub const MAX_ENTRIES: usize = 1000;
    
    /// Default terminal width if detection fails
    pub const TERMINAL_WIDTH: usize = 80;
}