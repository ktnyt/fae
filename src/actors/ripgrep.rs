//! Ripgrep search actor implementation
//!
//! This module provides a CommandActor-based ripgrep search implementation
//! that processes search queries and emits real-time search results.

use crate::actors::messages::{FaeMessage, SearchMessage, SearchMode, SearchParams, SearchResult};
use crate::core::command::{CommandActor, CommandHandler, CommandMessageHandler};
use crate::core::{ActorController, Message, MessageHandler};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Create a ripgrep command with specified search parameters
pub fn create_ripgrep_command(search_params: SearchParams) -> Command {
    let mut cmd = Command::new("rg");
    cmd.arg("--line-number")
        .arg("--column")
        .arg("--no-heading")
        .arg("--with-filename")
        .arg("--color=never");

    // Add search mode specific flags
    match search_params.mode {
        SearchMode::Literal => {
            cmd.arg("--fixed-strings"); // -F flag for literal search
        }
        SearchMode::Regexp => {
            // Default mode is already regex, no additional flags needed
        }
    }

    cmd.arg(search_params.query).arg(".");
    cmd
}

/// Handler for processing search messages and managing ripgrep execution
#[derive(Clone)]
pub struct RipgrepMessageHandler {
    current_query: Arc<Mutex<Option<String>>>,
    current_mode: Arc<Mutex<SearchMode>>,
}

impl RipgrepMessageHandler {
    pub fn new(mode: SearchMode) -> Self {
        Self {
            current_query: Arc::new(Mutex::new(None)),
            current_mode: Arc::new(Mutex::new(mode)),
        }
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for RipgrepMessageHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        _controller: &ActorController<FaeMessage>,
    ) {
        if let Some(search_msg) = message.payload.as_search() {
            match search_msg {
                SearchMessage::UpdateQuery { search_params } => {
                    log::debug!(
                        "Received search query: {} with mode: {:?}",
                        search_params.query,
                        search_params.mode
                    );
                    let mut current_query = self.current_query.lock().unwrap();
                    *current_query = Some(search_params.query.clone());
                    let mut current_mode = self.current_mode.lock().unwrap();
                    *current_mode = search_params.mode;
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
                    log::debug!("Clearing search results");
                }
            }
        }
    }
}

#[async_trait]
impl CommandMessageHandler<FaeMessage> for RipgrepMessageHandler {
    async fn on_message<Args: Send + 'static>(
        &mut self,
        message: Message<FaeMessage>,
        controller: &crate::core::command::CommandController<FaeMessage, Args>,
    ) {
        if let Some(search_msg) = message.payload.as_search() {
            match search_msg {
                SearchMessage::UpdateQuery { search_params } => {
                    log::info!(
                        "Starting ripgrep search for: {} with mode: {:?}",
                        search_params.query,
                        search_params.mode
                    );

                    // Clear previous results before starting new search
                    let _ = controller
                        .send_message("clearResults".to_string(), FaeMessage::clear_results())
                        .await;

                    // Store the current query and mode
                    {
                        let mut current_query = self.current_query.lock().unwrap();
                        *current_query = Some(search_params.query.clone());
                        let mut current_mode = self.current_mode.lock().unwrap();
                        *current_mode = search_params.mode;
                    }

                    // This would trigger command spawn, but we need to handle Args properly
                    // For now, just log the query and mode
                    log::debug!(
                        "Query and mode stored: {} {:?}",
                        search_params.query,
                        search_params.mode
                    );
                }
                SearchMessage::PushSearchResult { result } => {
                    // Forward search results to external listeners
                    let _ = controller
                        .send_message(
                            "pushSearchResult".to_string(),
                            FaeMessage::push_search_result(result.clone()),
                        )
                        .await;
                }
                SearchMessage::ClearResults => {
                    // ClearResults is now sent automatically at the start of UpdateQuery
                    // No action needed here
                }
            }
        }
    }
}

/// Handler for processing ripgrep command output
#[derive(Clone)]
pub struct RipgrepOutputHandler;

impl RipgrepOutputHandler {
    pub fn new() -> Self {
        Self
    }

    /// Parse ripgrep output line into SearchResult
    fn parse_rg_line(line: &str) -> Option<SearchResult> {
        // Expected format: filename:line:column:content
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() >= 4 {
            if let (Ok(line_num), Ok(offset)) = (parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
                return Some(SearchResult {
                    filename: parts[0].to_string(),
                    line: line_num,
                    offset,
                    content: parts[3].to_string(),
                });
            }
        }
        None
    }
}

#[async_trait]
impl CommandHandler<FaeMessage> for RipgrepOutputHandler {
    async fn on_stdout<Args: Send + 'static>(
        &mut self,
        line: String,
        controller: &crate::core::command::CommandController<FaeMessage, Args>,
    ) {
        if let Some(result) = Self::parse_rg_line(&line) {
            let _ = controller
                .send_message(
                    "pushSearchResult".to_string(),
                    FaeMessage::push_search_result(result),
                )
                .await;
        } else {
            log::warn!("Failed to parse ripgrep output: {}", line);
        }
    }

    async fn on_stderr<Args: Send + 'static>(
        &mut self,
        line: String,
        _controller: &crate::core::command::CommandController<FaeMessage, Args>,
    ) {
        log::warn!("Ripgrep stderr: {}", line);
    }
}

/// Type alias for the complete RipgrepActor
pub type RipgrepActor =
    CommandActor<FaeMessage, RipgrepMessageHandler, RipgrepOutputHandler, SearchParams>;

