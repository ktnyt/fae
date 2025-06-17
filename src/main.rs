//! Fast And Elegant (fae) - High-performance code symbol search tool
//!
//! Command-line usage:
//!   fae [query]       - Literal search (fallback: rg → ag → native)
//!   fae #[query]      - Symbol search
//!   fae $[query]      - Variable search
//!   fae @[query]      - Filepath search
//!   fae /[query]      - Regex search (fallback: rg → ag → native)

use fae::actors::messages::FaeMessage;
use fae::cli::create_search_params;
use fae::core::Message;
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
            timeout_ms: 15000, // Increased timeout for symbol indexing
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
            let mut app = TuiApp::new(".").await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            return app.run().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
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

    // Create control channels for external communication
    let (control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (result_sender, mut result_receiver) = tokio::sync::mpsc::unbounded_channel();

    // Create unified search system (CLI mode doesn't need file watching)
    // Pass search mode for optimization (skip symbol actors for non-symbol searches)
    let mut search_system = UnifiedSearchSystem::new_with_mode(
        &config.search_path,
        false,
        result_sender,
        control_receiver,
        Some(search_params.mode.clone()),
    )
    .await?;

    // Initialize symbol indexing
    let init_message = Message::new("initialize", FaeMessage::ClearResults); // Dummy payload for initialize
    if let Err(e) = control_sender.send(init_message) {
        eprintln!("Failed to send initialize message: {}", e);
        std::process::exit(1);
    }

    // Send search request
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params),
    );
    if let Err(e) = control_sender.send(search_message) {
        eprintln!("Failed to send search message: {}", e);
        std::process::exit(1);
    }

    // Wait for search completion or timeout
    let mut result_count = 0;
    let timeout = tokio::time::Duration::from_millis(config.timeout_ms);

    match tokio::time::timeout(timeout, async {
        while let Some(message) = result_receiver.recv().await {
            match &message.payload {
                FaeMessage::NotifySearchReport {
                    result_count: _count,
                } => {
                    // Return the actual number of results we printed, not the total found
                    log::debug!("Search completed, printed {} results", result_count);
                    return result_count;
                }
                FaeMessage::PushSearchResult(result) => {
                    // Print result immediately for CLI mode
                    println!(
                        "{}:{} - {}",
                        result.filename,
                        result.line,
                        result.content.trim()
                    );
                    result_count += 1;
                    // Note: Continue processing until NotifySearchReport for graceful shutdown
                    // ResultHandlerActor will automatically trigger completion when max_results is reached
                }
                _ => {}
            }
        }
        result_count
    })
    .await
    {
        Ok(count) => result_count = count,
        Err(_) => {
            eprintln!("Search timed out after {}ms", config.timeout_ms);
        }
    }

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
