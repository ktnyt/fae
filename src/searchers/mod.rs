//! 検索エンジンモジュール
//! 
//! 各種検索モードの実装を提供します：
//! - コンテンツ検索（grep風）
//! - シンボル検索（Tree-sitterベース）
//! - ファイル名検索
//! - 正規表現検索
//! - 外部バックエンド（ripgrep、ag）

pub mod content_search;
pub mod enhanced_content_search;
pub mod file_search;
pub mod backend;

// Re-export for easier access
pub use content_search::{ContentSearcher, ContentSearchStream};
pub use enhanced_content_search::{EnhancedContentSearcher, EnhancedContentSearchStream};
pub use file_search::{FileSearcher, FileSearchStream};
pub use backend::{ExternalSearchBackend, BackendDetector, RipgrepBackend, AgBackend};