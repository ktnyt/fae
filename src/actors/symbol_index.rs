//! Symbol index actor for maintaining symbol database
//!
//! This actor manages a symbol index for the entire codebase, tracking symbols
//! across all supported source files and maintaining the index as files change.

use crate::actors::messages::FaeMessage;
use crate::actors::symbol_extractor::SymbolExtractor;
use crate::actors::types::Symbol;
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use ignore::WalkBuilder;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc;

/// Symbol index handler that manages the symbol database
pub struct SymbolIndexHandler {
    search_path: String,
    symbol_extractor: SymbolExtractor,
    /// In-memory symbol index: filepath -> symbols
    symbol_index: HashMap<String, Vec<Symbol>>,
}

impl SymbolIndexHandler {
    /// Create a new SymbolIndexHandler
    pub fn new(search_path: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let symbol_extractor = SymbolExtractor::new()?;

        Ok(Self {
            search_path,
            symbol_extractor,
            symbol_index: HashMap::new(),
        })
    }

    /// Initialize symbol index by scanning all supported files
    async fn initialize_index(&mut self, controller: &ActorController<FaeMessage>) {
        log::info!("Initializing symbol index for path: {}", self.search_path);

        let search_path = self.search_path.clone();
        let controller_clone = controller.clone();

        // Perform initial indexing in a blocking task
        let result = tokio::task::spawn_blocking(move || {
            Self::scan_and_index_files(&search_path, controller_clone)
        })
        .await;

        match result {
            Ok(Ok(file_count)) => {
                log::info!(
                    "Symbol index initialization completed: {} files processed",
                    file_count
                );
            }
            Ok(Err(e)) => {
                log::error!("Symbol index initialization failed: {}", e);
            }
            Err(e) => {
                log::error!("Symbol index initialization task panicked: {}", e);
            }
        }
    }

    /// Scan directory and index all supported files
    fn scan_and_index_files(
        search_path: &str,
        controller: ActorController<FaeMessage>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut file_count = 0;
        let mut extractor = SymbolExtractor::new()?;

        // Walk through files using ignore crate
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Show hidden files by default
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global .gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .ignore(true) // Respect .ignore files
            .parents(true) // Check parent directories for ignore files
            .build();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Only process supported file types
            if !Self::is_supported_file(path) {
                continue;
            }

            let file_path_str = path.to_string_lossy().to_string();

            // Clear existing symbols for this file
            let clear_message = FaeMessage::ClearSymbolIndex(file_path_str.clone());
            if let Err(e) = tokio::runtime::Handle::current().block_on(async {
                controller
                    .send_message("clearSymbolIndex".to_string(), clear_message)
                    .await
            }) {
                log::warn!("Failed to send clearSymbolIndex message: {}", e);
                continue;
            }

            // Extract symbols from the file
            match extractor.extract_symbols_from_file(path) {
                Ok(symbols) => {
                    // Send each symbol to the index
                    for symbol in symbols {
                        let push_message = FaeMessage::PushSymbolIndex {
                            filepath: symbol.filepath.clone(),
                            line: symbol.line,
                            column: symbol.column,
                            content: symbol.content.clone(),
                            symbol_type: symbol.symbol_type,
                        };

                        if let Err(e) = tokio::runtime::Handle::current().block_on(async {
                            controller
                                .send_message("pushSymbolIndex".to_string(), push_message)
                                .await
                        }) {
                            log::warn!("Failed to send pushSymbolIndex message: {}", e);
                            break;
                        }
                    }
                    file_count += 1;
                }
                Err(e) => {
                    log::warn!("Failed to extract symbols from {}: {}", file_path_str, e);
                }
            }
        }

