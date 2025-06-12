//! 検索エンジンモジュール
//! 
//! 各種検索モードの実装を提供します：
//! - コンテンツ検索（grep風）
//! - シンボル検索（Tree-sitterベース）
//! - ファイル名検索
//! - 正規表現検索

pub mod content_search;

// Re-export for easier access
pub use content_search::{ContentSearcher, ContentSearchStream};