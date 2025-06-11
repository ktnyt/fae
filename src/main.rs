use clap::Parser;
use sfs::{run_tui, types::*, searcher::SearchManager, TreeSitterIndexer};
use std::path::Path;
use ignore::WalkBuilder;

#[derive(Parser)]
#[command(name = "sfs")]
#[command(about = "Symbol Fuzzy Search - Fast code symbol search tool")]
#[command(
    long_about = "SFS is a high-performance code search tool that finds symbols (functions, classes, variables, etc.) in your codebase using fuzzy matching."
)]
#[command(version)]
struct Cli {
    /// Search query (if not provided, launches TUI mode)
    query: Option<String>,

    /// Directory to search in (defaults to current directory)
    #[arg(short, long)]
    directory: Option<String>,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Use provided directory or default to current directory
    let directory = cli.directory
        .as_ref()
        .map(|d| Path::new(d))
        .unwrap_or_else(|| Path::new("."));
    let respect_gitignore = true;

    match cli.query {
        Some(query) => {
            // CLI search mode
            perform_search(query, directory, cli.verbose)?;
        }
        None => {
            // TUI mode
            if cli.verbose {
                println!("🖥️  Starting TUI mode...");
            }
            run_tui(directory.to_path_buf(), cli.verbose, respect_gitignore)?;
        }
    }

    Ok(())
}

fn perform_search(query: String, directory: &Path, verbose: bool) -> anyhow::Result<()> {
    if verbose {
        println!("🔍 Searching for '{}' in {}...", query, directory.display());
    }

    // Initialize indexer with default settings
    let mut indexer = TreeSitterIndexer::with_options(verbose, true); // respect_gitignore = true
    indexer.initialize_sync()?;

    // Index files in the specified directory using ignore crate
    let supported_extensions = [
        "ts", "tsx", "js", "jsx", "py", "rs", "go", "java", "cpp", "c", "h", "hpp",
        "cs", "php", "rb", "swift", "kt", "scala", "sh", "bash", "zsh", "fish"
    ];
    
    let walker = WalkBuilder::new(directory)
        .follow_links(false)
        .git_ignore(true)
        .build();
    
    for entry in walker {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if let Some(ext_str) = extension.to_str() {
                        if supported_extensions.contains(&ext_str) {
                            if verbose {
                                println!("📂 Indexing: {}", path.display());
                            }
                            if let Err(e) = indexer.index_file_sync(path) {
                                if verbose {
                                    eprintln!("⚠️  Failed to index {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Get all symbols
    let symbols = indexer.get_all_symbols();

    if verbose {
        println!("📚 Found {} symbols", symbols.len());
    }

    // Initialize searcher
    let searcher = SearchManager::new(symbols);

    // Search with default options
    let search_options = SearchOptions {
        include_files: Some(true),
        include_dirs: Some(true),
        types: None, // All symbol types
        threshold: Some(0.5),
        limit: Some(20), // Reasonable default
    };

    // Perform search using fuzzy search method
    let results = searcher.search_symbols(&query, &search_options);

    if results.is_empty() {
        println!("No results found for '{}'", query);
        return Ok(());
    }

    // Display results
    println!("🎯 Found {} results for '{}':", results.len(), query);
    for result in results {
        let symbol_icon = match result.symbol.symbol_type {
            SymbolType::Function => "🔧",
            SymbolType::Class => "📦",
            SymbolType::Variable => "📊",
            SymbolType::Method => "⚙️",
            SymbolType::Filename => "📄",
            SymbolType::Dirname => "📁",
            _ => "🏷️",
        };

        println!(
            "{} {} ({}:{}:{})",
            symbol_icon,
            result.symbol.name,
            result.symbol.file.display(),
            result.symbol.line,
            result.symbol.column
        );
    }

    Ok(())
}
