use crate::cli::search_strategy::{SearchStrategy, SearchResultStream};
use crate::searchers::FileSearcher;
use crate::display::{FileHeadingFormatter, FileInlineFormatter, ResultFormatter};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// ファイル検索戦略
/// 
/// ファイル名・ディレクトリ名のファジー検索を行う。
/// gitignoreに対応し、効率的なファイル探索を提供。
pub struct FileStrategy;

impl SearchStrategy for FileStrategy {
    fn name(&self) -> &'static str {
        "file"
    }
    
    fn create_stream(&self, project_root: &PathBuf, query: &str) -> Result<SearchResultStream> {
        let searcher = FileSearcher::new(project_root.clone())
            .context("Failed to create file searcher")?;
        
        let stream = searcher.search_stream(query)
            .context("File search failed")?;
        
        Ok(Box::new(stream))
    }
    
    fn create_formatters(&self, project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
        let heading_formatter = FileHeadingFormatter::new(project_root.clone());
        let inline_formatter = FileInlineFormatter::new(project_root.clone());
        
        (Box::new(heading_formatter), Box::new(inline_formatter))
    }
    
    fn supports_file_grouping(&self) -> bool {
        false // ファイル検索はファイルリストなのでグルーピング不要
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;
    use tempfile::TempDir;
    use std::fs::{File, create_dir};
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // テスト用ファイル・ディレクトリ作成
        let mut test_file = File::create(root.join("main.rs"))?;
        writeln!(test_file, "fn main() {{}}")?;
        
        create_dir(root.join("src"))?;
        let mut lib_file = File::create(root.join("src").join("lib.rs"))?;
        writeln!(lib_file, "// lib.rs")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_file_strategy_basic() {
        let strategy = FileStrategy;
        assert_eq!(strategy.name(), "file");
        assert!(!strategy.supports_file_grouping()); // ファイル検索はグルーピング不要
    }

    #[test]
    fn test_file_strategy_formatters() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = FileStrategy;
        
        let (heading, inline) = strategy.create_formatters(&temp_dir.path().to_path_buf());
        
        // フォーマッターが作成されることを確認
        assert_ne!(heading.as_ref().type_id(), std::any::TypeId::of::<()>());
        assert_ne!(inline.as_ref().type_id(), std::any::TypeId::of::<()>());
        
        Ok(())
    }

    #[test]
    fn test_file_strategy_stream() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = FileStrategy;
        
        let stream = strategy.create_stream(&temp_dir.path().to_path_buf(), "main")?;
        
        // ストリームが作成されることを確認
        let results: Vec<_> = stream.collect();
        
        // "main"にマッチするファイルがあるので、結果が返される
        assert!(!results.is_empty());
        
        Ok(())
    }
}