        Ok(file_count)
    }

    /// Check if file type is supported for symbol extraction
    fn is_supported_file(path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            matches!(extension.to_str(), Some("rs"))
        } else {
            false
        }
    }

    /// Handle file creation/update by re-indexing the file
    async fn handle_file_change(
        &mut self,
        filepath: &str,
        controller: &ActorController<FaeMessage>,
    ) {
        log::info!("Handling file change: {}", filepath);

        let path = Path::new(filepath);

        // Check if file type is supported
        if !Self::is_supported_file(path) {
            log::debug!("Skipping unsupported file type: {}", filepath);
            return;
        }

        // Clear existing symbols for this file
        let _ = controller
            .send_message(
                "clearSymbolIndex".to_string(),
                FaeMessage::ClearSymbolIndex(filepath.to_string()),
            )
            .await;

        // Re-extract symbols from the file
        match self.symbol_extractor.extract_symbols_from_file(path) {
            Ok(symbols) => {
                log::debug!("Extracted {} symbols from {}", symbols.len(), filepath);

                // Send each symbol to the index
                for symbol in symbols {
                    let push_message = FaeMessage::PushSymbolIndex {
                        filepath: symbol.filepath.clone(),
                        line: symbol.line,
                        column: symbol.column,
                        content: symbol.content.clone(),
                        symbol_type: symbol.symbol_type,
                    };

                    let _ = controller
                        .send_message("pushSymbolIndex".to_string(), push_message)
                        .await;
                }
            }
            Err(e) => {
                log::warn!("Failed to extract symbols from {}: {}", filepath, e);
            }
        }
    }

    /// Handle file deletion by clearing its symbols
    async fn handle_file_delete(
        &mut self,
        filepath: &str,
        controller: &ActorController<FaeMessage>,
    ) {
        log::info!("Handling file deletion: {}", filepath);

        // Clear symbols for the deleted file
        let _ = controller
            .send_message(
                "clearSymbolIndex".to_string(),
                FaeMessage::ClearSymbolIndex(filepath.to_string()),
            )
            .await;
    }

    /// Update internal symbol index
    fn update_symbol_index(&mut self, filepath: &str, symbol: Symbol) {
        self.symbol_index
            .entry(filepath.to_string())
            .or_insert_with(Vec::new)
            .push(symbol);
    }

    /// Clear symbols for a specific file
    fn clear_file_symbols(&mut self, filepath: &str) {
        self.symbol_index.remove(filepath);
    }

    /// Get total symbol count across all files
    pub fn total_symbol_count(&self) -> usize {
        self.symbol_index.values().map(|v| v.len()).sum()
    }

    /// Get file count in index
    pub fn indexed_file_count(&self) -> usize {
        self.symbol_index.len()
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for SymbolIndexHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &ActorController<FaeMessage>,
    ) {
        match message.method.as_str() {
            "initialize" => {
                log::info!("Initializing symbol index");
                self.initialize_index(controller).await;
            }
            "detectFileCreate" | "detectFileUpdate" => {
                if let FaeMessage::DetectFileCreate(filepath)
                | FaeMessage::DetectFileUpdate(filepath) = message.payload
                {
                    self.handle_file_change(&filepath, controller).await;
                } else {
                    log::warn!("detectFileCreate/Update received non-filepath payload");
                }
            }
            "detectFileDelete" => {
                if let FaeMessage::DetectFileDelete(filepath) = message.payload {
                    self.handle_file_delete(&filepath, controller).await;
                } else {
                    log::warn!("detectFileDelete received non-filepath payload");
                }
            }
            "clearSymbolIndex" => {
                if let FaeMessage::ClearSymbolIndex(filepath) = message.payload {
                    log::debug!("Clearing symbol index for: {}", filepath);
                    self.clear_file_symbols(&filepath);
                } else {
                    log::warn!("clearSymbolIndex received non-filepath payload");
                }
            }
            "pushSymbolIndex" => {
                if let FaeMessage::PushSymbolIndex {
                    filepath,
                    line,
                    column,
                    content,
                    symbol_type,
                } = message.payload
                {
                    let symbol = Symbol::new(filepath.clone(), line, column, content, symbol_type);
                    log::debug!(
                        "Adding symbol to index: {} ({}:{})",
                        symbol.content,
                        filepath,
                        line
                    );
                    self.update_symbol_index(&filepath, symbol);
                } else {
                    log::warn!("pushSymbolIndex received non-symbol payload");
                }
            }
            _ => {
                log::debug!("Unknown message method: {}", message.method);
            }
        }
    }
}

