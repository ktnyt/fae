//! Symbol search actor for fuzzy symbol searching
//!
//! This actor maintains a symbol index and provides fuzzy search functionality.
//! It receives symbol data from SymbolIndexActor and responds to search queries
//! by finding matching symbols and sending results.

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams, SearchResult, Symbol};
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Symbol search handler that maintains symbol index and provides fuzzy search
pub struct SymbolSearchHandler {
    /// Symbol index: filepath -> list of symbols
    symbol_index: HashMap<String, Vec<Symbol>>,
    /// Fuzzy matcher for symbol search
    fuzzy_matcher: SkimMatcherV2,
    /// Current search parameters
    current_search: Option<SearchParams>,
    /// Whether initial indexing has been completed
    initial_indexing_complete: bool,
}

impl Default for SymbolSearchHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolSearchHandler {
    /// Create a new SymbolSearchHandler
    pub fn new() -> Self {
        Self {
            symbol_index: HashMap::new(),
            fuzzy_matcher: SkimMatcherV2::default(),
            current_search: None,
            initial_indexing_complete: false,
        }
    }

    /// Clear all symbols for a specific file
    fn clear_file_symbols(&mut self, filepath: &str) {
        self.symbol_index.remove(filepath);
        log::trace!("Cleared symbols for file: {}", filepath);
    }

    /// Add a symbol to the index
    fn add_symbol(&mut self, symbol: Symbol) {
        let filepath = symbol.filepath.clone();
        log::trace!(
            "Adding symbol '{}' (type: {:?}) to index for file: {}",
            symbol.content,
            symbol.symbol_type,
            filepath
        );

        self.symbol_index
            .entry(filepath.clone())
            .or_default()
            .push(symbol);

        let (file_count, symbol_count) = self.get_index_stats();
        log::trace!(
            "Index now contains {} files with {} total symbols",
            file_count,
            symbol_count
        );
    }

    /// Perform fuzzy search on symbols
    async fn perform_search(
        &self,
        search_params: &SearchParams,
        controller: &ActorController<FaeMessage>,
    ) {
        // Only perform search for Symbol or Variable mode
        if !matches!(
            search_params.mode,
            SearchMode::Symbol | SearchMode::Variable
        ) {
            log::debug!(
                "Ignoring search for non-symbol/variable mode: {:?}",
                search_params.mode
            );
            return;
        }

        let query = &search_params.query;
        if query.is_empty() {
            log::debug!("Empty query, skipping search");
            return;
        }

        // Wait for initial indexing to complete before performing search
        if !self.initial_indexing_complete {
            log::info!(
                "Initial indexing not complete, skipping search for query: '{}' (mode: {:?})",
                query,
                search_params.mode
            );
            return;
        }

        // Get current index statistics
        let (file_count, symbol_count) = self.get_index_stats();
        log::info!(
            "Starting symbol search for query: '{}' (mode: {:?}) - Index: {} files, {} symbols",
            query,
            search_params.mode,
            file_count,
            symbol_count
        );

        let mut matches = Vec::new();
        let mut total_symbols_checked = 0;
        let mut filtered_symbols = 0;

        // Search through all symbols with filtering based on search mode
        for (filepath, symbols) in &self.symbol_index {
            log::trace!("Checking {} symbols in file: {}", symbols.len(), filepath);

            for symbol in symbols {
                total_symbols_checked += 1;

                // Filter symbols based on search mode
                if !self.is_symbol_allowed_for_mode(symbol, search_params.mode) {
                    continue;
                }

                filtered_symbols += 1;
                log::trace!(
                    "Checking symbol: '{}' (type: {:?}) against query: '{}'",
                    symbol.content,
                    symbol.symbol_type,
                    query
                );

                if let Some(score) = self.fuzzy_matcher.fuzzy_match(&symbol.content, query) {
                    log::trace!(
                        "Found match: '{}' with score: {} (type: {:?})",
                        symbol.content,
                        score,
                        symbol.symbol_type
                    );
                    matches.push((score, symbol));
                }
            }
        }

        log::info!(
            "Symbol search analysis: {} total symbols, {} after filtering, {} matches",
            total_symbols_checked,
            filtered_symbols,
            matches.len()
        );

        // Sort by score (higher is better)
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        // Send results (limit to reasonable number)
        let limit = 50; // Configurable limit
        let results_to_send = matches.len().min(limit);
        log::info!(
            "Sending {} search results (limit: {})",
            results_to_send,
            limit
        );

        for (index, (score, symbol)) in matches.into_iter().take(limit).enumerate() {
            let search_result = SearchResult {
                filename: symbol.filepath.clone(),
                line: symbol.line,
                column: symbol.column,
                content: format!("[{}] {}", symbol.symbol_type.display_name(), symbol.content),
            };

            log::trace!(
                "Sending result {}/{}: '{}' (score: {}, type: {:?})",
                index + 1,
                results_to_send,
                symbol.content,
                score,
                symbol.symbol_type
            );

            if let Err(e) = controller
                .send_message(
                    "pushSearchResult".to_string(),
                    FaeMessage::PushSearchResult(search_result),
                )
                .await
            {
                log::error!("Failed to send search result {}: {}", index + 1, e);
                break;
            }

            log::trace!(
                "Successfully sent search result for '{}' (score: {})",
                symbol.content,
                score
            );
        }

        log::info!(
            "Completed symbol search for query: '{}' - sent {} results",
            query,
            results_to_send
        );

        // Send completion notification
        if let Err(e) = controller
            .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
            .await
        {
            log::error!("Failed to send completeSearch message: {}", e);
        } else {
            log::trace!("Successfully sent completeSearch notification");
        }
    }

