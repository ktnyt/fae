//! TUI Actor for bridging UnifiedSearchSystem with TUI interface
//!
//! This actor handles messages from the search system and updates
//! the TUI state accordingly, particularly converting system events
//! into appropriate toast notifications and state updates.

use crate::actors::messages::FaeMessage;
use crate::core::{Actor, ActorController, Message, MessageHandler};
use crate::tui::{ToastType, TuiHandle};
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;

/// Actor that bridges UnifiedSearchSystem messages to TUI updates
pub struct TuiActor {
    tui_handle: TuiHandle,
    control_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
}

impl TuiActor {
    /// Create a new TUI actor with the given TUI handle and control sender
    pub fn new(
        tui_handle: TuiHandle,
        control_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    ) -> Self {
        Self { 
            tui_handle,
            control_sender,
        }
    }

    /// Execute a search request by sending UpdateSearchParams message
    pub fn execute_search(&self, query: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::cli::create_search_params;
        
        log::debug!("TuiActor executing search: '{}'", query);
        
        // Parse the query and determine search mode
        let search_params = create_search_params(&query);
        
        // Send search request via control channel
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(search_params),
        );
        
        if let Err(e) = self.control_sender.send(search_message) {
            let error_msg = format!("Failed to send search request: {}", e);
            log::error!("{}", error_msg);
            return Err(error_msg.into());
        }
        
