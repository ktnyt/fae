use super::search_strategy::SearchStrategy;
use anyhow::Result;
use std::path::PathBuf;
use std::io::{IsTerminal, Write};
use log::{debug, info};

/// 検索実行エンジン
/// 
/// 各種検索戦略を統一的に実行する責任を持つ。
/// CLI/TUI両方で使用可能な汎用実行エンジン。
#[derive(Debug, Clone)]
pub struct SearchRunner {
    project_root: PathBuf,
    heading: bool,
}

impl SearchRunner {
    /// 新しいSearchRunnerを作成
    pub fn new(project_root: PathBuf, heading: bool) -> Self {
        Self {
            project_root,
            heading,
        }
    }
    
    /// Broken pipeを安全にハンドリングして出力
    fn safe_println(text: &str) -> Result<()> {
        match writeln!(std::io::stdout(), "{}", text) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                // Broken pipeは正常な終了として扱う
                std::process::exit(0);
            }
            Err(e) => Err(anyhow::anyhow!("Output error: {}", e)),
        }
    }
    
    /// 空行を安全に出力
    fn safe_println_empty() -> Result<()> {
        match writeln!(std::io::stdout()) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                // Broken pipeは正常な終了として扱う
                std::process::exit(0);
            }
            Err(e) => Err(anyhow::anyhow!("Output error: {}", e)),
        }
    }
    
    /// 指定された戦略で検索を実行
    /// 
    /// # Arguments
    /// * `strategy` - 使用する検索戦略
    /// * `query` - 検索クエリ（プレフィックス除去済み）
    pub fn run_with_strategy<S: SearchStrategy>(&self, strategy: &S, query: &str) -> Result<()> {
        info!("Running {} search for: '{}'", strategy.name(), query);
        
        // 検索前の準備処理
        strategy.prepare(&self.project_root)?;
        
        // メタ情報の取得・表示
        if let Some(meta) = strategy.meta_info(&self.project_root)? {
            debug!("{}", meta);
        }
        
        let start_time = std::time::Instant::now();
        
        // 検索ストリームの作成
        let stream = strategy.create_stream(&self.project_root, query)?;
        
        // フォーマッターの準備
        let (heading_formatter, inline_formatter) = strategy.create_formatters(&self.project_root);
        
        // 出力形式の決定
        let use_tty_format = IsTerminal::is_terminal(&std::io::stdout()) || self.heading;
        let supports_grouping = strategy.supports_file_grouping();
        
        debug!("Output format: {}, File grouping: {}", 
               if use_tty_format { "TTY" } else { "Pipe" },
               if supports_grouping { "enabled" } else { "disabled" });
        
        // 結果処理の実行
        let results_count = if use_tty_format && supports_grouping {
            self.process_with_file_grouping(stream, &*heading_formatter)?
        } else {
            self.process_inline(stream, &*inline_formatter)?
        };
        
        let elapsed = start_time.elapsed();
        
        // 結果サマリーの表示
        self.show_search_summary(strategy.name(), query, results_count, elapsed);
        
        Ok(())
    }
    
    /// ファイルグルーピング付きで結果を処理（TTY形式）
    fn process_with_file_grouping(
        &self,
        stream: Box<dyn Iterator<Item = crate::types::SearchResult>>,
        formatter: &dyn crate::display::ResultFormatter,
    ) -> Result<usize> {
        let mut results_count = 0;
        let mut current_file: Option<PathBuf> = None;
        
        for result in stream {
            // ファイルが変わったらヘッダーを出力
            if current_file.as_ref() != Some(&result.file_path) {
                if current_file.is_some() {
                    Self::safe_println_empty()?; // 前のファイルとの間に空行
                }
                
                let relative_path = result.file_path
                    .strip_prefix(&self.project_root)
                    .unwrap_or(&result.file_path);
                Self::safe_println(&format!("{}:", relative_path.display()))?;
                current_file = Some(result.file_path.clone());
            }
            
            // 結果をフォーマットして出力
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            Self::safe_println(&output)?;
            
            results_count += 1;
        }
        
        Ok(results_count)
    }
    
    /// インライン形式で結果を処理（Pipe形式）
    fn process_inline(
        &self,
        stream: Box<dyn Iterator<Item = crate::types::SearchResult>>,
        formatter: &dyn crate::display::ResultFormatter,
    ) -> Result<usize> {
        let mut results_count = 0;
        
        for result in stream {
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            Self::safe_println(&output)?;
            
            results_count += 1;
        }
        
        Ok(results_count)
    }
    
    /// 検索結果のサマリーを表示
    fn show_search_summary(&self, search_type: &str, query: &str, count: usize, elapsed: std::time::Duration) {
        if count == 0 {
            let _ = Self::safe_println(&format!("No {} matches found for '{}'", search_type, query));
        } else {
            info!("Found {} {} matches in {:.2}ms", 
                 count, search_type, elapsed.as_secs_f64() * 1000.0);
        }
    }
    
    /// TUI用: 検索結果をVecとして収集
    /// 
    /// CLI版と異なり、結果を出力せずにVecとして返す。
    /// TUIが結果をメモリに保持して表示・ナビゲーションする用途。
    /// 
    /// # Arguments
    /// * `strategy` - 使用する検索戦略
    /// * `query` - 検索クエリ（プレフィックス除去済み）
    pub fn collect_results_with_strategy<S: SearchStrategy>(
        &self,
        strategy: &S,
        query: &str,
    ) -> Result<Vec<crate::types::SearchResult>> {
        use crate::types::SearchResult;
        
        info!("Collecting {} search results for: '{}'", strategy.name(), query);
        
        // 検索前の準備処理
        strategy.prepare(&self.project_root)?;
        
        // メタ情報をログに記録
        if let Some(meta) = strategy.meta_info(&self.project_root)? {
            debug!("{}", meta);
        }
        
        let start_time = std::time::Instant::now();
        
        // 検索ストリームの作成と結果収集
        let stream = strategy.create_stream(&self.project_root, query)?;
        let raw_results: Vec<SearchResult> = stream.collect();
        
        // 重複除去を適用
        let results = SearchResult::deduplicate(raw_results);
        
        let elapsed = start_time.elapsed();
        let count = results.len();
        
        // ログレベルでサマリー記録
        if count == 0 {
            debug!("No {} matches found for '{}'", strategy.name(), query);
        } else {
            info!("Collected {} {} matches in {:.2}ms", 
                 count, strategy.name(), elapsed.as_secs_f64() * 1000.0);
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::search_strategy::{SearchStrategy, SearchResultStream};
    use crate::types::SearchResult;
    use crate::display::ResultFormatter;
    use tempfile::TempDir;

    // テスト用のモック戦略
    #[allow(dead_code)]
    struct TestStrategy {
        results: Vec<SearchResult>,
    }
    
    impl TestStrategy {
        #[allow(dead_code)]
        fn new(results: Vec<SearchResult>) -> Self {
            Self { results }
        }
    }
    
    impl SearchStrategy for TestStrategy {
        fn name(&self) -> &'static str {
            "test"
        }
        
        fn create_stream(&self, _project_root: &PathBuf, _query: &str) -> Result<SearchResultStream> {
            Ok(Box::new(self.results.clone().into_iter()))
        }
        
        fn create_formatters(&self, _project_root: &PathBuf) -> (Box<dyn ResultFormatter>, Box<dyn ResultFormatter>) {
            // TODO: テスト用フォーマッターを実装
            todo!("Test formatters not implemented yet")
        }
        
        fn supports_file_grouping(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_search_runner_creation() {
        let temp_dir = TempDir::new().unwrap();
        let runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        assert_eq!(runner.project_root, temp_dir.path());
        assert!(!runner.heading);
    }

    #[test]
    fn test_search_runner_with_heading() {
        let temp_dir = TempDir::new().unwrap();
        let runner = SearchRunner::new(temp_dir.path().to_path_buf(), true);
        
        assert!(runner.heading);
    }
}