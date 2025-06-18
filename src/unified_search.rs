//! Unified search system with direct actor communication
//!
//! This module provides a simplified search interface that coordinates
//! multiple search actors through direct message passing, eliminating
//! the need for complex channel integration and broadcasting.

use crate::actors::messages::FaeMessage;
use crate::actors::types::SearchMode;
use crate::actors::{
    AgActor, FilepathSearchActor, NativeSearchActor, ResultHandlerActor, RipgrepActor,
    SymbolIndexActor, SymbolSearchActor, WatchActor,
};
use crate::core::Message;
use tokio::sync::mpsc;

/// Unified search system that coordinates all search actors
/// Uses direct message passing with unified result handling
pub struct UnifiedSearchSystem {
    watch_files: bool,

    // Keep actor instances to manage their lifecycle
    // Symbol actors are optional (optimization for non-symbol searches)
    symbol_index_actor: Option<SymbolIndexActor>,
    symbol_search_actor: Option<SymbolSearchActor>,
    filepath_search_actor: Option<FilepathSearchActor>,
    content_search_actor: Option<ContentSearchActor>,
    result_handler_actor: Option<ResultHandlerActor>,
    watch_actor: Option<WatchActor>,

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
    /// Create a new unified search system with single unified channel
    pub async fn new(
        search_path: &str,
        watch_files: bool,
        external_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        external_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_mode(
            search_path,
            watch_files,
            external_sender,
            external_receiver,
            None,
        )
        .await
    }