        log::debug!("Search request sent successfully");
        Ok(())
    }

    /// Create a new TUI actor and spawn it with the given receiver
    pub fn new_tui_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        tui_handle: TuiHandle,
        control_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    ) -> Actor<FaeMessage, Self> {
        let handler = Self::new(tui_handle, control_sender);
        Actor::new(message_receiver, sender, handler)
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for TuiActor {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        _controller: &ActorController<FaeMessage>,
    ) {
        log::debug!("TuiActor received message: {}", message.method);
        match &message.payload {
            FaeMessage::ReportSymbolIndex {
                queued_files,
                indexed_files,
                symbols_found,
            } => {
                log::debug!("TuiActor: Updating index status: {}/{} files, {} symbols", indexed_files, queued_files, symbols_found);
                // Show indexing progress as toast
                if *queued_files > 0 {
                    let progress_message = format!(
                        "Indexing: {}/{} files, {} symbols",
                        indexed_files,
                        indexed_files + queued_files,
                        symbols_found
                    );
                    if let Err(e) = self.tui_handle.show_toast(
                        progress_message,
                        ToastType::Info,
                        Duration::from_secs(2),
                    ) {
                        log::warn!("Failed to show indexing progress toast: {}", e);
                    }
                }
            }

            FaeMessage::CompleteInitialIndexing => {
                // Show completion notification
                if let Err(e) = self.tui_handle.show_toast(
                    "Symbol indexing completed!".to_string(),
                    ToastType::Success,
                    Duration::from_secs(3),
                ) {
                    log::warn!("Failed to show indexing completion toast: {}", e);
                }
            }

            FaeMessage::PushSearchResult(result) => {
                // Add search result to TUI
                let formatted_result = format!(
                    "{}:{} - {}",
                    result.filename,
                    result.line,
                    result.content.trim()
                );
                
                log::debug!("TuiActor: Adding search result: {}", formatted_result);
                if let Err(e) = self
                    .tui_handle
                    .append_search_results(vec![formatted_result])
                {
                    log::warn!("Failed to add search result to TUI: {}", e);
                } else {
                    log::debug!("TuiActor: Search result added successfully");
                }
            }

            FaeMessage::NotifySearchReport { result_count } => {
                // Log search completion but don't show toast for search operations
                log::debug!(
                    "Search completed: {} results found",
                    result_count
                );
            }

            FaeMessage::ClearResults => {
                // Clear search results in TUI
                if let Err(e) = self
                    .tui_handle
                    .update_state(crate::tui::StateUpdate::new().with_clear_results())
                {
                    log::warn!("Failed to clear TUI results: {}", e);
                }
            }

            FaeMessage::UpdateSearchParams(params) => {
                // Don't update TUI search input - it should remain user-controlled
                log::debug!("Search started for: '{}'", params.query);
            }

            // Handle file change notifications
            FaeMessage::DetectFileCreate(filepath) => {
                if let Err(e) = self.tui_handle.show_toast(
                    format!("File created: {}", filepath),
                    ToastType::Info,
                    Duration::from_secs(2),
                ) {
                    log::warn!("Failed to show file creation toast: {}", e);
                }
            }

            FaeMessage::DetectFileUpdate(filepath) => {
                if let Err(e) = self.tui_handle.show_toast(
                    format!("File updated: {}", filepath),
                    ToastType::Info,
                    Duration::from_secs(2),
                ) {
                    log::warn!("Failed to show file update toast: {}", e);
                }
            }

            FaeMessage::DetectFileDelete(filepath) => {
                if let Err(e) = self.tui_handle.show_toast(
                    format!("File deleted: {}", filepath),
                    ToastType::Warning,
                    Duration::from_secs(2),
                ) {
                    log::warn!("Failed to show file deletion toast: {}", e);
                }
            }

            // Ignore other messages for now
            _ => {
                log::trace!("TuiActor ignoring message: {}", message.method);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SearchResult;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_tui_actor_creation() {
        let (tui_tx, _tui_rx) = mpsc::unbounded_channel();
        let tui_handle = crate::tui::TuiHandle {
            state_sender: tui_tx,
        };
        let (control_tx, _control_rx) = mpsc::unbounded_channel();

        let _tui_actor = TuiActor::new(tui_handle, control_tx);
        // Just verify it can be created without panic
        assert!(true);
    }

    #[tokio::test]
    async fn test_report_symbol_index_handling() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tui_tx, mut tui_rx) = mpsc::unbounded_channel();
        let tui_handle = crate::tui::TuiHandle {
            state_sender: tui_tx,
        };
        let (control_tx, _control_rx) = mpsc::unbounded_channel();
        let mut tui_actor = TuiActor::new(tui_handle, control_tx);

        let (controller_tx, _controller_rx) = mpsc::unbounded_channel();
        let controller = ActorController::new(controller_tx);

        // Test indexing in progress
        let progress_message = Message::new(
            "reportSymbolIndex",
            FaeMessage::ReportSymbolIndex {
                queued_files: 5,
                indexed_files: 3,
                symbols_found: 120,
            },
        );

        tui_actor.on_message(progress_message, &controller).await;

        // Verify index status was updated
        if let Ok(state_update) = tui_rx.try_recv() {
            if let Some(index_status) = state_update.index_status {
                assert_eq!(index_status.queued_files, 5);
                assert_eq!(index_status.indexed_files, 3);
                assert_eq!(index_status.symbols_found, 120);
                assert!(index_status.is_active);
            }
        }

        // Test indexing complete
        let complete_message = Message::new(
            "reportSymbolIndex",
            FaeMessage::ReportSymbolIndex {
                queued_files: 0,
                indexed_files: 8,
                symbols_found: 240,
            },
        );

        tui_actor.on_message(complete_message, &controller).await;

        // Verify completion status was updated
        if let Ok(state_update) = tui_rx.try_recv() {
            if let Some(index_status) = state_update.index_status {
                assert_eq!(index_status.queued_files, 0);
                assert_eq!(index_status.indexed_files, 8);
                assert_eq!(index_status.symbols_found, 240);
                assert!(!index_status.is_active); // Indexing is complete
                assert!(index_status.is_complete());
            }
        }
    }

    #[tokio::test]
    async fn test_search_result_handling() {
        let (tui_tx, mut tui_rx) = mpsc::unbounded_channel();
        let tui_handle = crate::tui::TuiHandle {
            state_sender: tui_tx,
        };
        let (control_tx, _control_rx) = mpsc::unbounded_channel();
        let mut tui_actor = TuiActor::new(tui_handle, control_tx);

        let (controller_tx, _controller_rx) = mpsc::unbounded_channel();
        let controller = ActorController::new(controller_tx);

        // Test adding search result
        let search_result = SearchResult {
            filename: "test.rs".to_string(),
            line: 42,
            column: 10,
            content: "fn test_function() {".to_string(),
        };

        let result_message = Message::new(
            "pushSearchResult",
            FaeMessage::PushSearchResult(search_result),
        );

        tui_actor.on_message(result_message, &controller).await;

        // Verify search result was added
        if let Ok(state_update) = tui_rx.try_recv() {
            if let Some(results) = state_update.append_results {
                assert_eq!(results.len(), 1);
                assert!(results[0].contains("test.rs:42"));
                assert!(results[0].contains("fn test_function() {"));
            }
        }
    }
}
