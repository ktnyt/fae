//! Symbol index generation actor
//!
//! This actor is responsible for generating symbol index data by extracting symbols
//! from source files and broadcasting them as messages. It does not maintain any
//! internal index state - that responsibility belongs to SymbolSearchActor.

use crate::actors::messages::FaeMessage;
use crate::actors::symbol_extractor::SymbolExtractor;
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Symbol index generation handler that extracts and broadcasts symbols
pub struct SymbolIndexHandler {
    search_path: String,
    /// Track files currently being processed to handle race conditions
    processing_files: Arc<Mutex<HashSet<String>>>,
    /// Track ongoing processing tasks for potential cancellation
    processing_tasks: Arc<Mutex<Vec<AbortHandle>>>,
}

impl SymbolIndexHandler {
    /// Create a new SymbolIndexHandler
    pub fn new(search_path: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            search_path,
            processing_files: Arc::new(Mutex::new(HashSet::new())),
            processing_tasks: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Initialize symbol generation by scanning all supported files
    async fn initialize_index(&mut self, controller: &ActorController<FaeMessage>) {
        log::info!("Starting symbol generation for path: {}", self.search_path);

        let search_path = self.search_path.clone();
        let controller_clone = controller.clone();
        let processing_files_clone = self.processing_files.clone();

        // Perform initial indexing in a blocking task
        let result = tokio::task::spawn_blocking(move || {
            Self::scan_and_index_files(&search_path, controller_clone, processing_files_clone)
        })
        .await;

        match result {
            Ok(Ok(file_count)) => {
                log::info!(
                    "Symbol generation completed: {} files processed",
                    file_count
                );
            }
            Ok(Err(e)) => {
                log::error!("Symbol generation failed: {}", e);
            }
            Err(e) => {
                log::error!("Symbol generation task panicked: {}", e);
            }
        }
    }

    /// Scan directory and broadcast symbols from all supported files
    fn scan_and_index_files(
        search_path: &str,
        controller: ActorController<FaeMessage>,
        processing_files: Arc<Mutex<HashSet<String>>>,
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

            // Mark file as being processed
            {
                let mut files = processing_files.lock().unwrap();
                files.insert(file_path_str.clone());
            }

            // Clear existing symbols for this file
            let clear_message = FaeMessage::ClearSymbolIndex(file_path_str.clone());
            if let Err(e) = tokio::runtime::Handle::current().block_on(async {
                controller
                    .send_message("clearSymbolIndex".to_string(), clear_message)
                    .await
            }) {
                log::warn!("Failed to send clearSymbolIndex message: {}", e);
                // Remove from processing set on error
                let mut files = processing_files.lock().unwrap();
                files.remove(&file_path_str);
                continue;
            }

            // Extract symbols from the file
            match extractor.extract_symbols_from_file(path) {
                Ok(symbols) => {
                    // Broadcast each symbol, checking for interruption
                    for symbol in symbols {
                        // Check if processing was interrupted
                        let still_processing = {
                            let files = processing_files.lock().unwrap();
                            files.contains(&file_path_str)
                        };

                        if !still_processing {
                            log::info!("Processing of {} was interrupted during initialization", file_path_str);
                            break;
                        }

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
                    
                    // Send completion notification for this file
                    let complete_message = FaeMessage::CompleteSymbolIndex(file_path_str.clone());
                    if let Err(e) = tokio::runtime::Handle::current().block_on(async {
                        controller
                            .send_message("completeSymbolIndex".to_string(), complete_message)
                            .await
                    }) {
                        log::warn!("Failed to send completeSymbolIndex message: {}", e);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to extract symbols from {}: {}", file_path_str, e);
                }
            }

            // Remove from processing set when done (or interrupted)
            {
                let mut files = processing_files.lock().unwrap();
                files.remove(&file_path_str);
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

        // Check if this file is currently being processed
        let is_processing = {
            let processing_files = self.processing_files.lock().unwrap();
            processing_files.contains(filepath)
        };

        if is_processing {
            log::info!("File {} is currently being processed, interrupting previous processing", filepath);
            self.interrupt_file_processing(filepath).await;
        }

        // Mark file as being processed
        {
            let mut processing_files = self.processing_files.lock().unwrap();
            processing_files.insert(filepath.to_string());
        }

        // Clear existing symbols for this file
        let _ = controller
            .send_message(
                "clearSymbolIndex".to_string(),
                FaeMessage::ClearSymbolIndex(filepath.to_string()),
            )
            .await;

        // Process file in an abortable task
        let filepath_clone = filepath.to_string();
        let path_clone = path.to_path_buf();
        let controller_clone = controller.clone();
        let processing_files_clone = self.processing_files.clone();
        
        let handle = tokio::spawn(async move {
            Self::process_file_symbols(
                &filepath_clone,
                &path_clone,
                controller_clone,
                processing_files_clone,
            ).await
        });

        // Store the abort handle
        {
            let mut processing_tasks = self.processing_tasks.lock().unwrap();
            processing_tasks.push(handle.abort_handle());
        }

        // Wait for the task to complete
        let _ = handle.await;
    }

    /// Process file symbols in an abortable way
    async fn process_file_symbols(
        filepath: &str,
        path: &std::path::PathBuf,
        controller: ActorController<FaeMessage>,
        processing_files: Arc<Mutex<HashSet<String>>>,
    ) {
        // Use a new extractor instance for this task
        let mut extractor = match SymbolExtractor::new() {
            Ok(extractor) => extractor,
            Err(e) => {
                log::error!("Failed to create symbol extractor: {}", e);
                // Remove from processing set before returning
                {
                    let mut files = processing_files.lock().unwrap();
                    files.remove(filepath);
                }
                return;
            }
        };

        // Extract symbols from the file
        match extractor.extract_symbols_from_file(path) {
            Ok(symbols) => {
                log::debug!("Extracted {} symbols from {}", symbols.len(), filepath);

                // Broadcast each symbol
                for symbol in symbols {
                    // Check if processing was interrupted
                    let still_processing = {
                        let files = processing_files.lock().unwrap();
                        files.contains(filepath)
                    };

                    if !still_processing {
                        log::info!("Processing of {} was interrupted, stopping symbol broadcast", filepath);
                        return;
                    }

                    let push_message = FaeMessage::PushSymbolIndex {
                        filepath: symbol.filepath.clone(),
                        line: symbol.line,
                        column: symbol.column,
                        content: symbol.content.clone(),
                        symbol_type: symbol.symbol_type,
                    };

                    if let Err(e) = controller
                        .send_message("pushSymbolIndex".to_string(), push_message)
                        .await {
                        log::warn!("Failed to send pushSymbolIndex message: {}", e);
                        break;
                    }
                }

                log::debug!("Successfully processed symbols for {}", filepath);
                
                // Send completion notification
                let complete_message = FaeMessage::CompleteSymbolIndex(filepath.to_string());
                if let Err(e) = controller
                    .send_message("completeSymbolIndex".to_string(), complete_message)
                    .await {
                    log::warn!("Failed to send completeSymbolIndex message: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Failed to extract symbols from {}: {}", filepath, e);
            }
        }

        // Remove from processing set when done
        {
            let mut files = processing_files.lock().unwrap();
            files.remove(filepath);
        }
    }

    /// Interrupt processing for a specific file
    async fn interrupt_file_processing(&mut self, filepath: &str) {
        log::debug!("Interrupting processing for file: {}", filepath);
        
        // Remove from processing set to signal interruption
        {
            let mut processing_files = self.processing_files.lock().unwrap();
            processing_files.remove(filepath);
        }

        // Note: We don't abort the actual tasks here because they check the processing_files
        // set regularly and will self-terminate when they find they're no longer marked as processing
    }

    /// Handle file deletion by clearing its symbols
    async fn handle_file_delete(
        &mut self,
        filepath: &str,
        controller: &ActorController<FaeMessage>,
    ) {
        log::info!("Handling file deletion: {}", filepath);

        // Interrupt any ongoing processing for this file
        let is_processing = {
            let processing_files = self.processing_files.lock().unwrap();
            processing_files.contains(filepath)
        };

        if is_processing {
            log::info!("Interrupting processing for deleted file: {}", filepath);
            self.interrupt_file_processing(filepath).await;
        }

        // Clear symbols for the deleted file
        let _ = controller
            .send_message(
                "clearSymbolIndex".to_string(),
                FaeMessage::ClearSymbolIndex(filepath.to_string()),
            )
            .await;
        
        // Send completion notification for deletion
        let complete_message = FaeMessage::CompleteSymbolIndex(filepath.to_string());
        let _ = controller
            .send_message("completeSymbolIndex".to_string(), complete_message)
            .await;
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
                log::info!("Starting symbol generation");
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
            _ => {
                log::debug!("Unknown message method: {}", message.method);
            }
        }
    }
}

/// Symbol index generation actor that extracts and broadcasts symbols
pub type SymbolIndexActor = Actor<FaeMessage, SymbolIndexHandler>;

impl SymbolIndexActor {
    /// Create a new SymbolIndexActor for symbol generation
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
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
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
        let mut complete_count = 0;

        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                match msg.method.as_str() {
                    "clearSymbolIndex" => clear_count += 1,
                    "pushSymbolIndex" => push_count += 1,
                    "completeSymbolIndex" => complete_count += 1,
                    _ => {}
                }
            } else {
                break;
            }
        }

        println!(
            "Initialization results: {} clear, {} push, {} complete messages",
            clear_count, push_count, complete_count
        );

        // Should have processed some Rust files in src/
        assert!(clear_count > 0, "Should have cleared some file indices");
        assert!(push_count > 0, "Should have pushed some symbols");
        assert!(complete_count > 0, "Should have completed indexing for some files");

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
        let mut received_complete = false;

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
                    "completeSymbolIndex" => {
                        if let FaeMessage::CompleteSymbolIndex(filepath) = msg.payload {
                            if filepath.contains("types.rs") {
                                received_complete = true;
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
        assert!(
            received_complete,
            "Should have received completeSymbolIndex for types.rs"
        );

        // Clean up
        actor.shutdown();
    }
}
