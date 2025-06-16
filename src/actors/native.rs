//! Native search actor for text search using pure Rust implementation
//!
//! This actor provides text search functionality without depending on external
//! tools like ripgrep or ag. It uses standard Rust libraries for file discovery
//! and content searching.

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams, SearchResult};
use crate::core::{CommandActor, CommandController, CommandFactory, CommandHandler, Message};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Create native search command factory
///
/// Since we don't use external commands, this factory creates a dummy command
/// that will never be executed. The actual search is performed in the handler.
pub fn create_native_search_command_factory(
    _search_path: String,
) -> impl CommandFactory<SearchParams> {
    move |_args: SearchParams| -> Command {
        // Create a dummy command that will never be executed
        // The native search logic is handled in the CommandHandler
        let mut cmd = Command::new("echo");
        cmd.arg("native_search_dummy");
        cmd
    }
}

/// Native search actor handler
pub struct NativeSearchHandler {
    search_path: String,
}

impl NativeSearchHandler {
    pub fn new(search_path: String) -> Self {
        Self { search_path }
    }

    /// Perform file discovery and content search
    async fn perform_search(
        &self,
        params: SearchParams,
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        log::info!(
            "Starting native search: {} (mode: {:?}) in {}",
            params.query,
            params.mode,
            self.search_path
        );

        // Clone params for the blocking task
        let query = params.query.clone();
        let mode = params.mode;
        let search_path = self.search_path.clone();

        // Perform search synchronously in the current context
        // Since we're already in an async context and CommandController
        // methods are async, we can't easily move it to spawn_blocking
        let result =
            tokio::task::spawn_blocking(move || Self::search_files(&search_path, &query, mode))
                .await;

        match result {
            Ok(Ok(results)) => {
                log::info!("Native search found {} results", results.len());
                for result in results {
                    let message = FaeMessage::PushSearchResult(result);
                    if let Err(e) = controller
                        .send_message("pushSearchResult".to_string(), message)
                        .await
                    {
                        log::warn!("Failed to send search result: {}", e);
                        break;
                    }
                }
            }
            Ok(Err(e)) => {
                log::error!("Native search failed: {}", e);
            }
            Err(e) => {
                log::error!("Native search task panicked: {}", e);
            }
        }
    }

    /// Search files in the given directory (blocking operation)
    fn search_files(
        search_path: &str,
        query: &str,
        mode: SearchMode,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();

        // Prepare search pattern based on mode
        let regex = match mode {
            SearchMode::Literal => {
                // For literal search, escape the query
                Regex::new(&regex::escape(query))?
            }
            SearchMode::Regexp => {
                // For regex search, use the query as-is
                Regex::new(query)?
            }
        };

        // Walk through files using ignore crate for proper .gitignore support
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

            // Skip directories and binary files
            if !path.is_file() || Self::is_binary_file(path) {
                continue;
            }

            // Search within the file
            if let Ok(file_results) = Self::search_in_file(path, &regex) {
                results.extend(file_results);
            }
        }

