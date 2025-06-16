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
use fae::actors::{
    AgActor, FilepathSearchActor, NativeSearchActor, RipgrepActor, SymbolIndexActor,
    SymbolSearchActor,
};
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

/// Enum for different content search actors
pub enum ContentSearchActor {
    Ripgrep(RipgrepActor),
    Ag(AgActor),
    Native(NativeSearchActor),
}

impl ContentSearchActor {
    /// Shutdown the actor
    pub fn shutdown(&mut self) {
        match self {
            ContentSearchActor::Ripgrep(actor) => actor.shutdown(),
            ContentSearchActor::Ag(actor) => actor.shutdown(),
            ContentSearchActor::Native(actor) => actor.shutdown(),
        }
    }
}

/// Create content search actor with fallback strategy (rg → ag → native)
async fn create_content_search_actor(
    message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    search_path: &str,
) -> ContentSearchActor {
    // Try ripgrep first
    if is_tool_available("rg").await {
        log::info!("Using ripgrep for content search");
        return ContentSearchActor::Ripgrep(RipgrepActor::new_ripgrep_actor(
            message_receiver,
            sender,
            search_path,
        ));
    }

    // Fallback to ag
    if is_tool_available("ag").await {
        log::info!("Using ag for content search");
        return ContentSearchActor::Ag(AgActor::new_ag_actor(
            message_receiver,
            sender,
            search_path,
        ));
    }

    // Final fallback to native search
    log::info!("Using native search for content search");
    ContentSearchActor::Native(NativeSearchActor::new_native_search_actor(
        message_receiver,
        sender,
        search_path,
    ))
}

/// Execute content search with fallback (rg → ag → native)
async fn execute_content_search(
    search_params: SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    // Create appropriate actor based on availability
    let mut actor = create_content_search_actor(actor_rx, external_tx, search_path).await;

    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params),
    );
    actor_tx.send(search_message)?;

    let result_count = collect_search_results(&mut external_rx, max_results, timeout_ms).await;
    actor.shutdown();

    Ok(result_count)
}