    /// Check if a symbol is allowed for the given search mode
    fn is_symbol_allowed_for_mode(&self, symbol: &Symbol, mode: SearchMode) -> bool {
        use crate::actors::types::SymbolType;

        match mode {
            SearchMode::Symbol => {
                // Symbol mode excludes variables, constants, fields, and parameters
                !matches!(
                    symbol.symbol_type,
                    SymbolType::Variable
                        | SymbolType::Constant
                        | SymbolType::Field
                        | SymbolType::Parameter
                )
            }
            SearchMode::Variable => {
                // Variable mode includes variables, constants, fields, and parameters
                matches!(
                    symbol.symbol_type,
                    SymbolType::Variable
                        | SymbolType::Constant
                        | SymbolType::Field
                        | SymbolType::Parameter
                )
            }
            _ => false, // Other modes are not handled here
        }
    }

    /// Get statistics about the current index
    pub fn get_index_stats(&self) -> (usize, usize) {
        let file_count = self.symbol_index.len();
        let symbol_count = self.symbol_index.values().map(|v| v.len()).sum();
        (file_count, symbol_count)
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for SymbolSearchHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &ActorController<FaeMessage>,
    ) {
        match message.method.as_str() {
            "clearSymbolIndex" => {
                if let FaeMessage::ClearSymbolIndex(filepath) = message.payload {
                    self.clear_file_symbols(&filepath);
                } else {
                    log::warn!("clearSymbolIndex received unexpected payload");
                }
            }
            "pushSymbolIndex" => {
                if let FaeMessage::PushSymbolIndex {
                    filepath,
                    line,
                    column,
                    content,
                    symbol_type,
                } = message.payload
                {
                    let symbol = Symbol::new(filepath, line, column, content, symbol_type);
                    self.add_symbol(symbol);
                } else {
                    log::warn!("pushSymbolIndex received unexpected payload");
                }
            }
            "completeSymbolIndex" => {
                if let FaeMessage::CompleteSymbolIndex(filepath) = message.payload {
                    log::trace!("Symbol indexing completed for: {}", filepath);
                    // Note: Search will be triggered by updateSearchParams, not here
                    // to avoid repeated searches during bulk indexing
                } else {
                    log::warn!("completeSymbolIndex received unexpected payload");
                }
            }
            "completeInitialIndexing" => {
                if let FaeMessage::CompleteInitialIndexing = message.payload {
                    log::info!("Initial symbol indexing completed, enabling search functionality");
                    self.initial_indexing_complete = true;

                    // If there's a pending search, execute it now
                    if let Some(ref search_params) = self.current_search.clone() {
                        log::trace!("Executing pending search after initial indexing completion");
                        self.perform_search(search_params, controller).await;
                    }
                } else {
                    log::warn!("completeInitialIndexing received unexpected payload");
                }
            }
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(search_params) = message.payload {
                    log::trace!(
                        "Updated search params: query='{}', mode={:?}",
                        search_params.query,
                        search_params.mode
                    );

                    self.current_search = Some(search_params.clone());
                    self.perform_search(&search_params, controller).await;
                } else {
                    log::warn!("updateSearchParams received unexpected payload");
                }
            }
            _ => {
                log::trace!("Unknown message method: {}", message.method);
            }
        }
    }
}

