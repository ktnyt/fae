use clap::{Parser, ValueEnum};
use sfs::{indexer::TreeSitterIndexer, searcher::FuzzySearcher, types::*, run_tui_with_watch};
use std::path::PathBuf;

#[derive(Parser, Clone)]
#[command(name = "sfs")]
#[command(about = "Symbol Fuzzy Search - Fast code symbol search tool for developers")]
#[command(long_about = "SFS (Symbol Fuzzy Search) is a high-performance code search tool that indexes\nand searches symbols (functions, classes, variables, etc.) across your codebase.\n\nBy default, SFS respects .gitignore files and excludes ignored files from search.\nUse --include-ignored to search all files regardless of .gitignore rules.\n\nSupported languages: TypeScript, JavaScript, Python, PHP, Ruby, Go, Rust, Java, C, C++, C#, Scala, Perl (via regex)\nAll file types are searchable by filename and directory name.")]
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
    
    /// Enable verbose output (detailed progress information)
    #[arg(short, long)]
    verbose: bool,
    
    /// Include files normally ignored by .gitignore
    /// 
    /// By default, SFS respects .gitignore files and excludes ignored files from search.
    /// Use this flag to search all files in the directory regardless of .gitignore rules.
    #[arg(long)]
    include_ignored: bool,
    
    /// Enable real-time file watching and automatic index updates (default behavior)
    #[arg(long)]
    watch: bool,
    
    /// Disable real-time file watching
    #[arg(long)]
    no_watch: bool,
    
    /// Disable index cache loading and saving (force full re-index)
    #[arg(long)]
    no_cache: bool,
    
    /// Clear existing cache and rebuild from scratch
    #[arg(long)]
    clear_cache: bool,
    
    /// Display cache statistics and status information
    #[arg(long)]
    cache_info: bool,
    
    /// Enable memory-efficient cache with memory limit in MB (for large projects)
    #[arg(long)]
    memory_efficient_cache: Option<usize>,
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
    
    // Handle cache operations
    if cli.cache_info {
        let mut indexer = TreeSitterIndexer::with_options(cli.verbose, !cli.include_ignored);
        match indexer.load_cache(&cli.directory) {
            Ok(stats) => {
                println!("📊 Cache Statistics:");
                println!("  Total files: {}", stats.total_files);
                println!("  Total symbols: {}", stats.total_symbols);
                println!("  Cache created: {}", stats.cache_created);
                println!("  SFS version: {}", stats.sfs_version);
                
                // Check for both compressed and uncompressed cache files
                let compressed_cache_path = cli.directory.join(".sfscache.gz");
                let uncompressed_cache_path = cli.directory.join(".sfscache");
                
                if compressed_cache_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&compressed_cache_path) {
                        println!("  Cache file size: {} bytes (compressed)", metadata.len());
                    }
                    println!("  Cache location: {} (compressed)", compressed_cache_path.display());
                } else if uncompressed_cache_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&uncompressed_cache_path) {
                        println!("  Cache file size: {} bytes (uncompressed)", metadata.len());
                    }
                    println!("  Cache location: {} (uncompressed)", uncompressed_cache_path.display());
                } else {
                    println!("  No cache file found");
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to load cache info: {}", e);
            }
        }
        return Ok(());
    }
    
    if cli.clear_cache {
        let indexer = TreeSitterIndexer::with_options(cli.verbose, !cli.include_ignored);
        match indexer.delete_cache_file(&cli.directory) {
            Ok(()) => {
                println!("🗑️  Cache cleared successfully");
            }
            Err(e) => {
                eprintln!("❌ Failed to clear cache: {}", e);
            }
        }
        return Ok(());
    }
    
    // Determine watch mode: --no-watch disables, --watch explicitly enables, default is enabled
    let watch_enabled = if cli.watch && cli.no_watch {
        // If both flags are provided, show warning and default to enabled
        eprintln!("⚠️  Warning: Both --watch and --no-watch flags provided. Defaulting to watch enabled.");
        true
    } else if cli.no_watch {
        false
    } else {
        true // Default behavior: watching enabled
    };
    
    if cli.tui {
        // TUI mode with optional file watching
        run_tui_with_watch(cli.directory, cli.verbose, !cli.include_ignored, watch_enabled).await?;
    } else {
        let query = cli.query.clone();
        match query {
            Some(q) => {
                // CLI search mode
                perform_search(cli, q).await?;
            }
            None => {
                // Interactive mode - fallback to TUI with optional file watching
                if cli.verbose {
                    if watch_enabled {
                        println!("🖥️  Starting TUI mode with file watching...");
                    } else {
                        println!("🖥️  Starting TUI mode...");
                    }
                }
                run_tui_with_watch(cli.directory, cli.verbose, !cli.include_ignored, watch_enabled).await?;
            }
        }
    }
    
    Ok(())
}

async fn perform_search(cli: Cli, query: String) -> anyhow::Result<()> {
    if cli.verbose {
        println!("🔍 Indexing files in {:?}...", cli.directory);
    }
    
    // Initialize indexer
    let mut indexer = TreeSitterIndexer::with_options(cli.verbose, !cli.include_ignored);
    
    // Configure memory-efficient cache if specified
    if let Some(memory_limit_mb) = cli.memory_efficient_cache {
        indexer.enable_memory_efficient_cache(cli.directory.clone(), memory_limit_mb);
        if cli.verbose {
            println!("🧠 Memory-efficient cache enabled ({}MB limit)", memory_limit_mb);
        }
    }
    
    // Configure cache based on CLI flags
    if cli.no_cache {
        indexer.set_cache_enabled(false);
        if cli.verbose {
            println!("📦 Cache disabled");
        }
    } else {
        // Load existing cache if available
        match indexer.load_cache(&cli.directory) {
            Ok(stats) => {
                if cli.verbose && stats.total_files > 0 {
                    println!("📦 Loaded cache: {} files, {} symbols", stats.total_files, stats.total_symbols);
                }
            }
            Err(e) => {
                if cli.verbose {
                    println!("📦 No cache available: {}", e);
                }
            }
        }
    }
    
    indexer.initialize().await?;
    
    // Index directory - now supports all file types
    let patterns = vec!["**/*".to_string()];
    indexer.index_directory(&cli.directory, &patterns).await?;
    
    // Save cache if enabled
    if !cli.no_cache {
        if let Err(e) = indexer.save_cache(&cli.directory) {
            if cli.verbose {
                eprintln!("⚠️ Failed to save cache: {}", e);
            }
        } else if cli.verbose {
            println!("💾 Cache saved");
        }
    }
    
    let all_symbols = indexer.get_all_symbols();
    if cli.verbose {
        println!("📚 Found {} symbols", all_symbols.len());
    }
    
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
