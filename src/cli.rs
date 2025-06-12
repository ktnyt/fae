use crate::searchers::ContentSearcher;
use crate::search_coordinator::SearchCoordinator;
use crate::display::{CliFormatter, ResultFormatter};
use crate::types::SearchResult;
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use std::path::PathBuf;
use std::env;
use std::collections::HashMap;

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
    
    /// Maximum number of results to show
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
    
    /// Build and display project index
    #[arg(long)]
    pub index: bool,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
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
    
    if cli.verbose {
        println!("Project root: {}", project_root.display());
    }
    
    // インデックス構築モードの確認
    if cli.index {
        return run_index_build(&project_root, cli.verbose);
    }
    
    // クエリがない場合はヘルプを表示
    let Some(raw_query) = &cli.query else {
        let mut cmd = Cli::command();
        cmd.print_help()?;
        return Ok(());
    };
    
    // クエリからモードを検出
    let (mode, clean_query) = detect_mode(raw_query);
    
    if cli.verbose {
        println!("Detected mode: {:?}, Query: '{}'", mode, clean_query);
    }
    
    // モードに応じて処理を分岐
    match mode {
        SearchMode::Content => {
            run_content_search(&project_root, &clean_query, cli.limit, cli.verbose)
        }
        SearchMode::Symbol => {
            run_symbol_search(&project_root, &clean_query, cli.limit, cli.verbose)
        }
        SearchMode::File => {
            run_file_search(&project_root, &clean_query, cli.limit, cli.verbose)
        }
        SearchMode::Regex => {
            run_regex_search(&project_root, &clean_query, cli.limit, cli.verbose)
        }
        SearchMode::Index => {
            // この場合は上で処理済み
            unreachable!()
        }
    }
}

/// コンテンツ検索の実行
fn run_content_search(project_root: &PathBuf, query: &str, limit: usize, verbose: bool) -> Result<()> {
    if verbose {
        println!("Running content search for: '{}'", query);
    }
    
    let searcher = ContentSearcher::new(project_root.clone())
        .context("Failed to create content searcher")?;
    
    let start_time = std::time::Instant::now();
    let results = searcher.search(query, limit)
        .context("Content search failed")?;
    let elapsed = start_time.elapsed();
    
    if verbose {
        println!("Found {} results in {:.2}ms", results.len(), elapsed.as_secs_f64() * 1000.0);
        println!();
    }
    
    if results.is_empty() {
        println!("No matches found for '{}'", query);
        return Ok(());
    }
    
    let formatter = CliFormatter::new(project_root.clone());
    
    // TTY形式の場合はファイル名でグループ化
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        print_tty_format(&formatter, &results, verbose)?;
    } else {
        print_pipe_format(&formatter, &results, verbose)?;
    }
    
    if verbose {
        println!();
        println!("Total: {} matches", results.len());
    }
    
    Ok(())
}

/// シンボル検索の実行
fn run_symbol_search(project_root: &PathBuf, query: &str, limit: usize, verbose: bool) -> Result<()> {
    if verbose {
        println!("Running symbol search for: '{}'", query);
    }
    
    let mut coordinator = SearchCoordinator::new(project_root.clone())
        .context("Failed to create search coordinator")?;
    
    if verbose {
        println!("Building index...");
    }
    
    let start_time = std::time::Instant::now();
    let index_result = coordinator.build_index()
        .context("Failed to build index")?;
    let index_elapsed = start_time.elapsed();
    
    if verbose {
        println!("Index built: {} files, {} symbols in {:.2}ms", 
                 index_result.processed_files, 
                 index_result.total_symbols,
                 index_elapsed.as_secs_f64() * 1000.0);
        println!();
    }
    
    let search_start = std::time::Instant::now();
    let hits = coordinator.search_symbols(query, limit);
    let search_elapsed = search_start.elapsed();
    
    if verbose {
        println!("Found {} symbol matches in {:.2}ms", hits.len(), search_elapsed.as_secs_f64() * 1000.0);
        println!();
    }
    
    if hits.is_empty() {
        println!("No symbol matches found for '{}'", query);
        return Ok(());
    }
    
    // 各シンボルヒットをSearchResultに変換
    let mut results: Vec<crate::types::SearchResult> = Vec::new();
    
    for hit in hits.iter() {
        // シンボル詳細を取得
        let details = coordinator.get_symbol_details(&hit.symbol_name);
        
        for detail in &details {
            // SearchResult形式に変換
            let search_result = crate::types::SearchResult {
                file_path: detail.file_path.clone(),
                line: detail.line,
                column: detail.column,
                display_info: crate::types::DisplayInfo::Symbol {
                    name: hit.symbol_name.clone(),
                    symbol_type: detail.symbol_type.clone(),
                },
                score: hit.score as f64,
            };
            results.push(search_result);
        }
    }
    
    let formatter = CliFormatter::new(project_root.clone());
    
    // TTY形式の場合はファイル名でグループ化
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        print_symbol_tty_format(&formatter, &results, verbose)?;
    } else {
        print_symbol_pipe_format(&formatter, &results, verbose)?;
    }
    
    if verbose {
        println!();
        println!("Total: {} symbol matches", results.len());
    }
    
    Ok(())
}

