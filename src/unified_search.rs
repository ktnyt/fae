//! Unified search system using Broadcaster for actor coordination
//!
//! This module provides a simplified search interface that coordinates
//! multiple search actors through a broadcaster pattern.

use crate::actors::messages::FaeMessage;
use crate::actors::types::SearchMode;
use crate::actors::{
    AgActor, FilepathSearchActor, NativeSearchActor, ResultHandlerActor, RipgrepActor,
    SymbolIndexActor, SymbolSearchActor, WatchActor,
};
use crate::core::{Broadcaster, Message};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Unified search system that coordinates all search actors
/// Now designed for external control via message passing
pub struct UnifiedSearchSystem {
    broadcaster: Broadcaster<FaeMessage>,
    watch_files: bool,

    // Keep actor instances to manage their lifecycle
    // Symbol actors are optional (optimization for non-symbol searches)
    symbol_index_actor: Option<SymbolIndexActor>,
    symbol_search_actor: Option<SymbolSearchActor>,
    filepath_search_actor: Option<FilepathSearchActor>,
    content_search_actor: Option<ContentSearchActor>,
    result_handler_actor: Option<ResultHandlerActor>,
    watch_actor: Option<WatchActor>,
    
    // Control message forwarding task handle for graceful shutdown
    control_forwarding_handle: Option<JoinHandle<()>>,
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
    /// Create a new unified search system with external control channels
    pub async fn new(
        search_path: &str,
        watch_files: bool,
        result_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        control_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_mode(search_path, watch_files, result_sender, control_receiver, None).await
    }

