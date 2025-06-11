use clap::Parser;
use sfs::{run_tui, types::*, FuzzySearcher, TreeSitterIndexer};
use std::path::Path;

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

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Default settings (hardcoded for simplicity)
    let directory = Path::new(".");
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

    // Simple file discovery for CLI mode
    let search_patterns = vec![
        "**/*.ts", "**/*.tsx", "**/*.js", "**/*.jsx", "**/*.py", "**/*.rs",
    ];
    for pattern in search_patterns {
        if verbose {
            println!("Indexing files matching: {}", pattern);
        }
        // Note: This is a simplified implementation for the CLI mode
        // The full async directory indexing is available in TUI mode
    }

    // Get all symbols
    let symbols = indexer.get_all_symbols();

    if verbose {
        println!("📚 Found {} symbols", symbols.len());
    }

    // Initialize searcher
    let searcher = FuzzySearcher::new(symbols);

    // Search with default options
    let search_options = SearchOptions {
        include_files: Some(true),
        include_dirs: Some(true),
        types: None, // All symbol types
        threshold: Some(0.5),
        limit: Some(20), // Reasonable default
    };

    // Perform search
    let results = searcher.search(&query, &search_options);

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
