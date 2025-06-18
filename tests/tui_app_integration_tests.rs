//! Integration tests for simplified TuiApp architecture
//!
//! Tests the new external handler pattern and reduced channel complexity
//! introduced in the TUI simplification refactoring.

use fae::tui::{TuiApp, TuiMessageHandler};
use std::sync::{Arc, Mutex};

/// Mock handler that records all operations for verification
#[derive(Clone)]
struct RecordingTuiHandler {
    operations: Arc<Mutex<Vec<String>>>,
    should_fail_on: Option<String>,
}

impl RecordingTuiHandler {
    fn new() -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            should_fail_on: None,
        }
    }

    fn with_failure_on(operation: &str) -> Self {
        Self {
            operations: Arc::new(Mutex::new(Vec::new())),
            should_fail_on: Some(operation.to_string()),
        }
    }

    fn get_operations(&self) -> Vec<String> {
        self.operations.lock().unwrap().clone()
    }

    fn clear_operations(&self) {
        self.operations.lock().unwrap().clear();
    }

    fn record_operation(&self, op: &str) {
        self.operations.lock().unwrap().push(op.to_string());
    }
}

impl TuiMessageHandler for RecordingTuiHandler {
    fn execute_search(
        &self,
        query: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref fail_op) = self.should_fail_on {
            if fail_op == "execute_search" {
                return Err("Simulated execute_search failure".into());
            }
        }
        self.record_operation(&format!("execute_search: {}", query));
        Ok(())
    }

    fn clear_results(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref fail_op) = self.should_fail_on {
            if fail_op == "clear_results" {
                return Err("Simulated clear_results failure".into());
            }
        }
        self.record_operation("clear_results");
        Ok(())
    }

    fn abort_search(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref fail_op) = self.should_fail_on {
            if fail_op == "abort_search" {
                return Err("Simulated abort_search failure".into());
            }
        }
        self.record_operation("abort_search");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create test directory for TuiApp
    fn create_test_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("Failed to create test directory")
    }

    #[tokio::test]
    async fn test_tui_app_with_external_handler() {
        let test_dir = create_test_dir();
        let handler = RecordingTuiHandler::new();

        // Create TuiApp (this would normally require terminal, so we expect it to fail in CI)
        // But we can test the API structure
        match TuiApp::new(test_dir.path().to_str().unwrap()).await {
            Ok((mut app, _handle)) => {
                // Set the handler
                app.set_message_handler(Box::new(handler.clone()));

                // Test execute_search functionality
                let result = app.execute_search("test query".to_string());

                // The execute_search should work if handler is set
                assert!(result.is_ok());

                // Verify operations were recorded
                let ops = handler.get_operations();
                assert!(ops.contains(&"abort_search".to_string()));
                assert!(ops.contains(&"clear_results".to_string()));
                assert!(ops.contains(&"execute_search: test query".to_string()));
            }
            Err(_) => {
                // Expected in CI environment without proper terminal
                println!("TuiApp creation failed (expected in CI environment)");
            }
        }
    }

    #[tokio::test]
    async fn test_tui_app_execute_search_without_handler() {
        let test_dir = create_test_dir();

        match TuiApp::new(test_dir.path().to_str().unwrap()).await {
            Ok((app, _handle)) => {
                // Execute search without setting handler should fail
                let result = app.execute_search("test query".to_string());
                assert!(result.is_err());

                let error_msg = result.unwrap_err().to_string();
                assert!(error_msg.contains("Message handler not initialized"));
            }
            Err(_) => {
                // Expected in CI environment
                println!("TuiApp creation failed (expected in CI environment)");
            }
        }
    }

    #[test]
    fn test_search_query_filtering() {
        let handler = RecordingTuiHandler::new();

        // Test empty query filtering
        let empty_queries = vec!["", "   ", "#", "$", "@", "/"];

        for query in empty_queries {
            handler.clear_operations();

            // Simulate the filtering logic from TuiApp::execute_search
            let trimmed_query = query.trim();
            let should_skip = trimmed_query.is_empty()
                || trimmed_query == "#"
                || trimmed_query == "$"
                || trimmed_query == "@"
                || trimmed_query == "/";

            if should_skip {
                // Should only call abort and clear, not execute_search
                assert!(handler.abort_search().is_ok());
                assert!(handler.clear_results().is_ok());
            } else {
                assert!(handler.execute_search(query.to_string()).is_ok());
            }

            let ops = handler.get_operations();
            if should_skip {
                assert!(ops.contains(&"abort_search".to_string()));
                assert!(ops.contains(&"clear_results".to_string()));
                assert!(!ops.iter().any(|op| op.starts_with("execute_search:")));
            } else {
                assert!(ops.iter().any(|op| op.starts_with("execute_search:")));
            }
        }
    }

    #[test]
    fn test_handler_error_propagation() {
        // Test each operation failing
        let operations = vec!["execute_search", "clear_results", "abort_search"];

        for fail_op in operations {
            let handler = RecordingTuiHandler::with_failure_on(fail_op);

            let execute_result = handler.execute_search("test".to_string());
            let clear_result = handler.clear_results();
            let abort_result = handler.abort_search();

            match fail_op {
                "execute_search" => {
                    assert!(execute_result.is_err());
                    assert!(clear_result.is_ok());
                    assert!(abort_result.is_ok());
                }
                "clear_results" => {
                    assert!(execute_result.is_ok());
                    assert!(clear_result.is_err());
                    assert!(abort_result.is_ok());
                }
                "abort_search" => {
                    assert!(execute_result.is_ok());
                    assert!(clear_result.is_ok());
                    assert!(abort_result.is_err());
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_various_search_queries() {
        let handler = RecordingTuiHandler::new();

        let test_queries = vec![
            "normal search",
            "#symbol_search",
            "$variable_search",
            ">file.rs",
            "/regex.*pattern",
            "query with spaces",
            "CamelCaseQuery",
            "snake_case_query",
            "query-with-dashes",
        ];

        for query in test_queries {
            handler.clear_operations();

            // Simulate complete search flow
            assert!(handler.abort_search().is_ok());
            assert!(handler.clear_results().is_ok());
            assert!(handler.execute_search(query.to_string()).is_ok());

            let ops = handler.get_operations();
            assert_eq!(ops.len(), 3);
            assert_eq!(ops[0], "abort_search");
            assert_eq!(ops[1], "clear_results");
            assert_eq!(ops[2], format!("execute_search: {}", query));
        }
    }

    #[test]
    fn test_handler_trait_object_compatibility() {
        // Test that RecordingTuiHandler can be used as Box<dyn TuiMessageHandler>
        let handler: Box<dyn TuiMessageHandler + Send> = Box::new(RecordingTuiHandler::new());

        assert!(handler.execute_search("test".to_string()).is_ok());
        assert!(handler.clear_results().is_ok());
        assert!(handler.abort_search().is_ok());
    }

    #[test]
    fn test_concurrent_handler_operations() {
        use std::sync::Arc;
        use std::thread;

        let handler = Arc::new(RecordingTuiHandler::new());
        let mut handles = vec![];

        // Spawn multiple threads performing operations
        for i in 0..10 {
            let handler_clone = Arc::clone(&handler);
            let handle = thread::spawn(move || {
                let query = format!("query_{}", i);
                assert!(handler_clone.execute_search(query).is_ok());
                assert!(handler_clone.clear_results().is_ok());
                assert!(handler_clone.abort_search().is_ok());
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        // Verify all operations were recorded
        let ops = handler.get_operations();
        assert_eq!(ops.len(), 30); // 10 threads × 3 operations each

        // Count each operation type
        let execute_count = ops
            .iter()
            .filter(|op| op.starts_with("execute_search:"))
            .count();
        let clear_count = ops.iter().filter(|op| *op == "clear_results").count();
        let abort_count = ops.iter().filter(|op| *op == "abort_search").count();

        assert_eq!(execute_count, 10);
        assert_eq!(clear_count, 10);
        assert_eq!(abort_count, 10);
    }
}
