use crate::cli::search_strategy::{SearchStrategy, SearchResultStream};
use crate::display::{ContentHeadingFormatter, ContentInlineFormatter, ResultFormatter};
use anyhow::Result;
use std::path::PathBuf;

/// 正規表現検索戦略
/// 
/// ファイル内容に対して正規表現パターンマッチングを行う。
/// 現在は未実装で、将来的にregex crateを使用した実装を予定。
pub struct RegexStrategy;

impl SearchStrategy for RegexStrategy {
    fn name(&self) -> &'static str {
        "regex"
    }
    
    fn create_stream(&self, _project_root: &PathBuf, _query: &str) -> Result<SearchResultStream> {
        // TODO: 正規表現検索の実装
        // regex crateを使用してファイル内容を検索する
        Ok(Box::new(std::iter::empty()))
    }
    
    fn create_formatters(&self, project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
        // 正規表現検索は内容検索と同じフォーマッターを使用
        let heading_formatter = ContentHeadingFormatter::new(project_root.clone());
        let inline_formatter = ContentInlineFormatter::new(project_root.clone());
        
        (Box::new(heading_formatter), Box::new(inline_formatter))
    }
    
    fn supports_file_grouping(&self) -> bool {
        true // 正規表現検索はファイルごとのグルーピングをサポート
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use tempfile::TempDir;

    #[test]
    fn test_regex_strategy_basic() {
        let strategy = RegexStrategy;
        assert_eq!(strategy.name(), "regex");
        assert!(strategy.supports_file_grouping());
    }

    #[test]
    fn test_regex_strategy_formatters() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let strategy = RegexStrategy;
        
        let (heading, inline) = strategy.create_formatters(&temp_dir.path().to_path_buf());
        
        // フォーマッターが作成されることを確認
        assert_ne!(heading.as_ref().type_id(), std::any::TypeId::of::<()>());
        assert_ne!(inline.as_ref().type_id(), std::any::TypeId::of::<()>());
        
        Ok(())
    }

    #[test]
    fn test_regex_strategy_stream() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let strategy = RegexStrategy;
        
        let stream = strategy.create_stream(&temp_dir.path().to_path_buf(), r"\w+")?;
        
        // 現在は未実装なので空のストリーム
        let results: Vec<_> = stream.collect();
        assert!(results.is_empty());
        
        Ok(())
    }
}