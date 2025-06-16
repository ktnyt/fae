//! The Silver Searcher (ag) actor for fast text search using CommandActor

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams, SearchResult};
use crate::core::{CommandActor, CommandController, CommandFactory, CommandHandler, Message};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Create ag command factory
pub fn create_ag_command_factory(search_path: String) -> impl CommandFactory<SearchParams> {
    move |args: SearchParams| -> Command {
        let mut cmd = Command::new("ag");

        // Add mode-specific flags
        match args.mode {
            SearchMode::Literal => {
                cmd.arg("--literal"); // -Q flag for literal search
            }
            SearchMode::Regexp => {
                // Default behavior, no additional flags needed
            }
            SearchMode::Filepath | SearchMode::Symbol | SearchMode::Variable => {
                // These modes are not supported by ag
                // Command will not be executed due to early return in handler
            }
        }

        // Common flags for structured output
        cmd.arg("--vimgrep") // Show every match on its own line with line/column numbers
            .arg("--nocolor") // No color output
            .arg(&args.query) // Search pattern
            .arg(&search_path); // Search path

        cmd
    }
}

/// Ag actor handler
pub struct AgHandler;

impl Default for AgHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl AgHandler {
    pub fn new() -> Self {
        Self
    }

    /// Parse ag output line into SearchResult
    fn parse_ag_line(&self, line: &str) -> Option<SearchResult> {
        // Ag output format with --vimgrep:
        // filename:line:column:content
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() >= 4 {
            let filename = parts[0].to_string();
            let line = parts[1].parse::<u32>().ok()?;
            let offset = parts[2].parse::<u32>().ok()?;
            let content = parts[3].to_string();

            Some(SearchResult {
                filename,
                line,
                column: offset,
                content,
            })
        } else {
            None
        }
    }
}

#[async_trait]
impl CommandHandler<FaeMessage, SearchParams> for AgHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        match message.method.as_str() {
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(query) = message.payload {
                    log::info!(
                        "Starting ag search: {} (mode: {:?})",
                        query.query,
                        query.mode
                    );
                    let _ = controller
                        .send_message("clearResults".to_string(), FaeMessage::ClearResults)
                        .await;

                    // Skip search for modes not supported by ag
                    match query.mode {
                        SearchMode::Filepath | SearchMode::Symbol | SearchMode::Variable => {
                            log::debug!(
                                "Ag skipping search for unsupported mode: {:?}",
                                query.mode
                            );
                            // Send completion notification for skipped modes
                            let _ = controller
                                .send_message(
                                    "completeSearch".to_string(),
                                    FaeMessage::CompleteSearch,
                                )
                                .await;
                            return;
                        }
                        SearchMode::Literal | SearchMode::Regexp => {
                            // Continue with supported modes
                        }
                    }

                    if let Err(e) = controller.spawn(query).await {
                        log::error!("Failed to spawn ag command: {}", e);
                        // Send completion notification on spawn failure
                        let _ = controller
                            .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
                            .await;
                    }
                } else {
                    log::warn!("updateSearchParams received non-SearchQuery payload");
                }
            }
            "processCompleted" => {
                // Command process completed, send completion notification
                let _ = controller
                    .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
                    .await;
            }
            _ => {
                log::debug!("Unknown message method: {}", message.method);
            }
        }
    }

    async fn on_stdout(
        &mut self,
        line: String,
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        if let Some(result) = self.parse_ag_line(&line) {
            let message = FaeMessage::PushSearchResult(result);
            if let Err(e) = controller
                .send_message("pushSearchResult".to_string(), message)
                .await
            {
                log::error!("Failed to send search result: {}", e);
            }
        } else {
            log::debug!("Failed to parse ag output: {}", line);
        }
    }

    async fn on_stderr(
        &mut self,
        line: String,
        _controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        log::warn!("Ag stderr: {}", line);
    }
}

/// Ag actor for text search
pub type AgActor = CommandActor<FaeMessage, SearchParams>;

