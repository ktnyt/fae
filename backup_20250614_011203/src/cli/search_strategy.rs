use crate::types::SearchResult;
use crate::display::ResultFormatter;
use anyhow::Result;
use std::path::PathBuf;

/// 検索結果のストリーム（統一インターフェース）
pub type SearchResultStream = Box<dyn Iterator<Item = SearchResult>>;

/// 検索戦略の統一インターフェース
/// 
/// 各検索モード（Content, Symbol, File等）はこのトレイトを実装することで
/// 統一的な実行フローに組み込める。TUI実装時にも同じインターフェースを使用可能。
pub trait SearchStrategy: Send + Sync {
    /// 検索戦略の名前（ログ・デバッグ用）
    fn name(&self) -> &'static str;
    
    /// 検索ストリームを作成
    /// 
    /// # Arguments
    /// * `project_root` - プロジェクトルートディレクトリ
    /// * `query` - 検索クエリ（プレフィックス除去済み）
    fn create_stream(&self, project_root: &PathBuf, query: &str) -> Result<SearchResultStream>;
    
    /// フォーマッターペアを作成（TTY形式, Pipe形式）
    /// 
    /// # Returns
    /// * `(heading_formatter, inline_formatter)` - TTY用とPipe用のフォーマッター
    fn create_formatters(&self, project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>);
    
    /// ファイルグルーピング（ヘッダー表示）をサポートするか
    /// 
    /// # Returns
    /// * Content/Symbol検索: `true` - ファイルごとにヘッダー表示
    /// * File検索: `false` - ファイルリストなのでグルーピング不要
    fn supports_file_grouping(&self) -> bool;
    
    /// 検索前の準備処理（オプション）
    /// 
    /// 例: Symbol検索でのインデックス構築、外部バックエンドの初期化など
    fn prepare(&self, _project_root: &PathBuf) -> Result<()> {
        Ok(()) // デフォルト実装: 何もしない
    }
    
    /// 検索タイプ固有のメタ情報を取得（オプション）
    /// 
    /// 例: Content検索での使用バックエンド情報、Symbol検索でのインデックス統計など
    fn meta_info(&self, _project_root: &PathBuf) -> Result<Option<String>> {
        Ok(None) // デフォルト実装: メタ情報なし
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SearchResult, DisplayInfo};
    use crate::display::ResultFormatter;

    // テスト用のモック戦略
    struct MockStrategy;
    
    impl SearchStrategy for MockStrategy {
        fn name(&self) -> &'static str {
            "mock"
        }
        
        fn create_stream(&self, _project_root: &PathBuf, _query: &str) -> Result<SearchResultStream> {
            let results = vec![
                SearchResult {
                    file_path: PathBuf::from("test.rs"),
                    line: 1,
                    column: 1,
                    display_info: DisplayInfo::Content {
                        line_content: "test content".to_string(),
                        match_start: 0,
                        match_end: 4,
                    },
                    score: 1.0,
                }
            ];
            Ok(Box::new(results.into_iter()))
        }
        
        fn create_formatters(&self, _project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
            // TODO: モックフォーマッターを実装
            todo!("Mock formatters not implemented yet")
        }
        
        fn supports_file_grouping(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_mock_strategy_basic() {
        let strategy = MockStrategy;
        assert_eq!(strategy.name(), "mock");
        assert!(strategy.supports_file_grouping());
    }

    #[test]
    fn test_mock_strategy_stream() -> Result<()> {
        let strategy = MockStrategy;
        let project_root = PathBuf::from("/test");
        let stream = strategy.create_stream(&project_root, "test")?;
        
        let results: Vec<_> = stream.collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, PathBuf::from("test.rs"));
        
        Ok(())
    }
}