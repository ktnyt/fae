use crate::searchers::{EnhancedContentSearcher, FileSearcher};
use crate::search_coordinator::SearchCoordinator;
use crate::display::{
    ContentHeadingFormatter, ContentInlineFormatter,
    SymbolHeadingFormatter, SymbolInlineFormatter,
    FileHeadingFormatter, FileInlineFormatter,
    ResultFormatter
};
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use std::path::PathBuf;
use std::env;
use log::{debug, info};

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
pub fn run_cli() -> Result<()> {
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
    
    // クエリがない場合はヘルプを表示
    let Some(raw_query) = &cli.query else {
        let mut cmd = Cli::command();
        cmd.print_help()?;
        return Ok(());
    };
    
    // クエリからモードを検出
    let (mode, clean_query) = detect_mode(raw_query);
    
    debug!("Detected mode: {:?}, Query: '{}'", mode, clean_query);
    
    // モードに応じて処理を分岐
    match mode {
        SearchMode::Content => {
            run_content_search(&project_root, &clean_query, cli.heading)
        }
        SearchMode::Symbol => {
            run_symbol_search(&project_root, &clean_query, cli.heading)
        }
        SearchMode::File => {
            run_file_search(&project_root, &clean_query, cli.heading)
        }
        SearchMode::Regex => {
            run_regex_search(&project_root, &clean_query)
        }
        SearchMode::Index => {
            // この場合は上で処理済み
            unreachable!()
        }
    }
}

/// コンテンツ検索の実行
fn run_content_search(project_root: &PathBuf, query: &str, heading: bool) -> Result<()> {
    info!("Running content search for: '{}'", query);
    
    let searcher = EnhancedContentSearcher::new(project_root.clone())
        .context("Failed to create enhanced content searcher")?;
    
    let (primary, available) = searcher.backend_info();
    debug!("Using backend: {} (available: {})", primary, available.join(", "));
    
    let start_time = std::time::Instant::now();
    let stream = searcher.search_stream(query)
        .context("Content search failed")?;
    
    // 特化フォーマッターの準備
    let use_tty_format = std::io::IsTerminal::is_terminal(&std::io::stdout()) || heading;
    
    let mut results_count = 0;
    let mut current_file: Option<PathBuf> = None;
    
    // ストリーミング結果の処理
    for result in stream {
        
        // TTY形式の場合、ファイルが変わったらヘッダーを出力
        if use_tty_format {
            if current_file.as_ref() != Some(&result.file_path) {
                if current_file.is_some() {
                    println!(); // 前のファイルとの間に空行
                }
                let relative_path = result.file_path
                    .strip_prefix(project_root)
                    .unwrap_or(&result.file_path);
                println!("{}:", relative_path.display());
                current_file = Some(result.file_path.clone());
            }
            
            let formatter = ContentHeadingFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            
            println!("{}", output);
        } else {
            // Pipe形式の場合
            let formatter = ContentInlineFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            
            println!("{}", output);
        }
        
        results_count += 1;
    }
    
    let elapsed = start_time.elapsed();
    
    if results_count == 0 {
        println!("No matches found for '{}'", query);
    } else {
        info!("Found {} results in {:.2}ms", results_count, elapsed.as_secs_f64() * 1000.0);
    }
    
    Ok(())
}

/// シンボル検索の実行
fn run_symbol_search(project_root: &PathBuf, query: &str, heading: bool) -> Result<()> {
    info!("Running symbol search for: '{}'", query);
    
    let mut coordinator = SearchCoordinator::new(project_root.clone())
        .context("Failed to create search coordinator")?;
    
    info!("Building index...");
    
    let start_time = std::time::Instant::now();
    let index_result = coordinator.build_index()
        .context("Failed to build index")?;
    let index_elapsed = start_time.elapsed();
    
    info!("Index built: {} files, {} symbols in {:.2}ms", 
         index_result.processed_files, 
         index_result.total_symbols,
         index_elapsed.as_secs_f64() * 1000.0);
    
    let _search_start = std::time::Instant::now();
    let stream = coordinator.search_symbols_stream(query)
        .context("Symbol search failed")?;
    
    // 特化フォーマッターの準備
    let use_tty_format = std::io::IsTerminal::is_terminal(&std::io::stdout()) || heading;
    
    let mut results_count = 0;
    let mut current_file: Option<PathBuf> = None;
    
    // ストリーミング結果の処理
    for result in stream {
        
        // TTY形式の場合、ファイルが変わったらヘッダーを出力
        if use_tty_format {
            if current_file.as_ref() != Some(&result.file_path) {
                if current_file.is_some() {
                    println!(); // 前のファイルとの間に空行
                }
                let relative_path = result.file_path
                    .strip_prefix(project_root)
                    .unwrap_or(&result.file_path);
                println!("{}:", relative_path.display());
                current_file = Some(result.file_path.clone());
            }
            
            let formatter = SymbolHeadingFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            
            println!("{}", output);
        } else {
            // Pipe形式の場合
            let formatter = SymbolInlineFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            
            println!("{}", output);
        }
        
        results_count += 1;
    }
    
    let total_elapsed = start_time.elapsed();
    
    if results_count == 0 {
        println!("No symbol matches found for '{}'", query);
    } else {
        info!("Found {} symbol matches in {:.2}ms total", 
             results_count, total_elapsed.as_secs_f64() * 1000.0);
    }
    
    Ok(())
}

/// インデックス構築と表示
fn run_index_build(project_root: &PathBuf) -> Result<()> {
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

/// ファイル検索の実行
fn run_file_search(project_root: &PathBuf, query: &str, heading: bool) -> Result<()> {
    info!("Running file search for: '{}'", query);
    
    let searcher = FileSearcher::new(project_root.clone())
        .context("Failed to create file searcher")?;
    
    let start_time = std::time::Instant::now();
    let stream = searcher.search_stream(query)
        .context("File search failed")?;
    
    let mut results_count = 0;
    
    for result in stream {
        let _relative_path = result.file_path.strip_prefix(project_root)
            .unwrap_or(&result.file_path)
            .to_path_buf();
        
        if heading {
            // TTY形式の場合（実際はファイル検索ではファイルごとのグルーピングは意味がない）
            let formatter = FileHeadingFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            println!("{}", output);
        } else {
            // Pipe形式の場合
            let formatter = FileInlineFormatter::new(project_root.clone());
            let formatted = formatter.format_result(&result);
            let output = formatter.to_colored_string(&formatted);
            println!("{}", output);
        }
        
        results_count += 1;
    }
    
    let elapsed = start_time.elapsed();
    
    if results_count == 0 {
        println!("No files found matching '{}'", query);
    } else {
        info!("Found {} files in {:.2}ms", results_count, elapsed.as_secs_f64() * 1000.0);
    }
    
    Ok(())
}

/// 正規表現検索の実行
fn run_regex_search(_project_root: &PathBuf, query: &str) -> Result<()> {
    info!("Running regex search for: '{}'", query);
    
    // TODO: 正規表現検索の実装
    println!("Regex search not yet implemented. Query: '{}'", query);
    
    Ok(())
}

/// バックエンド情報の表示
fn show_backend_info(project_root: &PathBuf) -> Result<()> {
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
    fn test_content_search_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Content search should find the function
        let result = run_content_search(&temp_dir.path().to_path_buf(), "testFunction", false);
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_backend_info_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Backend info should work
        let result = show_backend_info(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_symbol_search_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Symbol search should work
        let result = run_symbol_search(&temp_dir.path().to_path_buf(), "test", false);
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_index_build_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Index build should work
        let result = run_index_build(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
        
        Ok(())
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
}