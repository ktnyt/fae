//! Fast And Elegant (fae) - High-performance code symbol search tool
//!
//! Command-line usage:
//!   fae [query]       - Literal search (fallback: rg → ag → native)
//!   fae #[query]      - Symbol search
//!   fae $[query]      - Variable search
//!   fae @[query]      - Filepath search
//!   fae /[query]      - Regex search (fallback: rg → ag → native)

use fae::cli::create_search_params;
use fae::tui::TuiApp;
use fae::unified_search::UnifiedSearchSystem;
use std::env;

/// CLI application configuration
struct FaeConfig {
    query: String,
    search_path: String,
    max_results: usize,
    timeout_ms: u64,
}

impl Default for FaeConfig {
    fn default() -> Self {
        Self {
            query: String::new(),
            search_path: ".".to_string(),
            max_results: 50,
            timeout_ms: 5000, // Increased timeout for unified system
        }
    }
}

/// Parse command line arguments
fn parse_args() -> Result<Option<FaeConfig>, String> {
    let args: Vec<String> = env::args().collect();

    // If no query provided, launch TUI mode
    if args.len() < 2 {
        return Ok(None);
    }

    let mut config = FaeConfig {
        query: args[1].clone(),
        ..Default::default()
    };

    // Parse additional arguments if needed (future extension)
    for i in 2..args.len() {
        match args[i].as_str() {
            "--path" => {
                if i + 1 < args.len() {
                    config.search_path = args[i + 1].clone();
                }
            }
            "--max-results" => {
                if i + 1 < args.len() {
                    if let Ok(max) = args[i + 1].parse() {
                        config.max_results = max;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Some(config))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments
    let config = match parse_args() {
        Ok(Some(config)) => config,
        Ok(None) => {
            // Launch TUI mode
            let mut app = TuiApp::new(".").await?;
            return app.run().await;
        }
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    // Parse query and determine search mode
    let search_params = create_search_params(&config.query);

    log::info!(
        "Starting search: '{}' (mode: {:?})",
        search_params.query,
        search_params.mode
    );

    // Create unified search system (CLI mode doesn't need file watching)
    let mut search_system = UnifiedSearchSystem::new(&config.search_path, false).await?;

    // Execute search through unified system
    let result_count = search_system
        .search(search_params, config.max_results, config.timeout_ms)
        .await?;

    // Shutdown the system
    search_system.shutdown();

    if result_count == 0 {
        eprintln!("No results found.");
        std::process::exit(1);
    } else {
        log::info!("Search completed with {} results", result_count);
    }

    Ok(())
}
