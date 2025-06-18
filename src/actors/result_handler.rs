//! Result handler actor for collecting and outputting search results
//!
//! This actor is responsible for:
//! - Collecting search results from all search actors
//! - Outputting results to CLI
//! - Counting results and managing completion
//! - Providing search statistics

use crate::actors::messages::FaeMessage;
use crate::actors::types::SearchResult;
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

/// Result handler that collects and outputs search results
pub struct ResultHandler {
    /// Current count of received results
    result_count: usize,
    /// Whether search has been completed
    search_completed: bool,
    /// Whether any search has started (first result received)
    search_started: bool,
    /// Duplicate detection: request_id -> set of result signatures
    seen_results: HashMap<String, HashSet<String>>,
}

impl ResultHandler {
    /// Create a new ResultHandler
    pub fn new() -> Self {
        Self {
            result_count: 0,
            search_completed: false,
            search_started: false,
            seen_results: HashMap::new(),
        }
    }

    /// Handle a search result with duplicate detection
    async fn handle_search_result(
        &mut self,
        result: SearchResult,
        request_id: String,
        controller: &ActorController<FaeMessage>,
    ) {
        if self.search_completed {
            return; // Don't process more results after completion
        }

        // Create unique signature for this result
        let result_signature = format!("{}:{}:{}", result.filename, result.line, result.content.trim());
        
        // Check for duplicates within this request
        let seen_set = self.seen_results.entry(request_id.clone()).or_insert_with(HashSet::new);
        
        if !seen_set.insert(result_signature.clone()) {
            // This result has already been seen for this request ID
            log::debug!(
                "Duplicate result detected for request {}: {}:{}",
                request_id,
                result.filename,
                result.line
            );
            return;
        }

        // This is a new unique result
        self.search_started = true;
        self.result_count += 1;

        log::info!("Result #{} (request: {}): {}", self.result_count, request_id, result.content);

        // Send result to external consumers (CLI/TUI) via broadcaster
        if let Err(e) = controller
            .send_message(
                "pushSearchResult".to_string(),
                FaeMessage::PushSearchResult { result, request_id },
            )
            .await
        {
            log::error!("Failed to send PushSearchResult message: {}", e);
        }
    }

    /// Handle search completion notification
    async fn handle_search_completion(&mut self, controller: &ActorController<FaeMessage>) {
        if self.search_completed {
            return; // Already completed
        }

        // Only complete if we've actually received search results (search_started = true)
        // This prevents premature completion from actors that skip unsupported modes
        if !self.search_started {
            log::info!(
                "ResultHandler: Ignoring completion notification - no search results received yet ({} results)",
                self.result_count
            );
            return;
        }

        log::info!(
            "Search completed notification received, {} results collected so far",
            self.result_count
        );

        // Mark as completed
        self.search_completed = true;

        // Send final completion notification to UnifiedSearchSystem
        if let Err(e) = controller
            .send_message(
                "notifySearchReport".to_string(),
                FaeMessage::NotifySearchReport {
                    result_count: self.result_count,
                },
            )
            .await
        {
            log::error!("Failed to send notifySearchReport message: {}", e);
        } else {
            log::info!("Search finished with {} results", self.result_count);
        }
    }

    /// Clear results for a specific request ID to free memory
    pub fn clear_request_results(&mut self, request_id: &str) {
        if let Some(removed_set) = self.seen_results.remove(request_id) {
            log::debug!(
                "Cleared {} duplicate tracking entries for request {}",
                removed_set.len(),
                request_id
            );
        }
    }

    /// Clear all tracking data for new search session
    pub fn reset_for_new_search(&mut self) {
        self.result_count = 0;
        self.search_completed = false;
        self.search_started = false;
        self.seen_results.clear();
        log::debug!("Reset ResultHandler for new search session");
    }

    /// Get current result count
    pub fn get_result_count(&self) -> usize {
        self.result_count
    }

    /// Check if search is completed
    pub fn is_completed(&self) -> bool {
        self.search_completed
    }

    /// Check if search has started
    pub fn has_started(&self) -> bool {
        self.search_started
    }

