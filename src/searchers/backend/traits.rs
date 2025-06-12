use crate::types::SearchResult;
use anyhow::Result;
use std::path::Path;

/// 外部検索ツールバックエンドの共通インターフェース
pub trait ExternalSearchBackend: Send + Sync {
    /// バックエンドの名前を取得
    fn name(&self) -> &'static str;
    
    /// バックエンドが利用可能かチェック
    fn is_available(&self) -> bool;
    
    /// コンテンツ検索を実行
    fn search_content(&self, project_root: &Path, query: &str) -> Result<Vec<SearchResult>>;
    
    /// 正規表現検索を実行
    fn search_regex(&self, project_root: &Path, pattern: &str) -> Result<Vec<SearchResult>> {
        // デフォルト実装：通常のコンテンツ検索にフォールバック
        self.search_content(project_root, pattern)
    }
    
    /// バックエンドの優先度を取得（高いほど優先）
    fn priority(&self) -> u32;
}