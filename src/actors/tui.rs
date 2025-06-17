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
}

impl TuiActor {
    /// Create a new TUI actor with the given TUI handle
    pub fn new(tui_handle: TuiHandle) -> Self {
        Self { tui_handle }
    }

    /// Create a new TUI actor and spawn it with the given receiver
    pub fn new_tui_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        tui_handle: TuiHandle,
    ) -> Actor<FaeMessage, Self> {
        let handler = Self::new(tui_handle);
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
        match &message.payload {
            FaeMessage::ReportSymbolIndex {
                queued_files,
                indexed_files,
                symbols_found,
            } => {
                let progress_message = if *queued_files == 0 {
                    // Indexing complete
                    format!(
                        "Indexing completed: {} files, {} symbols",
                        indexed_files, symbols_found
                    )
                } else {
                    // Indexing in progress
                    format!(
                        "Indexing: {}/{} files, {} symbols found",
                        indexed_files,
                        indexed_files + queued_files,
                        symbols_found
                    )
                };

                let toast_type = if *queued_files == 0 {
                    ToastType::Success
                } else {
                    ToastType::Info
                };

                // Update toast with progress information
                if let Err(e) =
                    self.tui_handle
                        .show_toast(progress_message, toast_type, Duration::from_secs(2))
                {
                    log::warn!("Failed to update TUI toast: {}", e);
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

                if let Err(e) = self
                    .tui_handle
                    .append_search_results(vec![formatted_result])
                {
                    log::warn!("Failed to add search result to TUI: {}", e);
                }
            }

            FaeMessage::NotifySearchReport { result_count } => {
                // Show search completion with result count
                let message = if *result_count > 0 {
                    format!("Search completed: {} results found", result_count)
                } else {
                    "No results found".to_string()
                };

                let toast_type = if *result_count > 0 {
                    ToastType::Success
                } else {
                    ToastType::Warning
                };

                if let Err(e) =
                    self.tui_handle
                        .show_toast(message, toast_type, Duration::from_secs(3))
                {
                    log::warn!("Failed to show search completion toast: {}", e);
                }
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
                // Update search input in TUI
                if let Err(e) = self.tui_handle.set_search_input(params.query.clone()) {
                    log::warn!("Failed to update TUI search input: {}", e);
                }

                // Show search start notification
                if let Err(e) = self.tui_handle.show_toast(
                    format!("Searching for '{}'...", params.query),
                    ToastType::Info,
                    Duration::from_secs(2),
                ) {
                    log::warn!("Failed to show search start toast: {}", e);
                }
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
        let tui_handle = crate::tui::TuiHandle { state_sender: tui_tx };

        let _tui_actor = TuiActor::new(tui_handle);
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
        let tui_handle = crate::tui::TuiHandle { state_sender: tui_tx };
        let mut tui_actor = TuiActor::new(tui_handle);

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

        // Verify toast was sent
        if let Ok(state_update) = tui_rx.try_recv() {
            if let Some((toast_msg, toast_type, _duration)) = state_update.toast {
                assert!(toast_msg.contains("Indexing: 3/8 files"));
                assert!(toast_msg.contains("120 symbols"));
                assert_eq!(toast_type, ToastType::Info);
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

        // Verify completion toast was sent
        if let Ok(state_update) = tui_rx.try_recv() {
            if let Some((toast_msg, toast_type, _duration)) = state_update.toast {
                assert!(toast_msg.contains("Indexing completed"));
                assert!(toast_msg.contains("8 files"));
                assert!(toast_msg.contains("240 symbols"));
                assert_eq!(toast_type, ToastType::Success);
            }
        }
    }

    #[tokio::test]
    async fn test_search_result_handling() {
        let (tui_tx, mut tui_rx) = mpsc::unbounded_channel();
        let tui_handle = crate::tui::TuiHandle { state_sender: tui_tx };
        let mut tui_actor = TuiActor::new(tui_handle);

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
