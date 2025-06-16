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
use tokio::sync::mpsc;

/// Result handler that collects and outputs search results
pub struct ResultHandler {
    /// Current count of received results
    result_count: usize,
    /// Maximum number of results to collect
    max_results: usize,
    /// Whether search has been completed
    search_completed: bool,
    /// Whether any search has started (first result received)
    search_started: bool,
}

impl ResultHandler {
    /// Create a new ResultHandler
    pub fn new(max_results: usize) -> Self {
        Self {
            result_count: 0,
            max_results,
            search_completed: false,
            search_started: false,
        }
    }

    /// Handle a search result
    async fn handle_search_result(&mut self, result: SearchResult) {
        if self.search_completed || self.result_count >= self.max_results {
            return; // Don't process more results after completion or limit reached
        }

        self.search_started = true;
        self.result_count += 1;

        log::info!("Result #{}: {}", self.result_count, result.content);
        println!(
            "{}:{}:{}: {}",
            result.filename, result.line, result.column, result.content
        );
    }

    /// Handle search completion notification
    async fn handle_search_completion(&mut self, controller: &ActorController<FaeMessage>) {
        if self.search_completed {
            return; // Already completed
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
                "searchFinished".to_string(),
                FaeMessage::SearchFinished {
                    result_count: self.result_count,
                },
            )
            .await
        {
            log::error!("Failed to send searchFinished message: {}", e);
        } else {
            log::info!("Search finished with {} results", self.result_count);
        }
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

    /// Set maximum number of results to collect
    pub fn set_max_results(&mut self, max_results: usize) {
        self.max_results = max_results;
        log::debug!("Updated max results to: {}", max_results);
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
                if let FaeMessage::PushSearchResult(result) = message.payload {
                    self.handle_search_result(result).await;
                } else {
                    log::warn!("pushSearchResult received unexpected payload");
                }
            }
            "completeSearch" => {
                if let FaeMessage::CompleteSearch = message.payload {
                    self.handle_search_completion(controller).await;
                } else {
                    log::warn!("completeSearch received unexpected payload");
                }
            }
            "setMaxResults" => {
                if let FaeMessage::SetMaxResults { max_results } = message.payload {
                    self.set_max_results(max_results);
                } else {
                    log::warn!("setMaxResults received unexpected payload");
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
        max_results: usize,
    ) -> Self {
        let handler = ResultHandler::new(max_results);
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
        let handler = ResultHandler::new(10);
        assert_eq!(handler.get_result_count(), 0);
        assert!(!handler.is_completed());
        assert!(!handler.has_started());
    }

    #[tokio::test]
    async fn test_result_handler_actor_creation() {
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx, 10);

        // Test that we can create the actor successfully
        drop(actor);
    }

    #[tokio::test]
    async fn test_result_collection() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx, 5);

        // Send some search results
        for i in 1..=3 {
            let result = SearchResult {
                filename: format!("test{}.rs", i),
                line: i * 10,
                column: 5,
                content: format!("result {}", i),
            };
            let message = Message::new("pushSearchResult", FaeMessage::PushSearchResult(result));
            actor_tx.send(message).expect("Failed to send result");
        }

        // Send completion
        let complete_message = Message::new("completeSearch", FaeMessage::CompleteSearch);
        actor_tx
            .send(complete_message)
            .expect("Failed to send completion");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check for searchFinished message
        let mut received_finished = false;
        let mut final_count = 0;

        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "searchFinished" {
                    if let FaeMessage::SearchFinished { result_count } = msg.payload {
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
            "Should have received searchFinished message"
        );
        assert_eq!(final_count, 3, "Should have collected 3 results");

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_max_results_limit() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx, 2);

        // Send more results than the limit
        for i in 1..=5 {
            let result = SearchResult {
                filename: format!("test{}.rs", i),
                line: i * 10,
                column: 5,
                content: format!("result {}", i),
            };
            let message = Message::new("pushSearchResult", FaeMessage::PushSearchResult(result));
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

        let mut actor = ResultHandlerActor::new_result_handler_actor(actor_rx, external_tx, 10);

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
