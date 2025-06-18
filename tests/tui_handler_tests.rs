//! Tests for TUI message handler and simplified architecture
//!
//! This test module covers the new TuiMessageHandler trait and the simplified
//! TUI channel architecture introduced to reduce complexity.

use fae::actors::messages::FaeMessage;
use fae::core::Message;
use fae::tui::TuiMessageHandler;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Mock implementation of TuiMessageHandler for testing
struct MockTuiHandler {
    executed_searches: Arc<Mutex<Vec<String>>>,
    cleared_count: Arc<Mutex<usize>>,
    aborted_count: Arc<Mutex<usize>>,
    should_fail: bool,
}

impl MockTuiHandler {
    fn new() -> Self {
        Self {
            executed_searches: Arc::new(Mutex::new(Vec::new())),
            cleared_count: Arc::new(Mutex::new(0)),
            aborted_count: Arc::new(Mutex::new(0)),
            should_fail: false,
        }
    }

    fn with_failure() -> Self {
        Self {
            executed_searches: Arc::new(Mutex::new(Vec::new())),
            cleared_count: Arc::new(Mutex::new(0)),
            aborted_count: Arc::new(Mutex::new(0)),
            should_fail: true,
        }
    }

    fn get_executed_searches(&self) -> Vec<String> {
        self.executed_searches.lock().unwrap().clone()
    }

    fn get_cleared_count(&self) -> usize {
        *self.cleared_count.lock().unwrap()
    }

    fn get_aborted_count(&self) -> usize {
        *self.aborted_count.lock().unwrap()
    }
}

impl TuiMessageHandler for MockTuiHandler {
    fn execute_search(&self, query: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.should_fail {
            return Err("Mock search execution failed".into());
        }
        self.executed_searches.lock().unwrap().push(query);
        Ok(())
    }

    fn clear_results(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.should_fail {
            return Err("Mock clear results failed".into());
        }
        *self.cleared_count.lock().unwrap() += 1;
        Ok(())
    }

    fn abort_search(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.should_fail {
            return Err("Mock abort search failed".into());
        }
        *self.aborted_count.lock().unwrap() += 1;
        Ok(())
    }
}

/// TuiSearchHandler implementation for testing (extracted from main.rs)
struct TuiSearchHandler {
    control_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
}

impl TuiSearchHandler {
    fn new() -> (Self, mpsc::UnboundedReceiver<Message<FaeMessage>>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { control_sender: sender }, receiver)
    }
}

impl TuiMessageHandler for TuiSearchHandler {
    fn execute_search(&self, query: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use fae::actors::messages::FaeMessage;
        use fae::cli::create_search_params;
        use fae::core::Message;

        // Parse the query and determine search mode
        let search_params = create_search_params(&query);

        // Generate request ID and send search request
        let request_id = tiny_id::ShortCodeGenerator::new_alphanumeric(8).next_string();
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams {
                params: search_params,
                request_id,
            },
        );