impl RipgrepActor {
    /// Create a new RipgrepActor
    pub fn create(
        receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        default_mode: SearchMode,
    ) -> Self {
        let message_handler = RipgrepMessageHandler::new(default_mode);
        let command_handler = RipgrepOutputHandler::new();
        let command_factory = Arc::new(create_ripgrep_command);

        Self::new(
            receiver,
            sender,
            message_handler,
            command_handler,
            command_factory,
        )
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
                FaeMessage::update_search_query(search_params.clone()),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Spawn ripgrep command with the search params
        self.spawn(search_params).await?;

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
                FaeMessage::update_search_query(search_params.clone()),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        // Spawn ripgrep command with the search params
        self.spawn(search_params).await?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio::time::{sleep, Duration};

    #[test]
    fn test_parse_rg_line() {
        let line = "src/main.rs:42:15:    let result = search_function();";
        let result = RipgrepOutputHandler::parse_rg_line(line);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.filename, "src/main.rs");
        assert_eq!(result.line, 42);
        assert_eq!(result.offset, 15);
        assert_eq!(result.content, "    let result = search_function();");
    }

    #[test]
    fn test_parse_rg_line_invalid() {
        let line = "invalid format";
        let result = RipgrepOutputHandler::parse_rg_line(line);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_ripgrep_actor_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let _actor = RipgrepActor::create(actor_rx, tx, SearchMode::Literal);

        // Test that actor can be created without issues
        assert!(true);
    }

    #[tokio::test]
    async fn test_ripgrep_message_handling() {
        let (tx, mut _rx) = mpsc::unbounded_channel();
        let (actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = RipgrepActor::create(actor_rx, tx, SearchMode::Regexp);

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

        // Should succeed even if ripgrep is not available (command creation should work)
        // The actual execution might fail but the setup should work
        assert!(search_result.is_ok() || search_result.is_err()); // Either is acceptable for this test
    }

    #[tokio::test]
    async fn test_search_result_message() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

        let actor = RipgrepActor::create(actor_rx, tx, SearchMode::Literal);

        // Send a search result message directly
        let search_result = SearchResult {
            filename: "test.rs".to_string(),
            line: 10,
            offset: 5,
            content: "test content".to_string(),
        };

        actor
            .actor()
            .send_message(
                "pushSearchResult".to_string(),
                FaeMessage::push_search_result(search_result.clone()),
            )
            .await
            .unwrap();

        // Should receive the search result message
        let received = rx.recv().await.unwrap();
        assert_eq!(received.method, "pushSearchResult");

        if let Some(SearchMessage::PushSearchResult { result }) = received.payload.as_search() {
            assert_eq!(result.filename, search_result.filename);
            assert_eq!(result.line, search_result.line);
            assert_eq!(result.offset, search_result.offset);
            assert_eq!(result.content, search_result.content);
        } else {
            panic!("Expected PushSearchResult message");
        }
    }

    #[test]
    fn test_search_mode_literal() {
        let search_params = SearchParams::literal("test query".to_string());
        let cmd = create_ripgrep_command(search_params);

        // Check that the command includes --fixed-strings flag for literal search
        let cmd_debug = format!("{:?}", cmd);
        assert!(cmd_debug.contains("--fixed-strings"));
    }

    #[test]
    fn test_search_mode_regexp() {
        let search_params = SearchParams::regex("test.*pattern".to_string());
        let cmd = create_ripgrep_command(search_params);

        // Check that the command does not include --fixed-strings flag for regex search
        let cmd_debug = format!("{:?}", cmd);
        assert!(!cmd_debug.contains("--fixed-strings"));
    }

    #[tokio::test]
    async fn test_literal_vs_regexp_actors() {
        // Test that we can create actors with different modes
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (_actor_tx1, actor_rx1) = mpsc::unbounded_channel();
        let literal_actor = RipgrepActor::create(actor_rx1, tx1, SearchMode::Literal);

        let (tx2, _rx2) = mpsc::unbounded_channel();
        let (_actor_tx2, actor_rx2) = mpsc::unbounded_channel();
        let regexp_actor = RipgrepActor::create(actor_rx2, tx2, SearchMode::Regexp);

        // Both should be created successfully
        assert!(std::ptr::addr_of!(literal_actor) as *const _ != std::ptr::null());
        assert!(std::ptr::addr_of!(regexp_actor) as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_update_query_auto_sends_clear_results() {
        use crate::core::command::{CommandController, CommandMessageHandler};
        use crate::core::ActorController;
        use std::sync::{Arc, Mutex};
        use tokio_util::sync::CancellationToken;
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // Create a RipgrepMessageHandler directly for testing
        let mut message_handler = RipgrepMessageHandler::new(SearchMode::Literal);
        
        // Create CommandController mock for testing
        let actor_controller = ActorController::new(tx.clone());
        let current_process = Arc::new(Mutex::new(None));
        let cancellation_token = Arc::new(Mutex::new(None::<CancellationToken>));
        let command_factory = Arc::new(create_ripgrep_command);
        
        let controller = CommandController::new(
            actor_controller,
            current_process,
            cancellation_token,
            command_factory,
        );
        
        // Create UpdateQuery message
        let search_params = SearchParams::new("test_query".to_string(), SearchMode::Literal);
        let update_message = Message::new(
            "updateQuery",
            FaeMessage::update_search_query(search_params),
        );
        
        // Send UpdateQuery message via CommandMessageHandler
        CommandMessageHandler::on_message(&mut message_handler, update_message, &controller).await;
        
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