/// Execute symbol/variable search
async fn execute_symbol_search(
    search_params: SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    log::info!(
        "Executing symbol/variable search with mode: {:?}",
        search_params.mode
    );

    // Set up message channels
    let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    // Create actors
    let mut symbol_index_actor =
        SymbolIndexActor::new_symbol_index_actor(symbol_index_rx, external_tx.clone(), search_path)
            .map_err(|e| -> Box<dyn std::error::Error> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

    let mut symbol_search_actor =
        SymbolSearchActor::new_symbol_search_actor(symbol_search_rx, external_tx.clone());

    // Start symbol indexing
    let init_message = Message::new("initialize", FaeMessage::ClearResults);
    symbol_index_tx.send(init_message)?;

    // Start message forwarding immediately and wait for indexing to complete
    let mut result_count = 0;
    let mut search_completed = false;
    let mut search_sent = false;
    let mut files_indexed = 0;
    let expected_files = 24; // Approximately expected based on earlier output

    while result_count < max_results && !search_completed {
        match timeout(Duration::from_millis(timeout_ms), external_rx.recv()).await {
            Ok(Some(message)) => {
                match message.method.as_str() {
                    "pushSearchResult" => {
                        if let FaeMessage::PushSearchResult(result) = message.payload {
                            result_count += 1;
                            println!(
                                "{}:{}:{}: {}",
                                result.filename, result.line, result.column, result.content
                            );
                        }
                    }
                    "completeSearch" => {
                        log::debug!("Symbol search completed notification received");
                        search_completed = true;
                    }
                    "clearSymbolIndex" | "pushSymbolIndex" => {
                        // Forward symbol index messages to SymbolSearchActor
                        if let Err(e) = symbol_search_tx.send(message) {
                            log::warn!(
                                "Failed to forward symbol index message to search actor: {}",
                                e
                            );
                        }
                    }
                    "completeSymbolIndex" => {
                        files_indexed += 1;
                        log::debug!("File indexing completed: {}/{}", files_indexed, expected_files);
                        
                        // Forward complete message to SymbolSearchActor
                        if let Err(e) = symbol_search_tx.send(message) {
                            log::warn!("Failed to forward complete symbol index message to search actor: {}", e);
                        }
                        
                        // Send search query once we have enough files indexed
                        if !search_sent && files_indexed >= (expected_files / 2) {
                            log::info!("Sending search query after {} files indexed", files_indexed);
                            let search_message = Message::new(
                                "updateSearchParams",
                                FaeMessage::UpdateSearchParams(search_params.clone()),
                            );
                            if let Err(e) = symbol_search_tx.send(search_message) {
                                log::error!("Failed to send search query: {}", e);
                            } else {
                                search_sent = true;
                            }
                        }
                    }
                    _ => {
                        log::debug!("Received unhandled message: {}", message.method);
                    }
                }
            }
            Ok(None) => break,
            Err(_) => {
                if result_count == 0 && !search_completed {
                    // If search hasn't been sent yet and we've waited, send it anyway
                    if !search_sent {
                        log::warn!("Timeout waiting for indexing, sending search query anyway");
                        let search_message = Message::new(
                            "updateSearchParams",
                            FaeMessage::UpdateSearchParams(search_params.clone()),
                        );
                        if let Err(e) = symbol_search_tx.send(search_message) {
                            log::error!("Failed to send fallback search query: {}", e);
                        } else {
                            search_sent = true;
                            continue; // Give it another chance
                        }
                    }
                    
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

/// Collect search results from external receiver
async fn collect_search_results(
    external_rx: &mut mpsc::UnboundedReceiver<Message<FaeMessage>>,
    max_results: usize,
    timeout_ms: u64,
) -> usize {
    let mut result_count = 0;
    let mut search_completed = false;

    while result_count < max_results && !search_completed {
        match timeout(Duration::from_millis(timeout_ms), external_rx.recv()).await {
            Ok(Some(message)) => {
                match message.method.as_str() {
                    "pushSearchResult" => {
                        if let FaeMessage::PushSearchResult(result) = message.payload {
                            result_count += 1;
                            println!(
                                "{}:{}:{}: {}",
                                result.filename, result.line, result.column, result.content
                            );
                        }
                    }
                    "completeSearch" => {
                        log::debug!("Search completed notification received");
                        search_completed = true;
                    }
                    _ => {
                        // Ignore other message types
                    }
                }
            }
            Ok(None) => break,
            Err(_) => {
                if result_count == 0 && !search_completed {
                    log::debug!("Timeout waiting for search results or completion");
                }
                break;
            }
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

    log::info!(
        "Starting search: '{}' (mode: {:?})",
        search_params.query,
        search_params.mode
    );

    // Execute search based on mode
    let result_count = match search_params.mode {
        SearchMode::Symbol | SearchMode::Variable => {
            execute_symbol_search(
                search_params,
                &config.search_path,
                config.max_results,
                config.timeout_ms,
            )
            .await?
        }
        SearchMode::Filepath => {
            execute_filepath_search(
                search_params,
                &config.search_path,
                config.max_results,
                config.timeout_ms,
            )
            .await?
        }
        SearchMode::Literal | SearchMode::Regexp => {
            execute_content_search(
                search_params,
                &config.search_path,
                config.max_results,
                config.timeout_ms,
            )
            .await?
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

/// Execute filepath search
async fn execute_filepath_search(
    search_params: SearchParams,
    search_path: &str,
    max_results: usize,
    timeout_ms: u64,
) -> Result<usize, Box<dyn std::error::Error>> {
    log::info!("Executing filepath search");

    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    let mut actor =
        FilepathSearchActor::new_filepath_search_actor(actor_rx, external_tx, search_path);

    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_params),
    );
    actor_tx.send(search_message)?;

    let result_count = collect_search_results(&mut external_rx, max_results, timeout_ms).await;
    actor.shutdown();

    Ok(result_count)
}
