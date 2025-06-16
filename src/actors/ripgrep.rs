//! Ripgrep actor for fast text search using CommandActor

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams, SearchResult};
use crate::core::{CommandActor, CommandController, CommandFactory, CommandHandler, Message};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Create ripgrep command factory
pub fn create_ripgrep_command_factory(search_path: String) -> impl CommandFactory<SearchParams> {
    move |args: SearchParams| -> Command {
        let mut cmd = Command::new("rg");

        // Add mode-specific flags
        match args.mode {
            SearchMode::Literal => {
                cmd.arg("--fixed-strings");
            }
            SearchMode::Regexp => {
                // Default behavior, no additional flags needed
            }
        }

        // Common flags for structured output
        cmd.arg("--vimgrep") // Show every match on its own line with line/column numbers
            .arg("--no-heading") // Don't group by file
            .arg("--color=never") // No color codes
            .arg(&args.query) // Search pattern
            .arg(&search_path); // Search path

        cmd
    }
}

/// Ripgrep actor handler
pub struct RipgrepHandler;

impl RipgrepHandler {
    pub fn new() -> Self {
        Self
    }

    /// Parse ripgrep output line into SearchResult
    fn parse_ripgrep_line(&self, line: &str) -> Option<SearchResult> {
        // Ripgrep output format with --vimgrep:
        // filename:line:column:content
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() >= 4 {
            let filename = parts[0].to_string();
            let line = parts[1].parse::<u32>().ok()?;
            let column = parts[2].parse::<u32>().ok()?;
            let content = parts[3].to_string();

            Some(SearchResult {
                filename,
                line,
                offset: column, // Store column position in offset field for compatibility
                content,
            })
        } else {
            None
        }
    }
}

#[async_trait]
impl CommandHandler<FaeMessage, SearchParams> for RipgrepHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        match message.method.as_str() {
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(query) = message.payload {
                    log::info!(
                        "Starting ripgrep search: {} (mode: {:?})",
                        query.query,
                        query.mode
                    );
                    let _ = controller
                        .send_message("clearResults".to_string(), FaeMessage::ClearResults)
                        .await;
                    if let Err(e) = controller.spawn(query).await {
                        log::error!("Failed to spawn ripgrep command: {}", e);
                    }
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
        controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        if let Some(result) = self.parse_ripgrep_line(&line) {
            let message = FaeMessage::PushSearchResult(result);
            if let Err(e) = controller
                .send_message("pushSearchResult".to_string(), message)
                .await
            {
                log::error!("Failed to send search result: {}", e);
            }
        } else {
            log::debug!("Failed to parse ripgrep output: {}", line);
        }
    }

    async fn on_stderr(
        &mut self,
        line: String,
        _controller: &CommandController<FaeMessage, SearchParams>,
    ) {
        log::warn!("Ripgrep stderr: {}", line);
    }
}

/// Ripgrep actor for text search
pub type RipgrepActor = CommandActor<FaeMessage, SearchParams>;

impl RipgrepActor {
    /// Create a new RipgrepActor
    pub fn new_ripgrep_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: impl Into<String>,
    ) -> Self {
        let command_factory = Arc::new(create_ripgrep_command_factory(search_path.into()));
        let handler = RipgrepHandler::new();

        Self::new(message_receiver, sender, command_factory, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_parse_ripgrep_line() {
        let handler = RipgrepHandler::new();

        // Test valid ripgrep output with column position
        let line = "src/main.rs:42:15:fn main() {";
        let result = handler.parse_ripgrep_line(line).unwrap();

        assert_eq!(result.filename, "src/main.rs");
        assert_eq!(result.line, 42);
        assert_eq!(result.offset, 15); // Now represents column position
        assert_eq!(result.content, "fn main() {");
    }

    #[test]
    fn test_parse_ripgrep_line_with_colons_in_content() {
        let handler = RipgrepHandler::new();

        // Test ripgrep output with colons in the content
        let line = "config.toml:10:5:server = \"localhost:8080\"";
        let result = handler.parse_ripgrep_line(line).unwrap();

        assert_eq!(result.filename, "config.toml");
        assert_eq!(result.line, 10);
        assert_eq!(result.offset, 5); // Column position
        assert_eq!(result.content, "server = \"localhost:8080\"");
    }

    #[tokio::test]
    async fn test_ripgrep_command_factory() {
        let factory = create_ripgrep_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "rg");
    }

    #[test]
    fn test_parse_ripgrep_line_invalid_format() {
        let handler = RipgrepHandler::new();

        // Test invalid format - too few colons
        let line = "invalidformat";
        let result = handler.parse_ripgrep_line(line);
        assert!(result.is_none());

        // Test invalid format - missing parts
        let line = "file.rs:42";
        let result = handler.parse_ripgrep_line(line);
        assert!(result.is_none());

        // Test invalid line number
        let line = "file.rs:not_a_number:15:content";
        let result = handler.parse_ripgrep_line(line);
        assert!(result.is_none());

        // Test invalid column number
        let line = "file.rs:42:not_a_number:content";
        let result = handler.parse_ripgrep_line(line);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ripgrep_command_factory_regex_mode() {
        let factory = create_ripgrep_command_factory("./src".to_string());

        let query = SearchParams {
            query: "test.*pattern".to_string(),
            mode: SearchMode::Regexp,
        };

        let cmd = factory(query);
        let program = cmd.as_std().get_program();
        assert_eq!(program, "rg");
    }

    #[tokio::test]
    async fn test_ripgrep_handler_error_cases() {
        // Test ripgrep handler edge cases using a full actor setup
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create RipgrepActor  
        let mut actor = RipgrepActor::new_ripgrep_actor(actor_rx, external_tx, "./test");

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

    #[tokio::test]
    async fn test_ripgrep_actor_integration() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create RipgrepActor
        let mut actor = RipgrepActor::new_ripgrep_actor(
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
                            result.filename, result.line, result.offset, result.content
                        );
                        result_count += 1;
                    }
                }
            } else {
                break;
            }
        }

        println!("Total search results: {}", result_count);
        assert!(
            result_count > 0,
            "Should find at least one match for 'CommandActor'"
        );

        // Clean up
        actor.shutdown();
    }
}