        self.control_sender.send(search_message)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(())
    }

    fn clear_results(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let clear_message = Message::new("clearResults", FaeMessage::ClearResults);
        self.control_sender.send(clear_message)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }

    fn abort_search(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let abort_message = Message::new("abortSearch", FaeMessage::AbortSearch);
        self.control_sender.send(abort_message)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_message_handler_trait() {
        let handler = MockTuiHandler::new();
        
        // Test execute_search
        assert!(handler.execute_search("test query".to_string()).is_ok());
        assert_eq!(handler.get_executed_searches(), vec!["test query"]);
        
        // Test clear_results
        assert!(handler.clear_results().is_ok());
        assert_eq!(handler.get_cleared_count(), 1);
        
        // Test abort_search
        assert!(handler.abort_search().is_ok());
        assert_eq!(handler.get_aborted_count(), 1);
    }

    #[test]
    fn test_tui_message_handler_multiple_operations() {
        let handler = MockTuiHandler::new();
        
        // Execute multiple searches
        assert!(handler.execute_search("query1".to_string()).is_ok());
        assert!(handler.execute_search("query2".to_string()).is_ok());
        assert_eq!(handler.get_executed_searches(), vec!["query1", "query2"]);
        
        // Multiple clears and aborts
        assert!(handler.clear_results().is_ok());
        assert!(handler.clear_results().is_ok());
        assert_eq!(handler.get_cleared_count(), 2);
        
        assert!(handler.abort_search().is_ok());
        assert!(handler.abort_search().is_ok());
        assert_eq!(handler.get_aborted_count(), 2);
    }

    #[test]
    fn test_tui_message_handler_error_handling() {
        let handler = MockTuiHandler::with_failure();
        
        // All operations should fail
        assert!(handler.execute_search("test".to_string()).is_err());
        assert!(handler.clear_results().is_err());
        assert!(handler.abort_search().is_err());
    }

    #[tokio::test]
    async fn test_tui_search_handler_execute_search() {
        let (handler, mut receiver) = TuiSearchHandler::new();
        
        // Execute a search
        assert!(handler.execute_search("test query".to_string()).is_ok());
        
        // Verify message was sent
        let message = receiver.recv().await.expect("Should receive message");
        assert_eq!(message.method, "updateSearchParams");
        
        match message.payload {
            FaeMessage::UpdateSearchParams { params, request_id } => {
                assert_eq!(params.query, "test query");
                assert!(!request_id.is_empty());
            }
            _ => panic!("Expected UpdateSearchParams message"),
        }
    }

    #[tokio::test]
    async fn test_tui_search_handler_clear_results() {
        let (handler, mut receiver) = TuiSearchHandler::new();
        
        // Clear results
        assert!(handler.clear_results().is_ok());
        
        // Verify message was sent
        let message = receiver.recv().await.expect("Should receive message");
        assert_eq!(message.method, "clearResults");
        
        match message.payload {
            FaeMessage::ClearResults => {},
            _ => panic!("Expected ClearResults message"),
        }
    }

    #[tokio::test]
    async fn test_tui_search_handler_abort_search() {
        let (handler, mut receiver) = TuiSearchHandler::new();
        
        // Abort search
        assert!(handler.abort_search().is_ok());
        
        // Verify message was sent
        let message = receiver.recv().await.expect("Should receive message");
        assert_eq!(message.method, "abortSearch");
        
        match message.payload {
            FaeMessage::AbortSearch => {},
            _ => panic!("Expected AbortSearch message"),
        }
    }

    #[tokio::test]
    async fn test_tui_search_handler_search_modes() {
        let (handler, mut receiver) = TuiSearchHandler::new();
        
        // Test different search modes
        let test_cases = vec![
            ("normal query", fae::actors::types::SearchMode::Literal),
            ("#symbol", fae::actors::types::SearchMode::Symbol),
            ("$variable", fae::actors::types::SearchMode::Variable),
            ("@filename", fae::actors::types::SearchMode::Filepath),
            ("/regex", fae::actors::types::SearchMode::Regexp),
        ];
        
        for (query, expected_mode) in test_cases {
            assert!(handler.execute_search(query.to_string()).is_ok());
            
            let message = receiver.recv().await.expect("Should receive message");
            match message.payload {
                FaeMessage::UpdateSearchParams { params, .. } => {
                    assert_eq!(params.mode, expected_mode);
                }
                _ => panic!("Expected UpdateSearchParams message"),
            }
        }
    }

    #[tokio::test]
    async fn test_tui_search_handler_channel_failure() {
        let (handler, receiver) = TuiSearchHandler::new();
        
        // Drop receiver to simulate channel failure
        drop(receiver);
        
        // Operations should fail when channel is closed
        assert!(handler.execute_search("test".to_string()).is_err());
        assert!(handler.clear_results().is_err());
        assert!(handler.abort_search().is_err());
    }

    #[tokio::test]
    async fn test_tui_search_handler_request_id_uniqueness() {
        let (handler, mut receiver) = TuiSearchHandler::new();
        
        // Execute multiple searches
        assert!(handler.execute_search("query1".to_string()).is_ok());
        assert!(handler.execute_search("query2".to_string()).is_ok());
        
        // Collect request IDs
        let mut request_ids = Vec::new();
        for _ in 0..2 {
            let message = receiver.recv().await.expect("Should receive message");
            match message.payload {
                FaeMessage::UpdateSearchParams { request_id, .. } => {
                    request_ids.push(request_id);
                }
                _ => panic!("Expected UpdateSearchParams message"),
            }
        }
        
        // Request IDs should be unique
        assert_ne!(request_ids[0], request_ids[1]);
        assert!(!request_ids[0].is_empty());
        assert!(!request_ids[1].is_empty());
    }
}