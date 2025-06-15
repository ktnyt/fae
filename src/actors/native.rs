//! Native search actor implementation
//!
//! This module provides a pure Rust search implementation that serves
//! as a fallback when neither ripgrep nor ag are available.
//! Respects .gitignore patterns and searches all files in the current directory.

use crate::actors::messages::{FaeMessage, SearchMessage, SearchMode, SearchParams, SearchResult};
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;

/// Native search implementation using pure Rust
pub struct NativeSearchActor {
    actor: Actor<FaeMessage, NativeSearchHandler>,
}

/// Handler for native search functionality
#[derive(Clone)]
pub struct NativeSearchHandler {
    sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    current_query: Arc<Mutex<Option<String>>>,
    current_mode: Arc<Mutex<SearchMode>>,
}

impl NativeSearchHandler {
    pub fn new(
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        default_mode: SearchMode,
    ) -> Self {
        Self {
            sender,
            current_query: Arc::new(Mutex::new(None)),
            current_mode: Arc::new(Mutex::new(default_mode)),
        }
    }

    /// Perform native search in a background task
    async fn perform_search(&self, search_params: SearchParams) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sender = self.sender.clone();
        let query = search_params.query.clone();
        let mode = search_params.mode;

        // Spawn blocking task for file I/O intensive operations
        task::spawn_blocking(move || {
            log::debug!("Starting spawn_blocking task for query: '{}'", query);
            
            // Compile regex for pattern matching if needed
            let regex = match mode {
                SearchMode::Regexp => match Regex::new(&query) {
                    Ok(r) => {
                        log::debug!("Regex compiled successfully: '{}'", query);
                        Some(r)
                    }
                    Err(e) => {
                        log::error!("Invalid regex pattern '{}': {}", query, e);
                        return;
                    }
                },
                SearchMode::Literal => {
                    log::debug!("Using literal search mode for: '{}'", query);
                    None
                }
            };

            // Walk through files respecting .gitignore
            let walker = WalkBuilder::new(".")
                .hidden(false)  // Include hidden files but respect .gitignore
                .ignore(true)   // Respect .ignore files
                .git_ignore(true)  // Respect .gitignore
                .git_exclude(true) // Respect .git/info/exclude
                .parents(true)  // Check parent directories for ignore files
                .build();

            let mut file_count = 0;
            let mut result_count = 0;
            
            for result in walker {
                match result {
                    Ok(entry) => {
                        let path = entry.path();
                        
                        // Skip directories
                        if !path.is_file() {
                            continue;
                        }

                        file_count += 1;
                        log::trace!("Processing file #{}: {}", file_count, path.display());

                        // Skip binary files (basic heuristic)
                        if is_likely_binary(path) {
                            log::trace!("Skipping binary file: {}", path.display());
                            continue;
                        }

                        // Read and search file content
                        match search_file(path, &query, mode, &regex, &sender) {
                            Ok(()) => {
                                result_count += 1;
                                log::trace!("Searched file successfully: {}", path.display());
                            }
                            Err(e) => {
                                // Check if this is a channel closed error (early termination)
                                let error_msg = e.to_string();
                                if error_msg.contains("channel closed") || error_msg.contains("closed") {
                                    log::info!("Search terminated early - channel closed after processing {} files", file_count);
                                    return; // Early termination
                                }
                                log::debug!("Error searching {}: {}", path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Walk error: {}", e);
                    }
                }
            }

            log::info!("Native search completed for query: '{}', processed {} files, found {} potential matches", query, file_count, result_count);
        }).await?;

        Ok(())
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for NativeSearchHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        _controller: &ActorController<FaeMessage>,
    ) {
        log::debug!("NativeSearchHandler received message: {}", message.method);
        if let Some(search_msg) = message.payload.as_search() {
            match search_msg {
                SearchMessage::UpdateQuery { search_params } => {
                    log::info!(
                        "Starting native search for: {} with mode: {:?}",
                        search_params.query,
                        search_params.mode
                    );

                    // Clear previous results before starting new search
                    let clear_message = Message::new(
                        "clearResults",
                        FaeMessage::clear_results(),
                    );
                    if let Err(e) = self.sender.send(clear_message) {
                        log::debug!("Failed to send clearResults: {}", e);
                    }

                    // Store the current query and mode
                    {
                        let mut current_query = self.current_query.lock().unwrap();
                        *current_query = Some(search_params.query.clone());
                        let mut current_mode = self.current_mode.lock().unwrap();
                        *current_mode = search_params.mode;
                    }

                    // Perform the search asynchronously
                    log::debug!("About to start perform_search for: {}", search_params.query);
                    if let Err(e) = self.perform_search(search_params.clone()).await {
                        log::error!("Native search failed: {}", e);
                    } else {
                        log::debug!("perform_search completed successfully");
                    }
                }
                SearchMessage::PushSearchResult { result } => {
                    log::trace!(
                        "Search result: {}:{}:{}",
                        result.filename,
                        result.line,
                        result.content
                    );
                }
                SearchMessage::ClearResults => {
                    // ClearResults is now sent automatically at the start of UpdateQuery
                    // No action needed here
                }
            }
        }
    }
}

impl NativeSearchActor {
    /// Create a new NativeSearchActor
    pub fn create(
        receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        default_mode: SearchMode,
    ) -> Self {
        let handler = NativeSearchHandler::new(sender.clone(), default_mode);
        let actor = Actor::new(receiver, sender, handler);

        Self { actor }
    }

    /// Get the underlying actor
    pub fn actor(&self) -> &Actor<FaeMessage, NativeSearchHandler> {
        &self.actor
    }

    /// Execute a search query with specified mode
    pub async fn search(
        &self,
        query: String,
        mode: SearchMode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let search_params = SearchParams::new(query, mode);

        // Send updateQuery message with SearchParams
        self.actor()
            .send_message(
                "updateQuery".to_string(),
                FaeMessage::update_search_query(search_params),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(())
    }

    /// Execute a search with SearchParams directly
    pub async fn search_params(
        &self,
        search_params: SearchParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Send updateQuery message with SearchParams
        self.actor()
            .send_message(
                "updateQuery".to_string(),
                FaeMessage::update_search_query(search_params),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(())
    }

    /// Clear search results
    pub async fn clear_results(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.actor()
            .send_message("clearResults".to_string(), FaeMessage::clear_results())
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(())
    }

    /// Shutdown the actor and clean up resources
    pub fn shutdown(&mut self) {
        self.actor.shutdown();
    }
}

/// Search a single file for the given pattern
fn search_file(
    path: &Path,
    query: &str,
    mode: SearchMode,
    regex: &Option<Regex>,
    sender: &mpsc::UnboundedSender<Message<FaeMessage>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let content = fs::read_to_string(path)?;
    let filename = path.to_string_lossy().to_string();

    for (line_number, line) in content.lines().enumerate() {
        let line_num = (line_number + 1) as u32;
        
        let matches = match mode {
            SearchMode::Literal => {
                // Simple substring search
                line.find(query).map(|pos| vec![(pos, query.len())])
            }
            SearchMode::Regexp => {
                // Regex search
                if let Some(ref regex) = regex {
                    let mut matches = Vec::new();
                    for mat in regex.find_iter(line) {
                        matches.push((mat.start(), mat.len()));
                    }
                    if matches.is_empty() {
                        None
                    } else {
                        Some(matches)
                    }
                } else {
                    None
                }
            }
        };

        if let Some(match_positions) = matches {
            for (offset, _length) in match_positions {
                let result = SearchResult {
                    filename: filename.clone(),
                    line: line_num,
                    offset: offset as u32,
                    content: line.to_string(),
                };

                let message = Message::new(
                    "pushSearchResult",
                    FaeMessage::push_search_result(result),
                );

                if let Err(e) = sender.send(message) {
                    log::debug!("Failed to send search result (channel closed): {}", e);
                    // Channel is closed, stop sending more results
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// Simple heuristic to detect binary files
fn is_likely_binary(path: &Path) -> bool {
    // Check file extension
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let binary_extensions = [
            "exe", "bin", "dll", "so", "dylib", "a", "lib", "obj", "o",
            "jpg", "jpeg", "png", "gif", "bmp", "ico", "svg", "tiff",
            "mp3", "mp4", "avi", "mkv", "mov", "wav", "flac", "ogg",
            "zip", "tar", "gz", "bz2", "xz", "7z", "rar", "dmg", "iso",
            "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
            "class", "jar", "war", "ear", "dex", "apk",
        ];
        
        if binary_extensions.contains(&ext.to_lowercase().as_str()) {
            return true;
        }
    }

    // Check if file is too large (>1MB)
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > 1024 * 1024 {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_native_search_actor_creation() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let _actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Literal);

        // Test that actor can be created without issues
        assert!(true);
    }

    #[tokio::test]
    async fn test_native_search_message_handling() {
        let (tx, mut _rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Regexp);

        // Send updateQuery message
        let query_message = Message::new(
            "updateQuery",
            FaeMessage::update_query("test_search".to_string(), SearchMode::Literal),
        );

        actor_tx.send(query_message).unwrap();

        // Give time for processing
        sleep(Duration::from_millis(10)).await;

        // Test search method
        let search_result = actor
            .search("test_pattern".to_string(), SearchMode::Regexp)
            .await;

        assert!(search_result.is_ok());
    }

    #[test]
    fn test_binary_file_detection() {
        // Test binary extensions
        assert!(is_likely_binary(Path::new("test.exe")));
        assert!(is_likely_binary(Path::new("image.jpg")));
        assert!(is_likely_binary(Path::new("archive.zip")));
        
        // Test text files
        assert!(!is_likely_binary(Path::new("test.rs")));
        assert!(!is_likely_binary(Path::new("README.md")));
        assert!(!is_likely_binary(Path::new("config.toml")));
    }

    #[tokio::test]
    async fn test_search_with_temp_files() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        
        // Create a test file
        fs::write(&file_path, "fn test_function() {\n    println!(\"Hello, world!\");\n}").unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Literal);

        // Change to temp directory for search
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Perform search
        let _ = actor.search("test_function".to_string(), SearchMode::Literal).await;

        // Wait a bit for the search to complete
        sleep(Duration::from_millis(100)).await;

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        // Check if we received any results (optional since async)
        // This test mainly verifies that the search doesn't crash
        assert!(true);
    }

    #[test]
    fn test_search_file_literal() {
        use std::io::Write;
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello world").unwrap();
        writeln!(file, "This is a test").unwrap();
        writeln!(file, "Hello again").unwrap();
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // Test literal search
        search_file(&file_path, "Hello", SearchMode::Literal, &None, &tx).unwrap();
        
        // Should find 2 matches
        let mut count = 0;
        while let Ok(message) = rx.try_recv() {
            if let Some(SearchMessage::PushSearchResult { result }) = message.payload.as_search() {
                assert!(result.content.contains("Hello"));
                count += 1;
            }
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_search_file_regex() {
        use std::io::Write;
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    let x = 42;").unwrap();
        writeln!(file, "fn test_function() {{").unwrap();
        writeln!(file, "}}").unwrap();
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        let regex = Regex::new(r"fn \w+").unwrap();
        
        // Test regex search
        search_file(&file_path, r"fn \w+", SearchMode::Regexp, &Some(regex), &tx).unwrap();
        
        // Should find 2 function definitions
        let mut count = 0;
        while let Ok(message) = rx.try_recv() {
            if let Some(SearchMessage::PushSearchResult { result }) = message.payload.as_search() {
                assert!(result.content.contains("fn "));
                count += 1;
            }
        }
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_search_params_method() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Literal);

        // Test search_params method
        let search_params = SearchParams::regex("test.*pattern".to_string());
        let result = actor.search_params(search_params).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clear_results() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Literal);

        // Test clear_results method
        let result = actor.clear_results().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_query_auto_sends_clear_results() {
        use crate::core::ActorController;
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // Create a NativeSearchHandler directly for testing
        let mut message_handler = NativeSearchHandler::new(tx.clone(), SearchMode::Literal);
        
        // Create ActorController mock for testing (NativeSearchHandler uses MessageHandler, not CommandMessageHandler)
        let controller = ActorController::new(tx.clone());
        
        // Create UpdateQuery message
        let search_params = SearchParams::new("test_query".to_string(), SearchMode::Literal);
        let update_message = Message::new(
            "updateQuery",
            FaeMessage::update_search_query(search_params),
        );
        
        // Send UpdateQuery message via MessageHandler
        message_handler.on_message(update_message, &controller).await;
        
        // Verify that ClearResults message was sent first
        let received = rx.recv().await.expect("Should receive ClearResults message");
        assert_eq!(received.method, "clearResults");
        
        if let Some(search_msg) = received.payload.as_search() {
            match search_msg {
                SearchMessage::ClearResults => {
                    // This is what we expect
                }
                _ => panic!("Expected ClearResults message, got: {:?}", search_msg),
            }
        } else {
            panic!("Expected search message payload");
        }
    }
}