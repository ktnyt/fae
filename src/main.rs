use clap::{Parser, ValueEnum};
use sfs_rs::{indexer::TreeSitterIndexer, searcher::FuzzySearcher, types::*, tui::run_tui};
use std::path::PathBuf;

#[derive(Parser, Clone)]
#[command(name = "sfs")]
#[command(about = "Symbol Fuzzy Search - Rust Implementation")]
#[command(version)]
struct Cli {
    /// Search query
    query: Option<String>,
    
    /// Directory to search in
    #[arg(short, long, default_value = ".")]
    directory: PathBuf,
    
    /// Symbol types to include
    #[arg(long, value_delimiter = ',')]
    types: Option<Vec<SymbolTypeArg>>,
    
    /// Maximum number of results
    #[arg(long, default_value = "10")]
    limit: usize,
    
    /// Fuzzy search threshold (0.0 to 1.0)
    #[arg(long, default_value = "0.5")]
    threshold: f64,
    
    /// Exclude files from search
    #[arg(long)]
    no_files: bool,
    
    /// Exclude directories from search  
    #[arg(long)]
    no_dirs: bool,
    
    /// Use TUI (Terminal User Interface) mode
    #[arg(long)]
    tui: bool,
}

#[derive(Clone, ValueEnum)]
enum SymbolTypeArg {
    Function,
    Variable,
    Class,
    Interface,
    Type,
    Enum,
    Constant,
    Method,
    Property,
    Filename,
    Dirname,
}

impl From<SymbolTypeArg> for SymbolType {
    fn from(arg: SymbolTypeArg) -> Self {
        match arg {
            SymbolTypeArg::Function => SymbolType::Function,
            SymbolTypeArg::Variable => SymbolType::Variable,
            SymbolTypeArg::Class => SymbolType::Class,
            SymbolTypeArg::Interface => SymbolType::Interface,
            SymbolTypeArg::Type => SymbolType::Type,
            SymbolTypeArg::Enum => SymbolType::Enum,
            SymbolTypeArg::Constant => SymbolType::Constant,
            SymbolTypeArg::Method => SymbolType::Method,
            SymbolTypeArg::Property => SymbolType::Property,
            SymbolTypeArg::Filename => SymbolType::Filename,
            SymbolTypeArg::Dirname => SymbolType::Dirname,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    if cli.tui {
        // TUI mode
        run_tui(cli.directory).await?;
    } else {
        let query = cli.query.clone();
        match query {
            Some(q) => {
                // CLI search mode
                perform_search(cli, q).await?;
            }
            None => {
                // Interactive mode - fallback to TUI
                println!("🖥️  Starting TUI mode...");
                run_tui(cli.directory).await?;
            }
        }
    }
    
    Ok(())
}

async fn perform_search(cli: Cli, query: String) -> anyhow::Result<()> {
    println!("🔍 Indexing files in {:?}...", cli.directory);
    
    // Initialize indexer
    let mut indexer = TreeSitterIndexer::new();
    indexer.initialize().await?;
    
    // Index directory
    let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string(), "**/*.py".to_string()];
    indexer.index_directory(&cli.directory, &patterns).await?;
    
    let all_symbols = indexer.get_all_symbols();
    println!("📚 Found {} symbols", all_symbols.len());
    
    if all_symbols.is_empty() {
        println!("🤷 No results found");
        return Ok(());
    }
    
    // Create searcher
    let searcher = FuzzySearcher::new(all_symbols);
    
    // Build search options
    let search_options = SearchOptions {
        include_files: if cli.no_files { Some(false) } else { None },
        include_dirs: if cli.no_dirs { Some(false) } else { None },
        types: cli.types.map(|types| types.into_iter().map(Into::into).collect()),
        threshold: Some(cli.threshold),
        limit: Some(cli.limit),
    };
    
    // Perform search
    let results = searcher.search(&query, &search_options);
    
    if results.is_empty() {
        println!("🤷 No results found for '{}'", query);
    } else {
        println!("🎯 Found {} results for '{}':", results.len(), query);
        for result in results {
            let icon = match result.symbol.symbol_type {
                SymbolType::Function => "🔧",
                SymbolType::Variable => "📦",
                SymbolType::Class => "🏗️",
                SymbolType::Interface => "📐",
                SymbolType::Type => "🔖",
                SymbolType::Enum => "📝",
                SymbolType::Constant => "🔒",
                SymbolType::Method => "⚙️",
                SymbolType::Property => "🔹",
                SymbolType::Filename => "📄",
                SymbolType::Dirname => "📁",
            };
            
            println!("{} {} ({}:{}:{})", 
                icon, 
                result.symbol.name,
                result.symbol.file.display(),
                result.symbol.line,
                result.symbol.column
            );
        }
    }
    
    Ok(())
}
