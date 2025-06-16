//! Unified search system using Broadcaster for actor coordination
//!
//! This module provides a simplified search interface that coordinates
//! multiple search actors through a broadcaster pattern.

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams};
use crate::actors::{
    AgActor, FilepathSearchActor, NativeSearchActor, RipgrepActor, SymbolIndexActor,
    SymbolSearchActor,
};
use crate::core::{Broadcaster, Message};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Unified search system that coordinates all search actors
pub struct UnifiedSearchSystem {
    broadcaster: Broadcaster<FaeMessage>,
    shared_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    result_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    
    // Store search parameters for delayed execution during symbol indexing
    pending_search_params: Option<SearchParams>,
    
    // Keep actor instances to manage their lifecycle
    symbol_index_actor: Option<SymbolIndexActor>,
    symbol_search_actor: Option<SymbolSearchActor>,
    filepath_search_actor: Option<FilepathSearchActor>,
    content_search_actor: Option<ContentSearchActor>,
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
        // Create individual result channels for each actor to avoid competition
        let (result_tx, result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_index_result_tx, symbol_index_result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_result_tx, symbol_search_result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (filepath_result_tx, filepath_result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (content_result_tx, content_result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create actor channels
        let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (filepath_search_tx, filepath_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (content_search_tx, content_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create actors with individual result channels
        let symbol_index_actor = SymbolIndexActor::new_symbol_index_actor(
            symbol_index_rx,
            symbol_index_result_tx.clone(),
            search_path,
        )?;

        let symbol_search_actor = SymbolSearchActor::new_symbol_search_actor(
            symbol_search_rx,
            symbol_search_result_tx.clone(),
        );

        let filepath_search_actor = FilepathSearchActor::new_filepath_search_actor(
            filepath_search_rx,
            filepath_result_tx.clone(),
            search_path,
        );

        let content_search_actor = Self::create_content_search_actor(
            content_search_rx,
            content_result_tx.clone(),
            search_path,
        ).await;

        // Create broadcaster with all actor senders
        let actor_senders = vec![
            symbol_index_tx,
            symbol_search_tx,
            filepath_search_tx,
            content_search_tx,
        ];

        let (broadcaster, shared_sender) = Broadcaster::new(actor_senders);

        // Start result distribution tasks for each actor's result channel
        let result_distributor_tx = result_tx.clone();
        Self::start_result_distribution(symbol_index_result_rx, result_distributor_tx.clone());
        Self::start_result_distribution(symbol_search_result_rx, result_distributor_tx.clone());
        Self::start_result_distribution(filepath_result_rx, result_distributor_tx.clone());
        Self::start_result_distribution(content_result_rx, result_distributor_tx.clone());

        Ok(Self {
            broadcaster,
            shared_sender,
            result_receiver: result_rx,
            pending_search_params: None,
            symbol_index_actor: Some(symbol_index_actor),
            symbol_search_actor: Some(symbol_search_actor),
            filepath_search_actor: Some(filepath_search_actor),
            content_search_actor: Some(content_search_actor),
        })
    }

    /// Start result distribution from an individual actor's result channel to the unified channel
    fn start_result_distribution(
        mut actor_result_rx: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        unified_result_tx: mpsc::UnboundedSender<Message<FaeMessage>>,
    ) {
        tokio::spawn(async move {
            log::debug!("Result distribution task started");
            while let Some(message) = actor_result_rx.recv().await {
                log::debug!("Distribution task forwarding message: {}", message.method);
                if let Err(e) = unified_result_tx.send(message) {
                    log::warn!("Failed to distribute result message: {}", e);
                    break;
                }
            }
            log::debug!("Result distribution task terminated");
        });
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

        // Initialize symbol indexing if needed for symbol/variable search
        if matches!(search_params.mode, SearchMode::Symbol | SearchMode::Variable) {
            let init_message = Message::new("initialize", FaeMessage::ClearResults);
            self.shared_sender.send(init_message)?;
            
            // For symbol search, delay sending search params until indexing is complete
            // The search parameters will be sent when we receive the final completeSymbolIndex
            // Store the search params for later use
            self.pending_search_params = Some(search_params);
        } else {
            // For non-symbol searches, send search parameters immediately
            let search_message = Message::new(
                "updateSearchParams",
                FaeMessage::UpdateSearchParams(search_params),
            );
            self.shared_sender.send(search_message)?;
        }

        // Collect results
        self.collect_results(max_results, timeout_ms).await
    }

    /// Collect search results from all actors
    async fn collect_results(
        &mut self,
        max_results: usize,
        timeout_ms: u64,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut result_count = 0;
        let mut search_completed = false;
        let mut search_started = false;

        while result_count < max_results && !search_completed {
            // Use a longer timeout initially for symbol indexing, reasonable timeout for results
            let current_timeout = if search_started { 3000 } else { timeout_ms };
            match timeout(Duration::from_millis(current_timeout), self.result_receiver.recv()).await {
                Ok(Some(message)) => {
                    log::debug!("Received message in collect_results: {}", message.method);
                    match message.method.as_str() {
                        "pushSearchResult" => {
                            if let FaeMessage::PushSearchResult(result) = message.payload {
                                search_started = true;
                                result_count += 1;
                                log::debug!("Received search result #{}: {}", result_count, result.content);
                                println!(
                                    "{}:{}:{}: {}",
                                    result.filename, result.line, result.column, result.content
                                );
                            }
                        }
                        "completeSearch" => {
                            log::info!("Search completed notification received, {} results collected so far", result_count);
                            // Process remaining messages for a reasonable time before marking as completed
                            let mut remaining_messages = 0;
                            let end_time = tokio::time::Instant::now() + tokio::time::Duration::from_millis(500);
                            
                            while tokio::time::Instant::now() < end_time {
                                match tokio::time::timeout(
                                    tokio::time::Duration::from_millis(50), 
                                    self.result_receiver.recv()
                                ).await {
                                    Ok(Some(msg)) => {
                                        if msg.method == "pushSearchResult" {
                                            if let FaeMessage::PushSearchResult(result) = msg.payload {
                                                result_count += 1;
                                                remaining_messages += 1;
                                                log::debug!("Received remaining search result #{}: {}", result_count, result.content);
                                                println!(
                                                    "{}:{}:{}: {}",
                                                    result.filename, result.line, result.column, result.content
                                                );
                                            }
                                        }
                                    }
                                    _ => break, // No more messages or timeout
                                }
                            }
                            
                            log::info!("Processed {} additional results after completion notification", remaining_messages);
                            search_completed = true;
                        }
                        "clearSymbolIndex" | "pushSymbolIndex" => {
                            // Forward symbol index messages via broadcaster for proper coordination
                            if let Err(e) = self.shared_sender.send(message) {
                                log::warn!("Failed to forward symbol index message: {}", e);
                            }
                        }
                        "completeSymbolIndex" => {
                            // Forward completion message 
                            if let Err(e) = self.shared_sender.send(message.clone()) {
                                log::warn!("Failed to forward complete symbol index message: {}", e);
                            }
                            
                            // Check if this is the final completion (all files processed)
                            if let FaeMessage::CompleteSymbolIndex(ref filepath) = message.payload {
                                if filepath == "all_files" && self.pending_search_params.is_some() {
                                    // This is the signal that all files have been processed
                                    if let Some(search_params) = self.pending_search_params.take() {
                                        log::info!("Sending delayed search parameters after all symbol indexing completion");
                                        let search_message = Message::new(
                                            "updateSearchParams",
                                            FaeMessage::UpdateSearchParams(search_params),
                                        );
                                        if let Err(e) = self.shared_sender.send(search_message) {
                                            log::error!("Failed to send delayed search parameters: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            log::debug!("Received unhandled message: {}", message.method);
                        }
                    }
                }
                Ok(None) => {
                    log::debug!("Result receiver channel closed");
                    break;
                }
                Err(_) => {
                    log::debug!("Timeout waiting for search results or completion (timeout: {}ms, results: {}, completed: {})", 
                                current_timeout, result_count, search_completed);
                    if result_count == 0 && !search_completed {
                        // Continue waiting for results
                        continue;
                    }
                    break;
                }
            }
        }

        Ok(result_count)
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
        assert!(result.is_ok(), "Should create unified search system successfully");
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
}