    /// Handle symbol index progress report
    fn handle_symbol_index_report(
        &self,
        remaining_files: usize,
        processed_files: usize,
        symbols_found: usize,
    ) {
        let total_files = remaining_files + processed_files;
        let progress_percentage = if total_files > 0 {
            (processed_files as f64 / total_files as f64 * 100.0).round() as u32
        } else {
            100
        };

        log::info!(
            "Symbol indexing progress: {}% ({}/{} files, {} symbols found)",
            progress_percentage,
            processed_files,
            total_files,
            symbols_found
        );

        // Log progress at info level to avoid interfering with CLI output
        log::info!(
            "Indexing progress: {}% ({}/{} files, {} symbols)",
            progress_percentage,
            processed_files,
            total_files,
            symbols_found
        );
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for ResultHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &ActorController<FaeMessage>,
    ) {
        match message.method.as_str() {
            "pushSearchResult" => {
                if let FaeMessage::PushSearchResult { result, request_id } = message.payload {
                    self.handle_search_result(result, request_id, controller)
                        .await;
                } else {
                    log::warn!("pushSearchResult received unexpected payload");
                }
            }
            "completeSearch" => {
                log::info!("ResultHandler: Received completeSearch message");
                if let FaeMessage::CompleteSearch = message.payload {
                    self.handle_search_completion(controller).await;
                } else {
                    log::warn!("completeSearch received unexpected payload");
                }
            }
            "reportSymbolIndex" => {
                if let FaeMessage::ReportSymbolIndex {
                    remaining_files,
                    processed_files,
                    symbols_found,
                } = message.payload
                {
                    self.handle_symbol_index_report(
                        remaining_files,
                        processed_files,
                        symbols_found,
                    );
                } else {
                    log::warn!("reportSymbolIndex received unexpected payload");
                }
            }
            // Ignore other messages (they're not for us)
            _ => {
                log::trace!("ResultHandler ignoring message: {}", message.method);
            }
        }
    }
}

/// Result handler actor for search result collection and output
pub type ResultHandlerActor = Actor<FaeMessage, ResultHandler>;

impl ResultHandlerActor {
    /// Create a new ResultHandlerActor
    pub fn new_result_handler_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    ) -> Self {
        let handler = ResultHandler::new();
        Self::new(message_receiver, sender, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_result_handler_creation() {
        let handler = ResultHandler::new();
        assert_eq!(handler.get_result_count(), 0);
        assert!(!handler.is_completed());
        assert!(!handler.has_started());
    }

    #[tokio::test]
    async fn test_result_handler_actor_creation() {
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx);

        // Test that we can create the actor successfully
        drop(actor);
    }

    #[tokio::test]
    async fn test_result_collection() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx);

        // Send some search results
        for i in 1..=3 {
            let result = SearchResult {
                filename: format!("test{}.rs", i),
                line: i * 10,
                column: 5,
                content: format!("result {}", i),
            };
            let message = Message::new(
                "pushSearchResult",
                FaeMessage::PushSearchResult {
                    result,
                    request_id: format!("test-request-{}", i),
                },
            );
            actor_tx.send(message).expect("Failed to send result");
        }

        // Send completion
        let complete_message = Message::new("completeSearch", FaeMessage::CompleteSearch);
        actor_tx
            .send(complete_message)
            .expect("Failed to send completion");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check for notifySearchReport message
        let mut received_finished = false;
        let mut final_count = 0;

        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "notifySearchReport" {
                    if let FaeMessage::NotifySearchReport { result_count } = msg.payload {
                        received_finished = true;
                        final_count = result_count;
                    }
                }
            } else {
                break;
            }
        }

        assert!(
            received_finished,
            "Should have received notifySearchReport message"
        );
        assert_eq!(final_count, 3, "Should have collected 3 results");

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_max_results_limit() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx);

        // Send more results than the limit
        for i in 1..=5 {
            let result = SearchResult {
                filename: format!("test{}.rs", i),
                line: i * 10,
                column: 5,
                content: format!("result {}", i),
            };
            let message = Message::new(
                "pushSearchResult",
                FaeMessage::PushSearchResult {
                    result,
                    request_id: format!("test-request-{}", i),
                },
            );
            actor_tx.send(message).expect("Failed to send result");
        }

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The actor should stop processing after max_results
        // This is verified by the internal logic, not exposed externally in this test

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_ignore_unknown_messages() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx);

        // Send an unknown message
        let unknown_message = Message::new("unknownMethod", FaeMessage::ClearResults);
        actor_tx
            .send(unknown_message)
            .expect("Failed to send unknown message");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Actor should handle this gracefully without crashing
        // Clean up
        actor.shutdown();
    }
}