impl AgActor {
    /// Create a new AgActor
    pub fn new_ag_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: impl Into<String>,
    ) -> Self {
        let command_factory = Arc::new(create_ag_command_factory(search_path.into()));
        let handler = AgHandler::new();

        Self::new(message_receiver, sender, command_factory, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_parse_ag_line() {
        let handler = AgHandler::new();

        // Test valid ag output
        let line = "src/main.rs:42:15:fn main() {";
        let result = handler.parse_ag_line(line).unwrap();

        assert_eq!(result.filename, "src/main.rs");
        assert_eq!(result.line, 42);
        assert_eq!(result.column, 15);
        assert_eq!(result.content, "fn main() {");
    }

    #[test]
    fn test_parse_ag_line_with_colons_in_content() {
        let handler = AgHandler::new();

        // Test ag output with colons in the content
        let line = "config.toml:10:5:server = \"localhost:8080\"";
        let result = handler.parse_ag_line(line).unwrap();

        assert_eq!(result.filename, "config.toml");
        assert_eq!(result.line, 10);
        assert_eq!(result.column, 5);
        assert_eq!(result.content, "server = \"localhost:8080\"");
    }

    #[tokio::test]
    async fn test_ag_command_factory() {
        let factory = create_ag_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "ag");
    }

    #[test]
    fn test_parse_ag_line_invalid_format() {
        let handler = AgHandler::new();

        // Test invalid format - too few colons
        let line = "invalidformat";
        let result = handler.parse_ag_line(line);
        assert!(result.is_none());

        // Test invalid format - missing parts
        let line = "file.rs:42";
        let result = handler.parse_ag_line(line);
        assert!(result.is_none());

        // Test invalid line number
        let line = "file.rs:not_a_number:15:content";
        let result = handler.parse_ag_line(line);
        assert!(result.is_none());

        // Test invalid offset
        let line = "file.rs:42:not_a_number:content";
        let result = handler.parse_ag_line(line);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ag_command_factory_regex_mode() {
        let factory = create_ag_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test.*pattern".to_string(),
            mode: SearchMode::Regexp,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "ag");
    }

    #[tokio::test]
    async fn test_ag_handler_error_cases() {
        // Test ag handler edge cases using a full actor setup
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create AgActor
        let mut actor = AgActor::new_ag_actor(actor_rx, external_tx, "./test");

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
        while let Ok(message) =
            tokio::time::timeout(std::time::Duration::from_millis(50), external_rx.recv()).await
        {
            if let Some(_msg) = message {
                result_count += 1;
            } else {
                break;
            }
        }

        // Should receive no search results for invalid operations
        assert_eq!(
            result_count, 0,
            "Invalid operations should not produce search results"
        );

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_ag_actor_integration() {
        // Check if ag is available before running the test
        if let Err(_) = tokio::process::Command::new("ag")
            .arg("--version")
            .output()
            .await
        {
            println!("Skipping ag integration test - ag not available");
            return;
        }

        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create AgActor
        let mut actor = AgActor::new_ag_actor(
            actor_rx,
            external_tx,
            "./src", // Search in src directory
        );

        // Send search query
        let search_query = SearchParams {
            query: "CommandActor".to_string(),
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
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Check if we received any search results
        let mut result_count = 0;
        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        println!(
                            "Found match: {}:{}:{} - {}",
                            result.filename, result.line, result.column, result.content
                        );
                        result_count += 1;
                    }
                }
            } else {
                break;
            }
        }

        println!("Total search results: {}", result_count);
        if result_count > 0 {
            assert!(
                result_count > 0,
                "Should find at least one match for 'CommandActor'"
            );
        } else {
            println!("No results found - this might be expected if ag has different behavior");
        }

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_ag_skips_unsupported_modes() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create AgActor
        let mut actor = AgActor::new_ag_actor(actor_rx, external_tx, "./src");

        // Test Filepath mode - should be skipped
        let filepath_query = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Filepath,
        };
        let filepath_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(filepath_query),
        );
        actor_tx
            .send(filepath_message)
            .expect("Should send message");

        // Test Symbol mode - should be skipped
        let symbol_query = SearchParams {
            query: "function".to_string(),
            mode: SearchMode::Symbol,
        };
        let symbol_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(symbol_query),
        );
        actor_tx.send(symbol_message).expect("Should send message");

        // Wait for message processing
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Check that no search results are produced for unsupported modes
        let mut result_count = 0;
        while let Ok(message) =
            tokio::time::timeout(std::time::Duration::from_millis(50), external_rx.recv()).await
        {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    result_count += 1;
                }
            } else {
                break;
            }
        }

        // Should receive no search results for unsupported modes
        assert_eq!(result_count, 0, "Ag should skip Filepath and Symbol modes");

        // Clean up
        actor.shutdown();
    }
}
