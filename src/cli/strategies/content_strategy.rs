use crate::cli::search_strategy::{SearchStrategy, SearchResultStream};
use crate::searchers::EnhancedContentSearcher;
use crate::display::{ContentHeadingFormatter, ContentInlineFormatter, ResultFormatter};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// コンテンツ検索戦略
/// 
/// ファイル内容の文字列検索を行う。ripgrep/ag等の外部バックエンドや
/// 内蔵検索エンジンを自動選択して最適なパフォーマンスを提供。
pub struct ContentStrategy;

impl SearchStrategy for ContentStrategy {
    fn name(&self) -> &'static str {
        "content"
    }
    
    fn create_stream(&self, project_root: &PathBuf, query: &str) -> Result<SearchResultStream> {
        let searcher = EnhancedContentSearcher::new(project_root.clone())
            .context("Failed to create enhanced content searcher")?;
        
        let stream = searcher.search_stream(query)
            .context("Content search failed")?;
        
        Ok(Box::new(stream))
    }
    
    fn create_formatters(&self, project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
        let heading_formatter = ContentHeadingFormatter::new(project_root.clone());
        let inline_formatter = ContentInlineFormatter::new(project_root.clone());
        
        (Box::new(heading_formatter), Box::new(inline_formatter))
    }
    
    fn supports_file_grouping(&self) -> bool {
        true // コンテンツ検索はファイルごとのグルーピングをサポート
    }
    
    fn meta_info(&self, project_root: &PathBuf) -> Result<Option<String>> {
        // バックエンド情報を取得
        let searcher = EnhancedContentSearcher::new(project_root.clone())
            .context("Failed to create searcher for meta info")?;
        
        let (primary, available) = searcher.backend_info();
        let meta = format!("Using backend: {} (available: {})", primary, available.join(", "));
        
        Ok(Some(meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // テスト用ファイル作成
        let mut test_file = File::create(root.join("test.rs"))?;
        writeln!(test_file, "fn test_function() {{")?;
        writeln!(test_file, "    println!(\"Hello, test!\");")?;
        writeln!(test_file, "}}")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_content_strategy_basic() {
        let strategy = ContentStrategy;
        assert_eq!(strategy.name(), "content");
        assert!(strategy.supports_file_grouping());
    }

    #[test]
    fn test_content_strategy_formatters() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = ContentStrategy;
        
        let (heading, inline) = strategy.create_formatters(&temp_dir.path().to_path_buf());
        
        // フォーマッターが作成されることを確認
        // 実際のフォーマット処理のテストは各フォーマッターのテストで行う
        assert_ne!(heading.as_ref().type_id(), std::any::TypeId::of::<()>());
        assert_ne!(inline.as_ref().type_id(), std::any::TypeId::of::<()>());
        
        Ok(())
    }

    #[test]
    fn test_content_strategy_meta_info() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = ContentStrategy;
        
        let meta = strategy.meta_info(&temp_dir.path().to_path_buf())?;
        
        assert!(meta.is_some());
        let meta_str = meta.unwrap();
        assert!(meta_str.contains("Using backend:"));
        
        Ok(())
    }

    #[test]
    fn test_content_strategy_stream() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = ContentStrategy;
        
        let stream = strategy.create_stream(&temp_dir.path().to_path_buf(), "test")?;
        
        // ストリームが作成されることを確認
        // 実際の検索結果のテストはEnhancedContentSearcherのテストで行う
        let results: Vec<_> = stream.collect();
        
        // "test"という文字列が含まれるファイルがあるので、結果が返される
        assert!(!results.is_empty());
        
        Ok(())
    }
}