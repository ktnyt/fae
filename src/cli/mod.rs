//! CLI module with Strategy pattern for search execution
//! 
//! このモジュールは検索機能のCLIインターフェースを提供します。
//! Strategy patternを使用して各検索モード（Content, Symbol, File等）を
//! 統一的に実行できる設計になっています。
//! 
//! # アーキテクチャ
//! 
//! ```text
//! CLI Args -> SearchRunner -> SearchStrategy -> SearchResultStream -> Formatters -> Output
//! ```
//! 
//! - `SearchRunner`: 検索実行エンジン（CLI/TUI共通）
//! - `SearchStrategy`: 検索モード別の戦略実装
//! - `strategies/`: 各検索モードの具体実装

pub mod search_strategy;
pub mod search_runner;
pub mod strategies;
pub mod cli_app;

// Re-exports for easy access
pub use search_strategy::{SearchStrategy, SearchResultStream};
pub use search_runner::SearchRunner;
pub use cli_app::run_cli;

// Strategy implementations
pub use strategies::{
    ContentStrategy,
    SymbolStrategy, 
    FileStrategy,
    RegexStrategy,
};