/// インデックス構築と表示
fn run_index_build(project_root: &PathBuf, verbose: bool) -> Result<()> {
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
    
    if verbose {
        println!();
        println!("Index is ready for symbol searches.");
        println!("Use 'fae \"#<query>\"' to search symbols.");
    }
    
    Ok(())
}

/// ファイル検索の実行
fn run_file_search(_project_root: &PathBuf, query: &str, _limit: usize, verbose: bool) -> Result<()> {
    if verbose {
        println!("Running file search for: '{}'", query);
    }
    
    // TODO: ファイル名検索の実装
    println!("File search not yet implemented. Query: '{}'", query);
    
    Ok(())
}

/// 正規表現検索の実行
fn run_regex_search(_project_root: &PathBuf, query: &str, _limit: usize, verbose: bool) -> Result<()> {
    if verbose {
        println!("Running regex search for: '{}'", query);
    }
    
    // TODO: 正規表現検索の実装
    println!("Regex search not yet implemented. Query: '{}'", query);
    
    Ok(())
}

/// TTY形式でContent Search結果を表示（--group style）
fn print_tty_format(formatter: &CliFormatter, results: &[SearchResult], verbose: bool) -> Result<()> {
    // ファイル名でグループ化
    let mut groups: HashMap<String, Vec<&SearchResult>> = HashMap::new();
    
    for result in results {
        let formatted = formatter.format_result(result);
        let file_name = &formatted.right_part; // TTY形式ではファイル名がright_partに格納
        groups.entry(file_name.clone()).or_insert_with(Vec::new).push(result);
    }
    
    // ファイル名でソートして表示
    let mut sorted_files: Vec<_> = groups.keys().collect();
    sorted_files.sort();
    
    for (i, file_name) in sorted_files.iter().enumerate() {
        if i > 0 {
            println!(); // ファイル間に空行
        }
        
        // ファイル名ヘッダー
        println!("{}", file_name);
        
        // そのファイルの検索結果
        let file_results = &groups[*file_name];
        for result in file_results {
            let formatted = formatter.format_result(result);
            
            if verbose {
                println!("{} (score: {:.2})", formatted.left_part, result.score);
            } else {
                println!("{}", formatted.left_part);
            }
        }
    }
    
    Ok(())
}

/// Pipe形式でContent Search結果を表示（--no-group style）
fn print_pipe_format(formatter: &CliFormatter, results: &[SearchResult], verbose: bool) -> Result<()> {
    for result in results {
        let formatted = formatter.format_result(result);
        
        if verbose {
            println!("{} (score: {:.2})", formatted.left_part, result.score);
        } else {
            println!("{}", formatted.left_part);
        }
    }
    
    Ok(())
}

/// TTY形式でSymbol Search結果を表示（--group style）
fn print_symbol_tty_format(formatter: &CliFormatter, results: &[SearchResult], verbose: bool) -> Result<()> {
    // ファイル名でグループ化
    let mut groups: HashMap<String, Vec<&SearchResult>> = HashMap::new();
    
    for result in results {
        let formatted = formatter.format_result(result);
        let file_name = &formatted.right_part; // TTY形式ではファイル名がright_partに格納
        groups.entry(file_name.clone()).or_insert_with(Vec::new).push(result);
    }
    
    // ファイル名でソートして表示
    let mut sorted_files: Vec<_> = groups.keys().collect();
    sorted_files.sort();
    
    for (i, file_name) in sorted_files.iter().enumerate() {
        if i > 0 {
            println!(); // ファイル間に空行
        }
        
        // ファイル名ヘッダー
        println!("{}", file_name);
        
        // そのファイルのシンボル検索結果
        let file_results = &groups[*file_name];
        for result in file_results {
            let formatted = formatter.format_result(result);
            
            if verbose {
                println!("{} (score: {:.2})", formatted.left_part, result.score);
            } else {
                println!("{}", formatted.left_part);
            }
        }
    }
    
    Ok(())
}

/// Pipe形式でSymbol Search結果を表示（--no-group style）
fn print_symbol_pipe_format(formatter: &CliFormatter, results: &[SearchResult], verbose: bool) -> Result<()> {
    for result in results {
        let formatted = formatter.format_result(result);
        
        if verbose {
            println!("{} (score: {:.2})", formatted.left_part, result.score);
        } else {
            println!("{}", formatted.left_part);
        }
    }
    
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
        let result = run_content_search(&temp_dir.path().to_path_buf(), "testFunction", 10, false);
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_symbol_search_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Symbol search should work
        let result = run_symbol_search(&temp_dir.path().to_path_buf(), "test", 10, false);
        assert!(result.is_ok());
        
        Ok(())
    }

    #[test]
    fn test_index_build_cli() -> Result<()> {
        let temp_dir = create_test_project()?;
        
        // Index build should work
        let result = run_index_build(&temp_dir.path().to_path_buf(), false);
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