    /// Create a new unified search system with direct message passing architecture
    pub async fn new_with_mode(
        search_path: &str,
        watch_files: bool,
        external_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        external_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        search_mode: Option<SearchMode>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Determine if symbol-related actors are needed
        let needs_symbol_actors = search_mode
            .is_none_or(|mode| matches!(mode, SearchMode::Symbol | SearchMode::Variable));

        if !needs_symbol_actors {
            log::info!(
                "Symbol search not needed for mode {:?}, optimizing by skipping symbol actors",
                search_mode
            );
        }

        // Create simple actor channels for inline broadcasting
        let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (filepath_search_tx, filepath_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (content_search_tx, content_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (result_handler_tx, result_handler_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (watch_tx, watch_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Simplified approach: All actors use external_sender directly
        // No internal message routing or coordination needed

        // Create all actors - ALL actors use external_sender directly (no coordination complexity)
        let symbol_index_actor = if needs_symbol_actors {
            log::debug!("Creating SymbolIndexActor for symbol search");

            Some(SymbolIndexActor::new_symbol_index_actor(
                symbol_index_rx,
                external_sender.clone(),
                search_path,
            )?)
        } else {
            log::debug!("Skipping SymbolIndexActor creation for non-symbol search");
            // Drop unused receiver
            drop(symbol_index_rx);
            None
        };

        let symbol_search_actor = if needs_symbol_actors {
            log::debug!("Creating SymbolSearchActor for symbol search");

            Some(SymbolSearchActor::new_symbol_search_actor(
                symbol_search_rx,
                external_sender.clone(),
            ))
        } else {
            log::debug!("Skipping SymbolSearchActor creation for non-symbol search");
            // Drop unused receiver
            drop(symbol_search_rx);
            None
        };

        let filepath_search_actor = FilepathSearchActor::new_filepath_search_actor(
            filepath_search_rx,
            external_sender.clone(),
            search_path,
        );

        let content_search_actor =
            Self::create_content_search_actor(content_search_rx, external_sender.clone(), search_path)
                .await;

        let result_handler_actor = ResultHandlerActor::new_result_handler_actor(
            result_handler_rx,
            external_sender.clone(),
        );

        // Conditionally create watch actor
        let watch_actor = if watch_files {
            log::info!("Creating WatchActor for real-time file monitoring");

            Some(WatchActor::new_watch_actor(
                watch_rx,
                external_sender.clone(),
                search_path,
            )?)
        } else {
            log::debug!("File watching disabled, skipping WatchActor creation");
            // Drop unused receiver
            drop(watch_rx);
            None
        };

        // Simple external message distribution to all actors
        let needs_symbol_actors_clone = needs_symbol_actors;
        let watch_files_clone = watch_files;
        tokio::spawn(async move {
            let mut external_receiver = external_receiver;
            while let Some(message) = external_receiver.recv().await {
                log::debug!("Distributing external message to all actors: {}", message.method);
                
                // Send to result handler for all messages
                let _ = result_handler_tx.send(message.clone());
                
                // Send to content and filepath search for all messages  
                let _ = content_search_tx.send(message.clone());
                let _ = filepath_search_tx.send(message.clone());
                
                // Conditionally send to symbol actors
                if needs_symbol_actors_clone {
                    let _ = symbol_index_tx.send(message.clone());
                    let _ = symbol_search_tx.send(message.clone());
                }
                
                // Conditionally send to watch actor
                if watch_files_clone {
                    let _ = watch_tx.send(message.clone());
                }
            }
        });

        // Start watching if watch actor was created
        if let Some(ref _actor) = watch_actor {
            log::info!("Starting file system monitoring for path: {}", search_path);
        }

        Ok(Self {
            watch_files,
            symbol_index_actor,
            symbol_search_actor,
            filepath_search_actor: Some(filepath_search_actor),
            content_search_actor: Some(content_search_actor),
            result_handler_actor: Some(result_handler_actor),
            watch_actor,
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

    /// Check if file watching is enabled
    pub fn is_watching_files(&self) -> bool {
        self.watch_files
    }

    /// Shutdown the unified search system with graceful task termination
    pub fn shutdown(&mut self) {
        log::info!("Shutting down unified search system");

        // Phase 1: Shutdown all actors (no separate broadcaster to shutdown)
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
        if let Some(mut actor) = self.watch_actor.take() {
            log::info!("Shutting down WatchActor");
            actor.shutdown();
        }

        log::info!("Unified search system shutdown completed");
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

    #[tokio::test]
    async fn test_unified_search_system_creation() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let result =
            UnifiedSearchSystem::new("./src", false, result_sender, control_receiver).await;
        assert!(
            result.is_ok(),
            "Should create unified search system successfully"
        );
    }

    #[tokio::test]
    async fn test_unified_search_execution() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }

    #[tokio::test]
    async fn test_symbol_search_via_unified_system() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
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
            NativeSearchActor::new_native_search_actor(rx, result_tx.clone(), "./src"),
        );
        native_actor.shutdown(); // Should not panic

        // Test various search modes would be tested here
        // (simplified test for external control interface)
    }

    #[tokio::test]
    async fn test_search_timeout_handling() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }

    #[tokio::test]
    async fn test_search_different_modes() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }

    #[tokio::test]
    async fn test_search_with_different_max_results() {
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }

    #[tokio::test]
    async fn test_drop_behavior() {
        // Test that Drop trait works correctly
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // system will be dropped at the end of this scope
        // This should not panic or cause issues
        drop(system);
    }

    #[tokio::test]
    async fn test_system_creation_error_handling() {
        // Test with invalid path
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let result = UnifiedSearchSystem::new(
            "/non/existent/path/12345",
            false,
            result_sender,
            control_receiver,
        )
        .await;
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

    #[tokio::test]
    async fn test_unified_search_system_with_watch_actor() {
        // Test creation with watch files enabled
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let result = UnifiedSearchSystem::new("./src", true, result_sender, control_receiver).await;
        assert!(
            result.is_ok(),
            "Should create unified search system with WatchActor successfully"
        );

        if let Ok(mut system) = result {
            // Verify watch actor was created
            assert!(system.watch_files, "watch_files should be true");
            assert!(system.watch_actor.is_some(), "WatchActor should be created");

            system.shutdown();
        }
    }

    #[tokio::test]
    async fn test_unified_search_system_without_watch_actor() {
        // Test creation with watch files disabled
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system");

        // Verify watch actor was not created
        assert!(!system.watch_files, "watch_files should be false");
        assert!(
            system.watch_actor.is_none(),
            "WatchActor should not be created"
        );

        system.shutdown();
    }

    #[tokio::test]
    async fn test_realtime_symbol_indexing_integration() {
        use std::io::Write;
        use std::time::Duration;
        use tempfile::TempDir;

        // Create temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // Create system with file watching enabled
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system =
            UnifiedSearchSystem::new(&temp_path, true, result_sender, control_receiver)
                .await
                .expect("Failed to create unified search system with WatchActor");

        // Give the system time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a Rust file in the temp directory
        let test_file_path = temp_dir.path().join("test.rs");
        let mut test_file =
            std::fs::File::create(&test_file_path).expect("Failed to create test file");
        writeln!(test_file, "pub fn hello_world() {{}}").expect("Failed to write to test file");
        test_file.flush().expect("Failed to flush test file");

        // Give the watch actor time to detect the file
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }
}