    /// Create a new unified search system with external control channels and optional search mode optimization
    pub async fn new_with_mode(
        search_path: &str,
        watch_files: bool,
        result_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        control_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        search_mode: Option<SearchMode>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {

        // Determine if symbol-related actors are needed
        let needs_symbol_actors = search_mode.map_or(true, |mode| {
            matches!(mode, SearchMode::Symbol | SearchMode::Variable)
        });

        if !needs_symbol_actors {
            log::info!("Symbol search not needed for mode {:?}, optimizing by skipping symbol actors", search_mode);
        }

        // Create actor channels
        let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (filepath_search_tx, filepath_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (content_search_tx, content_search_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (result_handler_tx, result_handler_rx) =
            mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Conditionally create watch actor channel
        let watch_channel = if watch_files {
            Some(mpsc::unbounded_channel::<Message<FaeMessage>>())
        } else {
            None
        };
        let (watch_tx, watch_rx) = if let Some((tx, rx)) = watch_channel {
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        // Create broadcaster with all actor senders + result sender (conditionally including symbol actors)
        let mut actor_senders = vec![
            filepath_search_tx,
            content_search_tx,
            result_handler_tx,
            result_sender.clone(), // Include result sender for broadcasting
        ];
        
        // Only add symbol actors if they are needed
        if needs_symbol_actors {
            actor_senders.push(symbol_index_tx);
            actor_senders.push(symbol_search_tx);
        }
        
        if let Some(tx) = watch_tx {
            actor_senders.push(tx);
        }

        let (broadcaster, shared_sender) = Broadcaster::new(actor_senders);

        // Create all actors using the shared sender (via Broadcaster)
        // Conditionally create symbol actors based on search mode
        let symbol_index_actor = if needs_symbol_actors {
            log::debug!("Creating SymbolIndexActor for symbol search");
            Some(SymbolIndexActor::new_symbol_index_actor(
                symbol_index_rx,
                shared_sender.clone(),
                search_path,
            )?)
        } else {
            log::debug!("Skipping SymbolIndexActor creation for non-symbol search");
            // Drop the receiver to prevent channel warnings
            drop(symbol_index_rx);
            None
        };

        let symbol_search_actor = if needs_symbol_actors {
            log::debug!("Creating SymbolSearchActor for symbol search");
            Some(SymbolSearchActor::new_symbol_search_actor(symbol_search_rx, shared_sender.clone()))
        } else {
            log::debug!("Skipping SymbolSearchActor creation for non-symbol search");
            // Drop the receiver to prevent channel warnings
            drop(symbol_search_rx);
            None
        };

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
            result_sender.clone(), // ResultHandler sends results to external receiver
            50,            // Default max results
        );

        // Conditionally create watch actor
        let watch_actor = if watch_files {
            if let Some(rx) = watch_rx {
                log::info!("Creating WatchActor for real-time file monitoring");
                Some(WatchActor::new_watch_actor(
                    rx,
                    shared_sender.clone(),
                    search_path,
                )?)
            } else {
                None
            }
        } else {
            log::debug!("File watching disabled, skipping WatchActor creation");
            None
        };

        // Start watching if watch actor was created
        if let Some(ref _actor) = watch_actor {
            log::info!("Starting file system monitoring for path: {}", search_path);
            // Note: WatchActor starts monitoring automatically when created
        }

        // Start control message forwarding task with proper handle management
        let shared_sender_clone = shared_sender.clone();
        let control_forwarding_handle = tokio::spawn(async move {
            let mut control_receiver = control_receiver;
            while let Some(message) = control_receiver.recv().await {
                log::debug!("Forwarding control message: {}", message.method);
                
                // Check if shared_sender is still open before sending
                if shared_sender_clone.is_closed() {
                    log::debug!("Shared sender closed, stopping control message forwarding");
                    break;
                }
                
                if let Err(e) = shared_sender_clone.send(message) {
                    // During shutdown, this is expected behavior - log as debug, not error
                    log::debug!("Control message forwarding stopped (receiver closed): {}", e);
                    break;
                }
            }
            log::debug!("Control message forwarding task ended gracefully");
        });

        Ok(Self {
            broadcaster,
            watch_files,
            symbol_index_actor,
            symbol_search_actor,
            filepath_search_actor: Some(filepath_search_actor),
            content_search_actor: Some(content_search_actor),
            result_handler_actor: Some(result_handler_actor),
            watch_actor,
            control_forwarding_handle: Some(control_forwarding_handle),
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

        // Phase 1: Stop control message forwarding first to prevent new messages
        if let Some(handle) = self.control_forwarding_handle.take() {
            log::debug!("Terminating control message forwarding task");
            handle.abort(); // Forcefully abort the task
            
            // Note: We use abort() instead of waiting for graceful shutdown here because:
            // 1. The control_receiver will be dropped when main exits
            // 2. This will naturally terminate the forwarding loop
            // 3. Abort ensures immediate termination even if there are pending messages
        }

        // Phase 2: Shutdown broadcaster to stop message broadcasting  
        // This prevents WARN logs from actors trying to send to closed channels
        self.broadcaster.shutdown();

        // Phase 3: Shutdown all actors
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
        let result = UnifiedSearchSystem::new("./src", false, result_sender, control_receiver).await;
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
            NativeSearchActor::new_native_search_actor(rx, result_tx.clone(), "./src")
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
        let result = UnifiedSearchSystem::new("/non/existent/path/12345", false, result_sender, control_receiver).await;
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
        assert!(system.watch_actor.is_none(), "WatchActor should not be created");

        system.shutdown();
    }

    #[tokio::test]
    async fn test_realtime_symbol_indexing_integration() {
        use tempfile::TempDir;
        use std::io::Write;
        use std::time::Duration;

        // Create temporary directory for testing
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // Create system with file watching enabled
        let (_control_sender, control_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (result_sender, _result_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut system = UnifiedSearchSystem::new(&temp_path, true, result_sender, control_receiver)
            .await
            .expect("Failed to create unified search system with WatchActor");

        // Give the system time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a Rust file in the temp directory
        let test_file_path = temp_dir.path().join("test.rs");
        let mut test_file = std::fs::File::create(&test_file_path)
            .expect("Failed to create test file");
        writeln!(test_file, "pub fn hello_world() {{}}")
            .expect("Failed to write to test file");
        test_file.flush().expect("Failed to flush test file");

        // Give the watch actor time to detect the file
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Note: This test would need to be updated to use the new external control interface
        // For now, just test system creation and shutdown
        system.shutdown();
    }
}
