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
}

impl SymbolSearchHandler {
    /// Create a new SymbolSearchHandler
    pub fn new() -> Self {
        Self {
            symbol_index: HashMap::new(),
            fuzzy_matcher: SkimMatcherV2::default(),
            current_search: None,
        }
    }

    /// Clear all symbols for a specific file
    fn clear_file_symbols(&mut self, filepath: &str) {
        self.symbol_index.remove(filepath);
        log::debug!("Cleared symbols for file: {}", filepath);
    }

    /// Add a symbol to the index
    fn add_symbol(&mut self, symbol: Symbol) {
        let filepath = symbol.filepath.clone();
        self.symbol_index
            .entry(filepath.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
        log::trace!("Added symbol to index for file: {}", filepath);
    }

    /// Perform fuzzy search on symbols
    async fn perform_search(
        &self,
        search_params: &SearchParams,
        controller: &ActorController<FaeMessage>,
    ) {
        // Only perform search for Symbol mode
        if search_params.mode != SearchMode::Symbol {
            log::debug!(
                "Ignoring search for non-symbol mode: {:?}",
                search_params.mode
            );
            return;
        }

        let query = &search_params.query;
        if query.is_empty() {
            log::debug!("Empty query, skipping search");
            return;
        }

        log::debug!("Performing symbol search for query: '{}'", query);

        let mut matches = Vec::new();

        // Search through all symbols
        for symbols in self.symbol_index.values() {
            for symbol in symbols {
                if let Some(score) = self.fuzzy_matcher.fuzzy_match(&symbol.content, query) {
                    matches.push((score, symbol));
                }
            }
        }

        // Sort by score (higher is better)
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        // Send results (limit to reasonable number)
        let limit = 50; // Configurable limit
        for (score, symbol) in matches.into_iter().take(limit) {
            let search_result = SearchResult {
                filename: symbol.filepath.clone(),
                line: symbol.line,
                column: symbol.column,
                content: format!("[{}] {}", symbol.symbol_type.display_name(), symbol.content),
            };

            if let Err(e) = controller
                .send_message(
                    "pushSearchResult".to_string(),
                    FaeMessage::PushSearchResult(search_result),
                )
                .await
            {
                log::warn!("Failed to send search result: {}", e);
                break;
            }

            log::trace!(
                "Sent search result for '{}' (score: {})",
                symbol.content,
                score
            );
        }

        log::debug!("Completed symbol search for query: '{}'", query);
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
                    log::debug!("Symbol indexing completed for: {}", filepath);

                    // If we have pending search, perform it now
                    if let Some(ref search_params) = self.current_search.clone() {
                        self.perform_search(search_params, controller).await;
                    }
                } else {
                    log::warn!("completeSymbolIndex received unexpected payload");
                }
            }
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(search_params) = message.payload {
                    log::debug!(
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
                log::debug!("Unknown message method: {}", message.method);
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
}
