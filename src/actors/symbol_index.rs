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
use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// File operation type for queue processing
#[derive(Debug, Clone)]
enum FileOperation {
    Create(String),
    Update(String),
    Delete(String),
}

/// Symbol index generation handler that extracts and broadcasts symbols
pub struct SymbolIndexHandler {
    search_path: String,
    /// Queue of file operations to process (create/update/delete)
    operation_queue: Arc<Mutex<VecDeque<FileOperation>>>,
    /// Track if we're currently processing a file to prevent concurrent operations
    is_processing: Arc<Mutex<bool>>,
}

impl SymbolIndexHandler {
    /// Create a new SymbolIndexHandler
    pub fn new(search_path: String) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            search_path,
            operation_queue: Arc::new(Mutex::new(VecDeque::new())),
            is_processing: Arc::new(Mutex::new(false)),
        })
    }

    /// Initialize symbol generation by populating queue with all supported files
    async fn initialize_index(&mut self, controller: &ActorController<FaeMessage>) {
        log::info!("Starting symbol generation for path: {}", self.search_path);

        // Populate queue with all supported files
        let search_path = self.search_path.clone();
        let operation_queue_clone = self.operation_queue.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::populate_initial_queue(&search_path, operation_queue_clone)
        })
        .await;

        match result {
            Ok(Ok(file_count)) => {
                log::info!(
                    "Initial queue populated with {} files",
                    file_count
                );
                // Start processing queue
                self.process_next_from_queue(controller).await;
            }
            Ok(Err(e)) => {
                log::error!("Queue population failed: {}", e);
            }
            Err(e) => {
                log::error!("Queue population task panicked: {}", e);
            }
        }
    }

    /// Populate initial queue with all supported files from directory scan
    fn populate_initial_queue(
        search_path: &str,
        operation_queue: Arc<Mutex<VecDeque<FileOperation>>>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut file_count = 0;

        // Walk through files using ignore crate
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Show hidden files by default
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global .gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .ignore(true) // Respect .ignore files
            .parents(true) // Check parent directories for ignore files
            .build();

        let mut queue = operation_queue.lock().unwrap();
        
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
            queue.push_back(FileOperation::Create(file_path_str));
            file_count += 1;
        }

        log::info!("Populated queue with {} file operations", file_count);
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

    /// Process next operation from queue
    async fn process_next_from_queue(&mut self, controller: &ActorController<FaeMessage>) {
        loop {
            // Try to get next operation from queue
            let operation = {
                let mut queue = self.operation_queue.lock().unwrap();
                queue.pop_front()
            };

            match operation {
                Some(op) => {
                    // Mark as processing
                    {
                        let mut processing = self.is_processing.lock().unwrap();
                        *processing = true;
                    }

                    // Process the operation
                    self.process_operation(op, controller).await;

                    // Mark as not processing
                    {
                        let mut processing = self.is_processing.lock().unwrap();
                        *processing = false;
                    }

                    // Continue to next operation
                    continue;
                }
                None => {
                    // Queue is empty - send CompleteSymbolIndex notification
                    log::info!("Operation queue is empty, sending CompleteSymbolIndex notification");
                    if let Err(e) = controller
                        .send_message(
                            "completeSymbolIndex".to_string(),
                            FaeMessage::CompleteSymbolIndex("all_files".to_string()),
                        )
                        .await
                    {
                        log::warn!("Failed to send CompleteSymbolIndex message: {}", e);
                    }
                    break;
                }
            }
        }
    }

    /// Add operation to queue if not already present
    fn add_operation_to_queue(&self, operation: FileOperation) {
        let mut queue = self.operation_queue.lock().unwrap();
        
        // Check if operation already exists for this file
        let filepath = match &operation {
            FileOperation::Create(path) | FileOperation::Update(path) | FileOperation::Delete(path) => path.clone(),
        };
        
        // Remove any existing operations for this file
        queue.retain(|op| {
            let existing_path = match op {
                FileOperation::Create(path) | FileOperation::Update(path) | FileOperation::Delete(path) => path,
            };
            existing_path != &filepath
        });
        
        // Add new operation
        queue.push_back(operation);
        log::debug!("Added operation to queue for file: {}", filepath);
    }

    /// Process a single file operation
    async fn process_operation(
        &mut self,
        operation: FileOperation,
        controller: &ActorController<FaeMessage>,
    ) {
        match operation {
            FileOperation::Create(filepath) | FileOperation::Update(filepath) => {
                self.handle_file_change(&filepath, controller).await;
            }
            FileOperation::Delete(filepath) => {
                self.handle_file_delete(&filepath, controller).await;
            }
        }
    }

    /// Handle file creation/update by re-indexing the file
    async fn handle_file_change(
        &mut self,
        filepath: &str,
        controller: &ActorController<FaeMessage>,
    ) {
        log::info!("Processing file change: {}", filepath);

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

        // Process file symbols
        Self::process_file_symbols_sync(filepath, path, controller.clone()).await;
    }

    /// Process file symbols synchronously (simplified version for queue processing)
    async fn process_file_symbols_sync(
        filepath: &str,
        path: &std::path::Path,
        controller: ActorController<FaeMessage>,
    ) {
        // Use a new extractor instance for this task
        let mut extractor = match SymbolExtractor::new() {
            Ok(extractor) => extractor,
            Err(e) => {
                log::error!("Failed to create symbol extractor: {}", e);
                return;
            }
        };

        // Extract symbols from the file
        match extractor.extract_symbols_from_file(path) {
            Ok(symbols) => {
                log::debug!("Extracted {} symbols from {}", symbols.len(), filepath);

                // Broadcast each symbol
                for symbol in symbols {
                    let push_message = FaeMessage::PushSymbolIndex {
                        filepath: symbol.filepath.clone(),
                        line: symbol.line,
                        column: symbol.column,
                        content: symbol.content.clone(),
                        symbol_type: symbol.symbol_type,
                    };

                    if let Err(e) = controller
                        .send_message("pushSymbolIndex".to_string(), push_message)
                        .await
                    {
                        log::warn!("Failed to send pushSymbolIndex message: {}", e);
                        break;
                    }
                }

                log::debug!("Successfully processed symbols for {}", filepath);

                // Send completion notification for this file
                let complete_message = FaeMessage::CompleteSymbolIndex(filepath.to_string());
                if let Err(e) = controller
                    .send_message("completeSymbolIndex".to_string(), complete_message)
                    .await
                {
                    log::warn!("Failed to send completeSymbolIndex message: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Failed to extract symbols from {}: {}", filepath, e);
            }
        }
    }

    /// Check if currently processing
    fn is_currently_processing(&self) -> bool {
        let processing = self.is_processing.lock().unwrap();
        *processing
    }

    /// Handle file deletion by clearing its symbols
    async fn handle_file_delete(
        &mut self,
        filepath: &str,
        controller: &ActorController<FaeMessage>,
    ) {
        log::info!("Processing file deletion: {}", filepath);

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
            "detectFileCreate" => {
                if let FaeMessage::DetectFileCreate(filepath) = message.payload {
                    // Add create operation to queue
                    self.add_operation_to_queue(FileOperation::Create(filepath));
                    // If not currently processing, start processing queue
                    if !self.is_currently_processing() {
                        self.process_next_from_queue(controller).await;
                    }
                } else {
                    log::warn!("detectFileCreate received non-filepath payload");
                }
            }
            "detectFileUpdate" => {
                if let FaeMessage::DetectFileUpdate(filepath) = message.payload {
                    // Add update operation to queue
                    self.add_operation_to_queue(FileOperation::Update(filepath));
                    // If not currently processing, start processing queue
                    if !self.is_currently_processing() {
                        self.process_next_from_queue(controller).await;
                    }
                } else {
                    log::warn!("detectFileUpdate received non-filepath payload");
                }
            }
            "detectFileDelete" => {
                if let FaeMessage::DetectFileDelete(filepath) = message.payload {
                    // Add delete operation to queue
                    self.add_operation_to_queue(FileOperation::Delete(filepath));
                    // If not currently processing, start processing queue
                    if !self.is_currently_processing() {
                        self.process_next_from_queue(controller).await;
                    }
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
        assert!(
            complete_count > 0,
            "Should have completed indexing for some files"
        );

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
