//! Fast And Elegant (fae) - High-performance code symbol search tool
//!
//! Command-line usage:
//!   fae [query]       - Literal search (fallback: rg → ag → native)
//!   fae #[query]      - Symbol search
//!   fae $[query]      - Variable search
//!   fae @[query]      - Filepath search
//!   fae /[query]      - Regex search (fallback: rg → ag → native)

use fae::actors::messages::FaeMessage;
use fae::actors::types::{SearchMode, SearchParams};
use fae::actors::{AgActor, NativeSearchActor, RipgrepActor, SymbolIndexActor, SymbolSearchActor};
use fae::cli::create_search_params;
use fae::core::Message;
use std::env;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

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
            timeout_ms: 3000,
        }
    }
}

/// Parse command line arguments
fn parse_args() -> Result<FaeConfig, String> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        return Err(format!(
            "Usage: {} [query]\n\n\
            Search modes:\n\
              [query]    - Literal search\n\
              #[query]   - Symbol search\n\
              $[query]   - Variable search\n\
              @[query]   - Filepath search\n\
              /[query]   - Regex search",
            args[0]
        ));
    }
    
    let mut config = FaeConfig::default();
    config.query = args[1].clone();
    
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
    
    Ok(config)
}

/// Check if external tool is available
async fn is_tool_available(tool: &str) -> bool {
    tokio::process::Command::new(tool)
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Execute content search with fallback (rg → ag → native)
async fn execute_content_search(
    search_params: SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Try ripgrep first
    if is_tool_available("rg").await {
        log::info!("Using ripgrep for content search");
        if let Ok(count) = execute_ripgrep_search(&search_params, search_path, max_results, timeout_ms).await {
            return Ok(count);
        }
        log::warn!("Ripgrep search failed, falling back to ag");
    }
    
    // Fallback to ag
    if is_tool_available("ag").await {
        log::info!("Using ag for content search");
        if let Ok(count) = execute_ag_search(&search_params, search_path, max_results, timeout_ms).await {
            return Ok(count);
        }
        log::warn!("Ag search failed, falling back to native");
    }
    
    // Final fallback to native search
    log::info!("Using native search");
    execute_native_search(&search_params, search_path, max_results, timeout_ms).await
}

/// Execute symbol/variable search
async fn execute_symbol_search(
    search_params: SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    log::info!("Executing symbol/variable search with mode: {:?}", search_params.mode);
    
    // Set up message channels
    let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    
    // Create actors
    let mut symbol_index_actor = SymbolIndexActor::new_symbol_index_actor(
        symbol_index_rx,
        external_tx.clone(),
        search_path,
    ).map_err(|e| -> Box<dyn std::error::Error> { Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())) })?;
    
    let mut symbol_search_actor = SymbolSearchActor::new_symbol_search_actor(
        symbol_search_rx,
        external_tx.clone(),
    );
    
    // Start symbol indexing
    let init_message = Message::new("initialize", FaeMessage::ClearResults);
    symbol_index_tx.send(init_message)?;
    
    // Wait a bit for initial indexing
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Send search query
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params),
    );
    symbol_search_tx.send(search_message)?;
    
    // Collect results
    let mut result_count = 0;
    while result_count < max_results {
        match timeout(Duration::from_millis(timeout_ms), external_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        result_count += 1;
                        println!(
                            "{}:{}:{}: {}",
                            result.filename,
                            result.line,
                            result.column,
                            result.content
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(_) => {
                if result_count == 0 {
                    // Wait a bit more for symbol indexing to complete
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                } else {
                    break;
                }
            }
        }
    }
    
    // Clean up
    symbol_index_actor.shutdown();
    symbol_search_actor.shutdown();
    
    Ok(result_count)
}

/// Execute ripgrep search
async fn execute_ripgrep_search(
    search_params: &SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    
    let mut actor = RipgrepActor::new_ripgrep_actor(actor_rx, external_tx, search_path);
    
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params.clone()),
    );
    actor_tx.send(search_message)?;
    
    let result_count = collect_search_results(&mut external_rx, max_results, timeout_ms).await;
    actor.shutdown();
    
    Ok(result_count)
}

/// Execute ag search
async fn execute_ag_search(
    search_params: &SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    
    let mut actor = AgActor::new_ag_actor(actor_rx, external_tx, search_path);
    
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params.clone()),
    );
    actor_tx.send(search_message)?;
    
    let result_count = collect_search_results(&mut external_rx, max_results, timeout_ms).await;
    actor.shutdown();
    
    Ok(result_count)
}

/// Execute native search
async fn execute_native_search(
    search_params: &SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    
    let mut actor = NativeSearchActor::new_native_search_actor(actor_rx, external_tx, search_path);
    
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params.clone()),
    );
    actor_tx.send(search_message)?;
    
    let result_count = collect_search_results(&mut external_rx, max_results, timeout_ms).await;
    actor.shutdown();
    
    Ok(result_count)
}

/// Collect search results from external receiver
async fn collect_search_results(
    external_rx: &mut mpsc::UnboundedReceiver<Message<FaeMessage>>,
    max_results: usize,
    timeout_ms: u64,
) -> usize {
    let mut result_count = 0;
    
    while result_count < max_results {
        match timeout(Duration::from_millis(timeout_ms), external_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        result_count += 1;
                        println!(
                            "{}:{}:{}: {}",
                            result.filename,
                            result.line,
                            result.column,
                            result.content
                        );
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }
    
    result_count
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();
    
    // Parse command line arguments
    let config = match parse_args() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };
    
    // Parse query and determine search mode
    let search_params = create_search_params(&config.query);
    
    log::info!("Starting search: '{}' (mode: {:?})", search_params.query, search_params.mode);
    
    // Execute search based on mode
    let result_count = match search_params.mode {
        SearchMode::Symbol | SearchMode::Variable => {
            execute_symbol_search(
                search_params,
                &config.search_path,
                config.max_results,
                config.timeout_ms,
            ).await?
        }
        SearchMode::Literal | SearchMode::Regexp | SearchMode::Filepath => {
            execute_content_search(
                search_params,
                &config.search_path,
                config.max_results,
                config.timeout_ms,
            ).await?
        }
    };
    
    if result_count == 0 {
        eprintln!("No results found.");
        std::process::exit(1);
    } else {
        log::info!("Search completed with {} results", result_count);
    }
    
    Ok(())
}