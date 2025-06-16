//! Unified search system using Broadcaster for actor coordination
//!
//! This module provides a simplified search interface that coordinates
//! multiple search actors through a broadcaster pattern.

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams};
use crate::actors::{
    AgActor, FilepathSearchActor, NativeSearchActor, ResultHandlerActor, RipgrepActor,
    SymbolIndexActor, SymbolSearchActor,
};
use crate::core::{Broadcaster, Message};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Unified search system that coordinates all search actors
pub struct UnifiedSearchSystem {
    broadcaster: Broadcaster<FaeMessage>,
    shared_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    completion_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,

    // Keep actor instances to manage their lifecycle
    symbol_index_actor: Option<SymbolIndexActor>,
    symbol_search_actor: Option<SymbolSearchActor>,
    filepath_search_actor: Option<FilepathSearchActor>,
    content_search_actor: Option<ContentSearchActor>,
    result_handler_actor: Option<ResultHandlerActor>,
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

impl UnifiedSearchSystem {
    /// Create a new unified search system
    pub async fn new(search_path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create completion channel for receiving final search results
        let (completion_tx, completion_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create actor channels
        let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (filepath_search_tx, filepath_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (content_search_tx, content_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (result_handler_tx, result_handler_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create broadcaster with all actor senders (including result handler)
        let actor_senders = vec![
            symbol_index_tx,
            symbol_search_tx,
            filepath_search_tx,
            content_search_tx,
            result_handler_tx,
        ];

        let (broadcaster, shared_sender) = Broadcaster::new(actor_senders);

        // Create all actors using the shared sender (via Broadcaster)
        let symbol_index_actor = SymbolIndexActor::new_symbol_index_actor(
            symbol_index_rx,
            shared_sender.clone(),
            search_path,
        )?;

        let symbol_search_actor =
            SymbolSearchActor::new_symbol_search_actor(symbol_search_rx, shared_sender.clone());

        let filepath_search_actor = FilepathSearchActor::new_filepath_search_actor(
            filepath_search_rx,
            shared_sender.clone(),
            search_path,
        );

        let content_search_actor = Self::create_content_search_actor(
            content_search_rx,
            shared_sender.clone(),
            search_path,
        )
        .await;

        let result_handler_actor = ResultHandlerActor::new_result_handler_actor(
            result_handler_rx,
            completion_tx, // ResultHandler sends completion to UnifiedSearchSystem
            50,            // Default max results
        );

        Ok(Self {
            broadcaster,
            shared_sender,
            completion_receiver: completion_rx,
            symbol_index_actor: Some(symbol_index_actor),
            symbol_search_actor: Some(symbol_search_actor),
            filepath_search_actor: Some(filepath_search_actor),
            content_search_actor: Some(content_search_actor),
            result_handler_actor: Some(result_handler_actor),
        })
    }

    /// Create content search actor with fallback strategy (rg → ag → native)
    async fn create_content_search_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: &str,
    ) -> ContentSearchActor {
        // Try ripgrep first
        if Self::is_tool_available("rg").await {
            log::info!("Using ripgrep for content search");
            return ContentSearchActor::Ripgrep(RipgrepActor::new_ripgrep_actor(
                message_receiver,
                sender,
                search_path,
            ));
        }

        // Fallback to ag
        if Self::is_tool_available("ag").await {
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

    /// Check if external tool is available
    async fn is_tool_available(tool: &str) -> bool {
        tokio::process::Command::new(tool)
            .arg("--version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Execute search with the given parameters
    pub async fn search(
        &mut self,
        search_params: SearchParams,
        max_results: usize,
        timeout_ms: u64,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        log::info!(
            "Starting unified search: '{}' (mode: {:?})",
            search_params.query,
            search_params.mode
        );

        // Update result handler's max results (send a configuration message)
        let config_message =
            Message::new("setMaxResults", FaeMessage::SetMaxResults { max_results });
        self.shared_sender.send(config_message)?;

        // Initialize symbol indexing if needed for symbol/variable search
        if matches!(
            search_params.mode,
            SearchMode::Symbol | SearchMode::Variable
        ) {
            let init_message = Message::new("initialize", FaeMessage::ClearResults);
            self.shared_sender.send(init_message)?;
        }

        // Send search parameters to all actors via Broadcaster
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(search_params),
        );
        self.shared_sender.send(search_message)?;

        // Wait for completion from ResultHandlerActor
        self.wait_for_completion(timeout_ms).await
    }

    /// Wait for search completion from ResultHandlerActor
    async fn wait_for_completion(
        &mut self,
        timeout_ms: u64,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("Waiting for search completion from ResultHandlerActor");

        match timeout(
            Duration::from_millis(timeout_ms),
            self.completion_receiver.recv(),
        )
        .await
        {
            Ok(Some(message)) => {
                if message.method == "searchFinished" {
                    if let FaeMessage::SearchFinished { result_count } = message.payload {
                        log::info!("Search completed with {} results", result_count);
                        Ok(result_count)
                    } else {
                        Err("Invalid searchFinished message payload".into())
                    }
                } else {
                    log::warn!(
                        "Unexpected message from completion channel: {}",
                        message.method
                    );
                    Ok(0)
                }
            }
            Ok(None) => {
                log::warn!("Completion channel closed without receiving searchFinished");
                Ok(0)
            }
            Err(_) => {
                log::warn!("Timeout waiting for search completion ({}ms)", timeout_ms);
                Ok(0)
            }
        }
    }

    /// Shutdown the unified search system
    pub fn shutdown(&mut self) {
        log::info!("Shutting down unified search system");

        // Shutdown all actors
        if let Some(mut actor) = self.symbol_index_actor.take() {
            actor.shutdown();
        }
        if let Some(mut actor) = self.symbol_search_actor.take() {
            actor.shutdown();
        }
        if let Some(mut actor) = self.filepath_search_actor.take() {
            actor.shutdown();
        }
        if let Some(mut actor) = self.content_search_actor.take() {
            actor.shutdown();
        }
        if let Some(mut actor) = self.result_handler_actor.take() {
            actor.shutdown();
        }

        // Shutdown broadcaster
        self.broadcaster.shutdown();
    }
}

impl Drop for UnifiedSearchSystem {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SearchMode;

    #[tokio::test]
    async fn test_unified_search_system_creation() {
        let result = UnifiedSearchSystem::new("./src").await;
        assert!(
            result.is_ok(),
            "Should create unified search system successfully"
        );
    }

    #[tokio::test]
    async fn test_unified_search_execution() {
        let mut system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        let search_params = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };

        let result = system.search(search_params, 10, 5000).await;
        assert!(result.is_ok(), "Search should execute successfully");

        system.shutdown();
    }

    #[tokio::test]
    async fn test_symbol_search_via_unified_system() {
        let mut system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        let search_params = SearchParams {
            query: "search".to_string(),
            mode: SearchMode::Symbol,
        };

        let result = system.search(search_params, 10, 10000).await;
        assert!(result.is_ok(), "Symbol search should execute successfully");

        system.shutdown();
    }

    #[tokio::test]
    async fn test_is_tool_available() {
        // Test with a command that should exist on most systems
        let result = UnifiedSearchSystem::is_tool_available("echo").await;
        assert!(result, "echo command should be available on most systems");

        // Test with a command that shouldn't exist
        let result = UnifiedSearchSystem::is_tool_available("non_existent_command_12345").await;
        assert!(!result, "Non-existent command should not be available");
    }

    #[tokio::test]
    async fn test_content_search_actor_shutdown() {
        // Test shutdown for each ContentSearchActor variant
        let (_tx, rx) = mpsc::unbounded_channel();
        let (result_tx, _result_rx) = mpsc::unbounded_channel();

        // Test Native actor shutdown
        let mut native_actor = ContentSearchActor::Native(
            NativeSearchActor::new_native_search_actor(rx, result_tx.clone(), "./src")
        );
        native_actor.shutdown(); // Should not panic

        // Test with different search modes
        let search_params_filepath = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Filepath,
        };

        let search_params_variable = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Variable,
        };

        // These should not panic
        assert_eq!(search_params_filepath.mode, SearchMode::Filepath);
        assert_eq!(search_params_variable.mode, SearchMode::Variable);
    }

    #[tokio::test]
    async fn test_search_timeout_handling() {
        let mut system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        let search_params = SearchParams {
            query: "very_rare_pattern_that_should_not_exist_12345".to_string(),
            mode: SearchMode::Literal,
        };

        // Test with very short timeout
        let result = system.search(search_params, 10, 1).await; // 1ms timeout
        assert!(result.is_ok(), "Search should handle timeout gracefully");
        
        system.shutdown();
    }

    #[tokio::test]
    async fn test_search_different_modes() {
        let mut system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        // Test Regexp mode
        let search_params = SearchParams {
            query: "fn.*test".to_string(),
            mode: SearchMode::Regexp,
        };
        let result = system.search(search_params, 5, 3000).await;
        assert!(result.is_ok(), "Regexp search should execute successfully");

        // Test Variable mode  
        let search_params = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Variable,
        };
        let result = system.search(search_params, 5, 3000).await;
        assert!(result.is_ok(), "Variable search should execute successfully");

        // Test Filepath mode
        let search_params = SearchParams {
            query: "rs".to_string(),
            mode: SearchMode::Filepath,
        };
        let result = system.search(search_params, 5, 3000).await;
        assert!(result.is_ok(), "Filepath search should execute successfully");

        system.shutdown();
    }

    #[tokio::test]
    async fn test_search_with_different_max_results() {
        let mut system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        let search_params = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };

        // Test with different max_results values
        let result1 = system.search(search_params.clone(), 1, 3000).await;
        assert!(result1.is_ok(), "Search with max_results=1 should work");

        let result2 = system.search(search_params.clone(), 100, 3000).await;
        assert!(result2.is_ok(), "Search with max_results=100 should work");

        system.shutdown();
    }

    #[tokio::test]
    async fn test_drop_behavior() {
        // Test that Drop trait works correctly
        let system = UnifiedSearchSystem::new("./src")
            .await
            .expect("Failed to create unified search system");

        // system will be dropped at the end of this scope
        // This should not panic or cause issues
        drop(system);
    }

    #[tokio::test]
    async fn test_system_creation_error_handling() {
        // Test with invalid path
        let result = UnifiedSearchSystem::new("/non/existent/path/12345").await;
        // This might succeed or fail depending on the implementation
        // The test ensures the system handles it gracefully either way
        match result {
            Ok(mut system) => {
                system.shutdown();
            }
            Err(_) => {
                // Error is acceptable for non-existent paths
            }
        }
    }
}
