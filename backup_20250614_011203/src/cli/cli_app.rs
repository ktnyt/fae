use super::{SearchRunner, ContentStrategy, SymbolStrategy, FileStrategy, RegexStrategy};
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::env;
use log::debug;

/// fae - Fast And Elegant code search
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Search query with optional mode prefix:
    /// - No prefix: Content search (default)
    /// - #query: Symbol search
    /// - >query: File search
    /// - /query: Regex search
    pub query: Option<String>,
    
    /// Project directory (defaults to current directory)
    #[arg(short, long)]
    pub directory: Option<PathBuf>,
    
    /// Build and display project index
    #[arg(long)]
    pub index: bool,
    
    /// Force grouped output with file headers (same as rg --heading)
    #[arg(long)]
    pub heading: bool,
    
    /// Show available search backends information
    #[arg(long)]
    pub backends: bool,
    
    /// Start interactive TUI mode
    #[arg(long)]
    pub tui: bool,
}

#[derive(Debug, PartialEq)]
pub enum SearchMode {
    Content,
    Symbol,
    File,
    Regex,
    Index,
}

/// クエリからモードを検出
fn detect_mode(query: &str) -> (SearchMode, String) {
    if query.starts_with('#') {
        (SearchMode::Symbol, query[1..].to_string())
    } else if query.starts_with('>') {
        (SearchMode::File, query[1..].to_string())
    } else if query.starts_with('/') {
        (SearchMode::Regex, query[1..].to_string())
    } else {
        (SearchMode::Content, query.to_string())
    }
}

/// CLI実行エントリーポイント
pub async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    
    // プロジェクトディレクトリの決定
    let project_root = cli.directory
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    debug!("Project root: {}", project_root.display());
    
    // バックエンド情報表示モードの確認
    if cli.backends {
        return show_backend_info(&project_root);
    }
    
    // インデックス構築モードの確認
    if cli.index {
        return run_index_build(&project_root);
    }
    
    // TUIモードの確認
    if cli.tui {
        return run_tui_mode(&project_root).await;
    }
    
    // クエリがない場合はTUIモードを起動
    let Some(raw_query) = &cli.query else {
        return run_tui_mode(&project_root).await;
    };
    
    // クエリからモードを検出
    let (mode, clean_query) = detect_mode(raw_query);
    
    debug!("Detected mode: {:?}, Query: '{}'", mode, clean_query);
    
    // SearchRunnerで統一実行
    let runner = SearchRunner::new(project_root.clone(), cli.heading);
    
    match mode {
        SearchMode::Content => {
            runner.run_with_strategy(&ContentStrategy, &clean_query)
        }
        SearchMode::Symbol => {
            runner.run_with_strategy(&SymbolStrategy::new(), &clean_query)
        }
        SearchMode::File => {
            runner.run_with_strategy(&FileStrategy, &clean_query)
        }
        SearchMode::Regex => {
            runner.run_with_strategy(&RegexStrategy, &clean_query)
        }
        SearchMode::Index => {
            // この場合は上で処理済み
            unreachable!()
        }
    }
}

/// バックエンド情報の表示（従来のまま）
fn show_backend_info(project_root: &PathBuf) -> Result<()> {
    use crate::searchers::EnhancedContentSearcher;
    
    println!("Search Backend Information");
    println!("==========================");
    
    let searcher = EnhancedContentSearcher::new(project_root.clone())
        .context("Failed to create enhanced content searcher")?;
    
    let (primary, available) = searcher.backend_info();
    
    println!("Primary backend: {}", primary);
    println!("Available backends:");
    
    for (i, backend) in available.iter().enumerate() {
        let marker = if i == 0 { "→" } else { " " };
        println!("  {} {} {}", marker, backend, if i == 0 { "(active)" } else { "" });
    }
    
    println!();
    println!("Backend priorities:");
    println!("  1. ripgrep (rg) - fastest, Rust-based");
    println!("  2. ag (the_silver_searcher) - fast, C-based");
    println!("  3. built-in - fallback, always available");
    println!();
    println!("To install external backends:");
    println!("  ripgrep: cargo install ripgrep");
    println!("  ag: brew install the_silver_searcher (macOS)");
    
    Ok(())
}

