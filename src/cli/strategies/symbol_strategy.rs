use crate::cli::search_strategy::{SearchStrategy, SearchResultStream};
use crate::search_coordinator::SearchCoordinator;
use crate::display::{SymbolHeadingFormatter, SymbolInlineFormatter, ResultFormatter};
use anyhow::{Context, Result};
use std::path::PathBuf;
use log::info;

/// シンボル検索戦略
/// 
/// Tree-sitterを使用してソースコードからシンボル（関数、クラス等）を抽出し、
/// ファジー検索を行う。検索前にインデックス構築が必要。
pub struct SymbolStrategy;

impl SearchStrategy for SymbolStrategy {
    fn name(&self) -> &'static str {
        "symbol"
    }
    
    fn create_stream(&self, project_root: &PathBuf, query: &str) -> Result<SearchResultStream> {
        let coordinator = SearchCoordinator::new(project_root.clone())
            .context("Failed to create search coordinator")?;
        
        let stream = coordinator.search_symbols_stream(query)
            .context("Symbol search failed")?;
        
        Ok(Box::new(stream))
    }
    
    fn create_formatters(&self, project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
        let heading_formatter = SymbolHeadingFormatter::new(project_root.clone());
        let inline_formatter = SymbolInlineFormatter::new(project_root.clone());
        
        (Box::new(heading_formatter), Box::new(inline_formatter))
    }
    
    fn supports_file_grouping(&self) -> bool {
        true // シンボル検索はファイルごとのグルーピングをサポート
    }
    
    fn prepare(&self, project_root: &PathBuf) -> Result<()> {
        // インデックス構築
        info!("Building index...");
        
        let mut coordinator = SearchCoordinator::new(project_root.clone())
            .context("Failed to create search coordinator")?;
        
        let start_time = std::time::Instant::now();
        let index_result = coordinator.build_index()
            .context("Failed to build index")?;
        let elapsed = start_time.elapsed();
        
        info!("Index built: {} files, {} symbols in {:.2}ms", 
             index_result.processed_files, 
             index_result.total_symbols,
             elapsed.as_secs_f64() * 1000.0);
        
        Ok(())
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

        // テスト用TypeScriptファイル作成
        let mut ts_file = File::create(root.join("test.ts"))?;
        writeln!(ts_file, "function testFunction() {{")?;
        writeln!(ts_file, "    console.log('Hello from test');")?;
        writeln!(ts_file, "}}")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_symbol_strategy_basic() {
        let strategy = SymbolStrategy;
        assert_eq!(strategy.name(), "symbol");
        assert!(strategy.supports_file_grouping());
    }

    #[test]
    fn test_symbol_strategy_formatters() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = SymbolStrategy;
        
        let (heading, inline) = strategy.create_formatters(&temp_dir.path().to_path_buf());
        
        // フォーマッターが作成されることを確認
        assert_ne!(heading.as_ref().type_id(), std::any::TypeId::of::<()>());
        assert_ne!(inline.as_ref().type_id(), std::any::TypeId::of::<()>());
        
        Ok(())
    }

    #[test]
    fn test_symbol_strategy_prepare() -> Result<()> {
        let temp_dir = create_test_project()?;
        let strategy = SymbolStrategy;
        
        // インデックス構築の準備処理が正常に実行されることを確認
        let result = strategy.prepare(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        
        Ok(())
    }
}