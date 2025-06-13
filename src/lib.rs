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
//! 
//! // ファイルからシンボルを抽出してキャッシュ
//! let symbols = cache.get_symbols(Path::new("src/main.rs")).unwrap();
//! for symbol in symbols {
//!     println!("{}: {} at line {}", symbol.symbol_type.icon(), symbol.name, symbol.line);
//! }
//! 
//! // シンボルをファジー検索
//! let search_results = cache.fuzzy_search_symbols("main", 10);
//! for hit in search_results {
//!     println!("Found: {} (score: {})", hit.metadata.name, hit.score);
//!     
//!     // 完全な詳細情報がヒットに含まれている
//!     println!("  {}:{}:{}", hit.metadata.file_path.display(), hit.metadata.line, hit.metadata.column);
//! }
//! ```

pub mod types;
pub mod cache_manager;
pub mod cli;
pub mod display;
pub mod index_manager;
pub mod languages;
pub mod realtime_indexer;
pub mod search_coordinator;
pub mod searchers;
pub mod symbol_index;
pub mod tree_sitter;
pub mod tui;
pub mod workers;

// Re-export commonly used types
pub use types::{
    SearchMode, SearchResult, DisplayInfo, SymbolType,
    CachedFileInfo, CachedSymbol, FormattedResult,
};

pub use cache_manager::{CacheManager, CacheStats};
pub use cli::{SearchRunner, SearchStrategy, SearchResultStream};
pub use display::{DisplayFormatter, CliFormatter, TuiFormatter, ResultFormatter};
pub use index_manager::{IndexManager, FileInfo};
pub use realtime_indexer::{RealtimeIndexer, FileChangeEvent, IndexUpdateResult};
pub use search_coordinator::{SearchCoordinator, IndexProgress, IndexResult, SymbolSearchStream};
pub use searchers::{ContentSearcher, ContentSearchStream, RegexSearcher, RegexSearchStream};
pub use symbol_index::{SymbolIndex, SymbolMetadata, SearchHit};
pub use workers::{
    Worker, WorkerHandle, WorkerManager, Message, MessageBus, WorkerMessage,
    TuiWorker, SearchHandlerWorker, ContentSearchWorker
};

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