/// インデックス構築と表示（従来のまま）
fn run_index_build(project_root: &PathBuf) -> Result<()> {
    use crate::search_coordinator::SearchCoordinator;
    
    println!("Building project index...");
    
    let mut coordinator = SearchCoordinator::new(project_root.clone())
        .context("Failed to create search coordinator")?;
    
    let start_time = std::time::Instant::now();
    let result = coordinator.build_index()
        .context("Failed to build index")?;
    let elapsed = start_time.elapsed();
    
    println!("Index build completed!");
    println!("  Files processed: {}", result.processed_files);
    println!("  Symbols extracted: {}", result.total_symbols);
    println!("  Build time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    
    if result.error_files > 0 {
        println!("  Files with errors: {}", result.error_files);
    }
    
    println!();
    println!("Index is ready for symbol searches.");
    println!("Use 'fae \"#<query>\"' to search symbols.");
    
    Ok(())
}

/// TUIモードを実行
async fn run_tui_mode(project_root: &PathBuf) -> Result<()> {
    use crate::workers::{WorkerManager, SimpleTuiWorker, SearchRouterWorker, ContentSearchWorker};
    
    println!("Starting TUI with new worker system...");
    
    // ワーカーマネージャーを作成
    let mut manager = WorkerManager::new();
    
    // TUIワーカーを追加
    let mut tui_worker = SimpleTuiWorker::new("tui".to_string());
    tui_worker.set_message_bus(manager.get_message_bus());
    manager.add_worker(tui_worker).await
        .context("Failed to add TUI worker")?;
    
    // SearchRouterWorkerワーカーを追加
    let mut search_router = SearchRouterWorker::new("search_router".to_string());
    search_router.set_message_bus(manager.get_message_bus());
    manager.add_worker(search_router).await
        .context("Failed to add SearchRouterWorker worker")?;
    
    // ContentSearchWorkerワーカーを追加
    let mut content_searcher = ContentSearchWorker::new(
        "content_searcher".to_string(),
        "search_router".to_string(),
        project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearchWorker: {}", e))?;
    content_searcher.set_message_bus(manager.get_message_bus());
    manager.add_worker(content_searcher).await
        .context("Failed to add ContentSearchWorker worker")?;
    
    println!("All workers initialized. Starting TUI...");
    
    // ワーカーシステムを実行
    // TUIワーカーが終了信号を送るまで待機
    tokio::signal::ctrl_c().await
        .context("Failed to listen for shutdown signal")?;
    
    println!("Shutting down workers...");
    manager.shutdown_all().await
        .context("Failed to shutdown workers")?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // テスト用ファイル作成
        let mut ts_file = File::create(root.join("test.ts"))?;
        writeln!(ts_file, "function testFunction() {{")?;
        writeln!(ts_file, "    console.log('Hello from test');")?;
        writeln!(ts_file, "}}")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_mode_detection() -> Result<()> {
        // Content search (default)
        let (mode, query) = detect_mode("console.log");
        assert_eq!(mode, SearchMode::Content);
        assert_eq!(query, "console.log");
        
        // Symbol search
        let (mode, query) = detect_mode("#handleClick");
        assert_eq!(mode, SearchMode::Symbol);
        assert_eq!(query, "handleClick");
        
        // File search
        let (mode, query) = detect_mode(">main.ts");
        assert_eq!(mode, SearchMode::File);
        assert_eq!(query, "main.ts");
        
        // Regex search
        let (mode, query) = detect_mode("/\\w+Error");
        assert_eq!(mode, SearchMode::Regex);
        assert_eq!(query, "\\w+Error");
        
        Ok(())
    }

    #[test]
    fn test_backend_info() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Backend info should work
        let result = show_backend_info(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_index_build() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Index build should work
        let result = run_index_build(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        
        Ok(())
    }
}