/// Symbol index actor for managing symbol database
pub type SymbolIndexActor = Actor<FaeMessage, SymbolIndexHandler>;

impl SymbolIndexActor {
    /// Create a new SymbolIndexActor
    pub fn new_symbol_index_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: impl Into<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let search_path_str = search_path.into();
        let handler = SymbolIndexHandler::new(search_path_str)?;

        Ok(Self::new(message_receiver, sender, handler))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_is_supported_file() {
        assert!(SymbolIndexHandler::is_supported_file(Path::new("test.rs")));
        assert!(SymbolIndexHandler::is_supported_file(Path::new(
            "/path/to/main.rs"
        )));
        assert!(!SymbolIndexHandler::is_supported_file(Path::new("test.py")));
        assert!(!SymbolIndexHandler::is_supported_file(Path::new(
            "README.md"
        )));
        assert!(!SymbolIndexHandler::is_supported_file(Path::new(
            "Cargo.toml"
        )));
    }

    #[tokio::test]
    async fn test_symbol_index_actor_creation() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, _external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let result = SymbolIndexActor::new_symbol_index_actor(actor_rx, external_tx, "./src");

        assert!(
            result.is_ok(),
            "Should create SymbolIndexActor successfully"
        );
    }

    #[tokio::test]
    async fn test_symbol_index_initialization() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolIndexActor::new_symbol_index_actor(actor_rx, external_tx, "./src")
            .expect("Failed to create actor");

        // Send initialize message
        let init_message = Message::new("initialize", FaeMessage::ClearResults); // Dummy payload
        actor_tx
            .send(init_message)
            .expect("Failed to send initialize message");

        // Wait for initialization to complete
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Check that we received symbol index messages
        let mut clear_count = 0;
        let mut push_count = 0;

        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                match msg.method.as_str() {
                    "clearSymbolIndex" => clear_count += 1,
                    "pushSymbolIndex" => push_count += 1,
                    _ => {}
                }
            } else {
                break;
            }
        }

        println!(
            "Initialization results: {} clear, {} push messages",
            clear_count, push_count
        );

        // Should have processed some Rust files in src/
        assert!(clear_count > 0, "Should have cleared some file indices");
        assert!(push_count > 0, "Should have pushed some symbols");

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_file_change_handling() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = SymbolIndexActor::new_symbol_index_actor(actor_rx, external_tx, "./src")
            .expect("Failed to create actor");

        // Send file update message for a Rust file
        let file_update = Message::new(
            "detectFileUpdate",
            FaeMessage::DetectFileUpdate("./src/actors/types.rs".to_string()),
        );
        actor_tx
            .send(file_update)
            .expect("Failed to send file update message");

        // Wait for processing
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Check that we received symbol index messages
        let mut received_clear = false;
        let mut received_push = false;

        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                match msg.method.as_str() {
                    "clearSymbolIndex" => {
                        if let FaeMessage::ClearSymbolIndex(filepath) = msg.payload {
                            if filepath.contains("types.rs") {
                                received_clear = true;
                            }
                        }
                    }
                    "pushSymbolIndex" => {
                        if let FaeMessage::PushSymbolIndex { filepath, .. } = msg.payload {
                            if filepath.contains("types.rs") {
                                received_push = true;
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                break;
            }
        }

        assert!(
            received_clear,
            "Should have received clearSymbolIndex for types.rs"
        );
        assert!(
            received_push,
            "Should have received pushSymbolIndex for types.rs"
        );

        // Clean up
        actor.shutdown();
    }
}