/// Symbol search actor that maintains symbol index and provides fuzzy search
pub type SymbolSearchActor = Actor<FaeMessage, SymbolSearchHandler>;

impl SymbolSearchActor {
    /// Create a new SymbolSearchActor
    pub fn new_symbol_search_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    ) -> Self {
        let handler = SymbolSearchHandler::new();
        Self::new(message_receiver, sender, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SymbolType;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_symbol_search_handler_creation() {
        let handler = SymbolSearchHandler::new();
        let (file_count, symbol_count) = handler.get_index_stats();
        assert_eq!(file_count, 0);
        assert_eq!(symbol_count, 0);
    }

    #[test]
    fn test_add_and_clear_symbols() {
        let mut handler = SymbolSearchHandler::new();

        // Add symbols
        let symbol1 = Symbol::new(
            "test.rs".to_string(),
            10,
            5,
            "my_function".to_string(),
            SymbolType::Function,
        );
        let symbol2 = Symbol::new(
            "test.rs".to_string(),
            20,
            1,
            "MyStruct".to_string(),
            SymbolType::Struct,
        );

        handler.add_symbol(symbol1);
        handler.add_symbol(symbol2);

        let (file_count, symbol_count) = handler.get_index_stats();
        assert_eq!(file_count, 1);
        assert_eq!(symbol_count, 2);

        // Clear symbols
        handler.clear_file_symbols("test.rs");

        let (file_count, symbol_count) = handler.get_index_stats();
        assert_eq!(file_count, 0);
        assert_eq!(symbol_count, 0);
    }

    #[tokio::test]
    async fn test_symbol_search_actor_creation() {
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = SymbolSearchActor::new_symbol_search_actor(actor_rx, external_tx);

        // Test that we can create the actor successfully
        drop(actor);
    }

    #[tokio::test]
    async fn test_symbol_index_management() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolSearchActor::new_symbol_search_actor(actor_rx, external_tx);

        // Clear any existing symbols
        let clear_message = Message::new(
            "clearSymbolIndex",
            FaeMessage::ClearSymbolIndex("test.rs".to_string()),
        );
        actor_tx
            .send(clear_message)
            .expect("Failed to send clear message");

        // Add some symbols
        let push_message1 = Message::new(
            "pushSymbolIndex",
            FaeMessage::PushSymbolIndex {
                filepath: "test.rs".to_string(),
                line: 10,
                column: 5,
                content: "my_function".to_string(),
                symbol_type: SymbolType::Function,
            },
        );
        actor_tx
            .send(push_message1)
            .expect("Failed to send push message");

        let push_message2 = Message::new(
            "pushSymbolIndex",
            FaeMessage::PushSymbolIndex {
                filepath: "test.rs".to_string(),
                line: 20,
                column: 1,
                content: "MyStruct".to_string(),
                symbol_type: SymbolType::Struct,
            },
        );
        actor_tx
            .send(push_message2)
            .expect("Failed to send push message");

        // Complete indexing (should not trigger search yet)
        let complete_message = Message::new(
            "completeSymbolIndex",
            FaeMessage::CompleteSymbolIndex("test.rs".to_string()),
        );
        actor_tx
            .send(complete_message)
            .expect("Failed to send complete message");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_symbol_search() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolSearchActor::new_symbol_search_actor(actor_rx, external_tx);

        // Add some symbols first
        let push_message1 = Message::new(
            "pushSymbolIndex",
            FaeMessage::PushSymbolIndex {
                filepath: "test.rs".to_string(),
                line: 10,
                column: 5,
                content: "my_function".to_string(),
                symbol_type: SymbolType::Function,
            },
        );
        actor_tx
            .send(push_message1)
            .expect("Failed to send push message");

        let push_message2 = Message::new(
            "pushSymbolIndex",
            FaeMessage::PushSymbolIndex {
                filepath: "test.rs".to_string(),
                line: 20,
                column: 1,
                content: "MyStruct".to_string(),
                symbol_type: SymbolType::Struct,
            },
        );
        actor_tx
            .send(push_message2)
            .expect("Failed to send push message");

        // Wait for symbols to be added
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Perform search
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(SearchParams {
                query: "func".to_string(),
                mode: SearchMode::Symbol,
            }),
        );
        actor_tx
            .send(search_message)
            .expect("Failed to send search message");

        // Wait for search results
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check for search results
        let mut received_results = false;
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        println!("Received search result: {}", result.content);
                        if result.content.contains("my_function") {
                            received_results = true;
                        }
                    }
                }
            } else {
                break;
            }
        }

        assert!(
            received_results,
            "Should have received search results for 'func'"
        );

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_variable_search_filtering() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolSearchActor::new_symbol_search_actor(actor_rx, external_tx);

        // Add various symbol types
        let symbols = vec![
            (SymbolType::Function, "my_function"),
            (SymbolType::Variable, "my_variable"),
            (SymbolType::Constant, "MY_CONSTANT"),
            (SymbolType::Struct, "MyStruct"),
            (SymbolType::Variable, "another_var"),
            (SymbolType::Method, "my_method"),
            (SymbolType::Field, "my_field"),
        ];

        for (symbol_type, content) in symbols {
            let push_message = Message::new(
                "pushSymbolIndex",
                FaeMessage::PushSymbolIndex {
                    filepath: "test.rs".to_string(),
                    line: 10,
                    column: 5,
                    content: content.to_string(),
                    symbol_type,
                },
            );
            actor_tx
                .send(push_message)
                .expect("Failed to send push message");
        }

        // Wait for symbols to be added
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Test Symbol search (should exclude variables and constants)
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(SearchParams {
                query: "my".to_string(),
                mode: SearchMode::Symbol,
            }),
        );
        actor_tx
            .send(search_message)
            .expect("Failed to send search message");

        // Wait for search results
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Collect Symbol search results
        let mut symbol_results = Vec::new();
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        symbol_results.push(result.content.clone());
                    }
                }
            } else {
                break;
            }
        }

        println!("Symbol search results: {:?}", symbol_results);

        // Verify Symbol search excludes variables and constants
        assert!(
            symbol_results.iter().any(|r| r.contains("my_function")),
            "Symbol search should include functions"
        );
        assert!(
            symbol_results.iter().any(|r| r.contains("MyStruct")),
            "Symbol search should include structs"
        );
        assert!(
            symbol_results.iter().any(|r| r.contains("my_method")),
            "Symbol search should include methods"
        );
        assert!(
            !symbol_results.iter().any(|r| r.contains("my_variable")),
            "Symbol search should exclude variables"
        );
        assert!(
            !symbol_results.iter().any(|r| r.contains("MY_CONSTANT")),
            "Symbol search should exclude constants"
        );
        assert!(
            !symbol_results.iter().any(|r| r.contains("my_field")),
            "Symbol search should exclude fields"
        );

        // Test Variable search (should only include variables, constants, and fields)
        let variable_search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(SearchParams {
                query: "my".to_string(),
                mode: SearchMode::Variable,
            }),
        );
        actor_tx
            .send(variable_search_message)
            .expect("Failed to send variable search message");

        // Wait for search results
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Collect Variable search results
        let mut variable_results = Vec::new();
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        variable_results.push(result.content.clone());
                    }
                }
            } else {
                break;
            }
        }

        println!("Variable search results: {:?}", variable_results);

        // Verify Variable search only includes variables, constants, and fields
        assert!(
            variable_results.iter().any(|r| r.contains("my_variable")),
            "Variable search should include variables"
        );
        assert!(
            variable_results.iter().any(|r| r.contains("MY_CONSTANT")),
            "Variable search should include constants"
        );
        assert!(
            variable_results.iter().any(|r| r.contains("my_field")),
            "Variable search should include fields"
        );
        assert!(
            !variable_results.iter().any(|r| r.contains("my_function")),
            "Variable search should exclude functions"
        );
        assert!(
            !variable_results.iter().any(|r| r.contains("MyStruct")),
            "Variable search should exclude structs"
        );
        assert!(
            !variable_results.iter().any(|r| r.contains("my_method")),
            "Variable search should exclude methods"
        );

        // Clean up
        actor.shutdown();
    }

    #[test]
    fn test_symbol_filtering_logic() {
        let handler = SymbolSearchHandler::new();

        // Test Symbol mode filtering
        let function_symbol = Symbol::new(
            "test.rs".to_string(),
            10,
            5,
            "test_function".to_string(),
            SymbolType::Function,
        );
        let variable_symbol = Symbol::new(
            "test.rs".to_string(),
            20,
            5,
            "test_variable".to_string(),
            SymbolType::Variable,
        );
        let constant_symbol = Symbol::new(
            "test.rs".to_string(),
            30,
            5,
            "TEST_CONSTANT".to_string(),
            SymbolType::Constant,
        );
        let field_symbol = Symbol::new(
            "test.rs".to_string(),
            40,
            5,
            "test_field".to_string(),
            SymbolType::Field,
        );

        // Symbol mode should include functions but exclude variables/constants/fields
        assert!(
            handler.is_symbol_allowed_for_mode(&function_symbol, SearchMode::Symbol),
            "Symbol mode should allow functions"
        );
        assert!(
            !handler.is_symbol_allowed_for_mode(&variable_symbol, SearchMode::Symbol),
            "Symbol mode should exclude variables"
        );
        assert!(
            !handler.is_symbol_allowed_for_mode(&constant_symbol, SearchMode::Symbol),
            "Symbol mode should exclude constants"
        );
        assert!(
            !handler.is_symbol_allowed_for_mode(&field_symbol, SearchMode::Symbol),
            "Symbol mode should exclude fields"
        );

        // Variable mode should exclude functions but include variables/constants/fields
        assert!(
            !handler.is_symbol_allowed_for_mode(&function_symbol, SearchMode::Variable),
            "Variable mode should exclude functions"
        );
        assert!(
            handler.is_symbol_allowed_for_mode(&variable_symbol, SearchMode::Variable),
            "Variable mode should allow variables"
        );
        assert!(
            handler.is_symbol_allowed_for_mode(&constant_symbol, SearchMode::Variable),
            "Variable mode should allow constants"
        );
        assert!(
            handler.is_symbol_allowed_for_mode(&field_symbol, SearchMode::Variable),
            "Variable mode should allow fields"
        );

        // Other modes should return false
        assert!(
            !handler.is_symbol_allowed_for_mode(&function_symbol, SearchMode::Literal),
            "Literal mode should return false"
        );
        assert!(
            !handler.is_symbol_allowed_for_mode(&variable_symbol, SearchMode::Filepath),
            "Filepath mode should return false"
        );
    }

    #[tokio::test]
    async fn test_search_waiting_for_initial_indexing() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolSearchActor::new_symbol_search_actor(actor_rx, external_tx);

        // Add some symbols first
        let push_message = Message::new(
            "pushSymbolIndex",
            FaeMessage::PushSymbolIndex {
                filepath: "test.rs".to_string(),
                line: 10,
                column: 5,
                content: "my_function".to_string(),
                symbol_type: SymbolType::Function,
            },
        );
        actor_tx
            .send(push_message)
            .expect("Failed to send push message");

        // Wait for symbols to be added
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Try to search BEFORE initial indexing completion - should be skipped
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(SearchParams {
                query: "func".to_string(),
                mode: SearchMode::Symbol,
            }),
        );
        actor_tx
            .send(search_message)
            .expect("Failed to send search message");

        // Wait a bit - no search results should be received yet
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that no search results were received
        let mut received_results_before = false;
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    received_results_before = true;
                    break;
                }
            } else {
                break;
            }
        }

        assert!(
            !received_results_before,
            "Should not receive search results before initial indexing completion"
        );

        // Now send initial indexing completion
        let complete_initial_message = Message::new(
            "completeInitialIndexing",
            FaeMessage::CompleteInitialIndexing,
        );
        actor_tx
            .send(complete_initial_message)
            .expect("Failed to send complete initial indexing message");

        // Wait for search to be executed
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check that search results are now received
        let mut received_results_after = false;
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        if result.content.contains("my_function") {
                            received_results_after = true;
                            break;
                        }
                    }
                }
            } else {
                break;
            }
        }

        assert!(
            received_results_after,
            "Should receive search results after initial indexing completion"
        );

        // Clean up
        actor.shutdown();
    }

    #[test]
    fn test_initial_indexing_complete_flag() {
        let handler = SymbolSearchHandler::new();
        assert!(
            !handler.initial_indexing_complete,
            "Initial indexing should not be complete at creation"
        );
    }
}