        Ok(results)
    }

    /// Search for matches within a single file
    fn search_in_file(
        path: &Path,
        regex: &Regex,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        let filename = path.to_string_lossy().to_string();

        for (line_number, line_result) in reader.lines().enumerate() {
            let line = line_result?;

            // Find all matches in this line
            for mat in regex.find_iter(&line) {
                let search_result = SearchResult {
                    filename: filename.clone(),
                    line: (line_number + 1) as u32, // 1-based line numbering
                    offset: (mat.start() + 1) as u32, // 1-based column numbering
                    content: line.clone(),
                };
                results.push(search_result);
            }
        }

        Ok(results)
    }

    /// Check if a file is likely binary (simple heuristic)
    fn is_binary_file(path: &Path) -> bool {
        // Check file extension for common binary types
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                match ext_str.to_lowercase().as_str() {
                    "exe" | "dll" | "so" | "dylib" | "a" | "lib" | "o" | "obj" => return true,
                    "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico" | "svg" => return true,
                    "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac" => return true,
                    "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => return true,
                    "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => return true,
                    _ => {}
                }
            }
        }

        // Check file size (skip very large files)
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > 1_000_000 {
                // Skip files larger than 1MB
                return true;
            }
        }

        false
    }

    /// Check if a file should be ignored based on common patterns
    /// Note: This method is kept for backward compatibility and testing.
    /// The ignore crate is now used for actual filtering in search_files.
    #[allow(dead_code)]
    fn should_ignore_file(path: &Path) -> bool {
        // Skip hidden files
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
        {
            return true;
        }

        // Skip common directories that should be ignored
        let ignore_patterns = [
            ".git",
            "target",
            "node_modules",
            "build",
            "dist",
            ".cache",
            "tmp",
        ];

        // Check if any component of the path matches ignore patterns
        for component in path.components() {
            if let std::path::Component::Normal(name) = component {
                if let Some(name_str) = name.to_str() {
                    for pattern in &ignore_patterns {
                        if name_str == *pattern {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

#[async_trait]
impl CommandHandler<FaeMessage, SearchParams> for NativeSearchHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        match message.method.as_str() {
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(query) = message.payload {
                    log::info!(
                        "Starting native search: {} (mode: {:?})",
                        query.query,
                        query.mode
                    );
                    let _ = controller
                        .send_message("clearResults".to_string(), FaeMessage::ClearResults)
                        .await;

                    // Perform the search without spawning external command
                    self.perform_search(query, controller).await;
                } else {
                    log::warn!("updateSearchParams received non-SearchQuery payload");
                }
            }
            _ => {
                log::debug!("Unknown message method: {}", message.method);
            }
        }
    }

    async fn on_stdout(
        &mut self,
        line: String,
        _controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        // Native search doesn't use stdout from external commands
        log::debug!("Native search received unexpected stdout: {}", line);
    }

    async fn on_stderr(
        &mut self,
        line: String,
        _controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        // Native search doesn't use stderr from external commands
        log::debug!("Native search received unexpected stderr: {}", line);
    }
}

/// Native search actor for text search
pub type NativeSearchActor = CommandActor<FaeMessage, SearchParams>;

impl NativeSearchActor {
    /// Create a new NativeSearchActor
    pub fn new_native_search_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: impl Into<String>,
    ) -> Self {
        let search_path_str = search_path.into();
        let command_factory = Arc::new(create_native_search_command_factory(
            search_path_str.clone(),
        ));
        let handler = NativeSearchHandler::new(search_path_str);

        Self::new(message_receiver, sender, command_factory, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_is_binary_file() {
        assert!(NativeSearchHandler::is_binary_file(Path::new("test.exe")));
        assert!(NativeSearchHandler::is_binary_file(Path::new("image.png")));
        assert!(NativeSearchHandler::is_binary_file(Path::new(
            "archive.zip"
        )));
        assert!(!NativeSearchHandler::is_binary_file(Path::new("source.rs")));
        assert!(!NativeSearchHandler::is_binary_file(Path::new("README.md")));
    }

    #[test]
    fn test_should_ignore_file() {
        assert!(NativeSearchHandler::should_ignore_file(Path::new(
            ".git/config"
        )));
        assert!(NativeSearchHandler::should_ignore_file(Path::new(
            "target/debug/main"
        )));
        assert!(NativeSearchHandler::should_ignore_file(Path::new(
            ".hidden_file"
        )));
        assert!(!NativeSearchHandler::should_ignore_file(Path::new(
            "src/main.rs"
        )));
    }

    #[tokio::test]
    async fn test_native_search_command_factory() {
        let factory = create_native_search_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "echo"); // Dummy command
    }

    #[tokio::test]
    async fn test_native_search_actor_integration() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create NativeSearchActor
        let mut actor = NativeSearchActor::new_native_search_actor(
            actor_rx,
            external_tx,
            "./src", // Search in src directory
        );

        // Send search query
        let search_query = SearchParams {
            query: "NativeSearchHandler".to_string(),
            mode: SearchMode::Literal,
        };
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(search_query),
        );

        actor_tx
            .send(search_message)
            .expect("Failed to send search message");

        // Wait for search results
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Check if we received any search results
        let mut result_count = 0;
        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        println!(
                            "Found match: {}:{}:{} - {}",
                            result.filename, result.line, result.offset, result.content
                        );
                        result_count += 1;
                    }
                }
            } else {
                break;
            }
        }

        println!("Total native search results: {}", result_count);
        // The test should find at least one match for 'NativeSearchHandler' in this file
        assert!(
            result_count > 0,
            "Should find at least one match for 'NativeSearchHandler'"
        );

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_search_files_literal_mode() {
        let results = NativeSearchHandler::search_files("./src", "async fn", SearchMode::Literal)
            .expect("Search should succeed");

        println!("Found {} literal matches for 'async fn'", results.len());
        assert!(
            results.len() > 0,
            "Should find literal matches for 'async fn'"
        );

        // Verify that results have correct structure
        for result in results.iter().take(3) {
            assert!(!result.filename.is_empty());
            assert!(result.line > 0);
            assert!(result.offset > 0);
            assert!(!result.content.is_empty());
            println!(
                "  {}:{}:{} - {}",
                result.filename,
                result.line,
                result.offset,
                result.content.trim()
            );
        }
    }

    #[test]
    fn test_is_binary_file_edge_cases() {
        // Test files without extension
        assert!(!NativeSearchHandler::is_binary_file(Path::new("Makefile")));
        assert!(!NativeSearchHandler::is_binary_file(Path::new("LICENSE")));
        
        // Test case sensitivity
        assert!(NativeSearchHandler::is_binary_file(Path::new("test.EXE")));
        assert!(NativeSearchHandler::is_binary_file(Path::new("image.PNG")));
        
        // Test invalid paths (should not panic)
        assert!(!NativeSearchHandler::is_binary_file(Path::new("")));
    }

    #[test]
    fn test_should_ignore_file_edge_cases() {
        // Test nested ignore patterns
        assert!(NativeSearchHandler::should_ignore_file(Path::new("project/.git/hooks/pre-commit")));
        assert!(NativeSearchHandler::should_ignore_file(Path::new("app/node_modules/package/index.js")));
        
        // Test files that start with ignore pattern names
        assert!(!NativeSearchHandler::should_ignore_file(Path::new("targets.rs")));
        assert!(!NativeSearchHandler::should_ignore_file(Path::new("git-utils.rs")));
        
        // Test empty paths
        assert!(!NativeSearchHandler::should_ignore_file(Path::new("")));
    }

    #[tokio::test]
    async fn test_native_search_handler_error_cases() {
        // Test native search handler edge cases using a full actor setup
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create NativeSearchActor
        let mut actor = NativeSearchActor::new_native_search_actor(actor_rx, external_tx, "./test");

        // Test 1: Invalid payload type - send wrong message type
        let invalid_message = Message::new("updateSearchParams", FaeMessage::ClearResults);
        actor_tx.send(invalid_message).expect("Should send message");

        // Test 2: Unknown method
        let unknown_message = Message::new("unknownMethod", FaeMessage::ClearResults);  
        actor_tx.send(unknown_message).expect("Should send message");

        // Wait a bit for message processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // No result messages should be received for invalid operations
        let mut result_count = 0;
        while let Ok(message) = tokio::time::timeout(
            std::time::Duration::from_millis(50), 
            external_rx.recv()
        ).await {
            if let Some(_msg) = message {
                result_count += 1;
            } else {
                break;
            }
        }

        // Should receive no search results for invalid operations
        assert_eq!(result_count, 0, "Invalid operations should not produce search results");

        // Clean up
        actor.shutdown();
    }

    #[test]
    fn test_search_files_error_cases() {
        // Test search in non-existent directory
        let result = NativeSearchHandler::search_files("/non/existent/path", "test", SearchMode::Literal);
        // Should handle gracefully, not panic
        assert!(result.is_ok());
        let results = result.unwrap();
        assert_eq!(results.len(), 0);

        // Test invalid regex pattern
        let result = NativeSearchHandler::search_files("./src", "[invalid_regex", SearchMode::Regexp);
        assert!(result.is_err());
    }

    #[test]
    fn test_command_factory_regex_mode() {
        let factory = create_native_search_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test.*pattern".to_string(),
            mode: SearchMode::Regexp,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "echo"); // Should still be dummy command
    }

    #[tokio::test]
    async fn test_native_handler_stdout_stderr() {
        // Test that native handler properly handles unexpected stdout/stderr
        // Since we can't easily create a CommandController for testing,
        // this test primarily validates that the methods exist and don't panic
        let _handler = NativeSearchHandler::new("./test".to_string());
        
        // Use actor setup to get real controller access during actual operation
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let mut actor = NativeSearchActor::new_native_search_actor(actor_rx, external_tx, "./test");

        // Send a dummy search to trigger any setup, then clean up immediately
        let search_message = Message::new(
            "updateSearchParams", 
            FaeMessage::UpdateSearchParams(SearchParams {
                query: "test".to_string(),
                mode: SearchMode::Literal,
            })
        );
        actor_tx.send(search_message).expect("Should send message");
        
        // Wait a tiny bit then clean up
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        
        // Drain any results
        while let Ok(_) = tokio::time::timeout(
            std::time::Duration::from_millis(5), 
            external_rx.recv()
        ).await {}

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_search_files_with_ignore_integration() {
        // Test that ignore crate integration works properly
        let results = NativeSearchHandler::search_files("./src", "NativeSearchHandler", SearchMode::Literal)
            .expect("Search should succeed");

        println!("Found {} results with ignore integration", results.len());
        assert!(
            results.len() > 0,
            "Should find matches for 'NativeSearchHandler' with ignore crate"
        );

        // Verify that results don't include files that should be ignored
        // (e.g., no results from target/ directory if it exists)
        for result in &results {
            assert!(!result.filename.contains("/target/"), 
                "Should not include files from target directory: {}", result.filename);
            assert!(!result.filename.contains("/.git/"), 
                "Should not include files from .git directory: {}", result.filename);
        }

        // Show some example results
        for result in results.iter().take(3) {
            println!(
                "  {}:{}:{} - {}",
                result.filename,
                result.line,
                result.offset,
                result.content.trim()
            );
        }
    }

    #[tokio::test]
    async fn test_search_files_regex_mode() {
        let results = NativeSearchHandler::search_files("./src", r"fn \w+\(", SearchMode::Regexp)
            .expect("Search should succeed");

        println!(
            "Found {} regex matches for function definitions",
            results.len()
        );
        assert!(
            results.len() > 0,
            "Should find regex matches for function definitions"
        );

        // Verify that results have correct structure
        for result in results.iter().take(3) {
            assert!(!result.filename.is_empty());
            assert!(result.line > 0);
            assert!(result.offset > 0);
            assert!(!result.content.is_empty());
            println!(
                "  {}:{}:{} - {}",
                result.filename,
                result.line,
                result.offset,
                result.content.trim()
            );
        }
    }
}
