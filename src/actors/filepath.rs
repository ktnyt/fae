//! Filepath search actor for fuzzy file and directory path matching
//!
//! This actor provides fuzzy search functionality specifically for file and directory paths.
//! It discovers all files and directories that are not ignored, then performs fuzzy matching
//! against their paths using the skim matcher algorithm.

use crate::actors::messages::FaeMessage;
use crate::actors::types::{SearchMode, SearchParams, SearchResult};
use crate::core::{Actor, ActorController, Message, MessageHandler};
use async_trait::async_trait;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ignore::WalkBuilder;
use tokio::sync::mpsc;

/// Filepath search actor handler
pub struct FilepathSearchHandler {
    search_path: String,
}

impl FilepathSearchHandler {
    pub fn new(search_path: String) -> Self {
        Self { search_path }
    }

    /// Perform filepath discovery and fuzzy matching with streaming results
    async fn perform_search(&self, params: SearchParams, controller: &ActorController<FaeMessage>) {
        log::info!(
            "Starting filepath search: {} (mode: {:?}) in {}",
            params.query,
            params.mode,
            self.search_path
        );

        // Clone params and controller for the streaming task
        let query = params.query.clone();
        let mode = params.mode;
        let search_path = self.search_path.clone();
        let controller_clone = controller.clone();

        // Perform streaming search in a blocking task
        let result = tokio::task::spawn_blocking(move || {
            Self::search_filepaths(&search_path, &query, mode, controller_clone)
        })
        .await;

        match result {
            Ok(Ok(result_count)) => {
                log::info!("Filepath search completed with {} results", result_count);
                // Send completion notification
                let _ = controller
                    .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
                    .await;
            }
            Ok(Err(e)) => {
                log::error!("Filepath search failed: {}", e);
                // Send completion notification even on error
                let _ = controller
                    .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
                    .await;
            }
            Err(e) => {
                log::error!("Filepath search task panicked: {}", e);
                // Send completion notification even on panic
                let _ = controller
                    .send_message("completeSearch".to_string(), FaeMessage::CompleteSearch)
                    .await;
            }
        }
    }

    /// Search filepaths using fuzzy matching with streaming results (blocking operation)
    fn search_filepaths(
        search_path: &str,
        query: &str,
        mode: SearchMode,
        controller: ActorController<FaeMessage>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut result_count = 0;

        // Only handle Filepath mode
        if mode != SearchMode::Filepath {
            return Ok(result_count);
        }

        let matcher = SkimMatcherV2::default();
        let mut scored_results = Vec::new();

        // Walk through files and directories using ignore crate
        let walker = WalkBuilder::new(search_path)
            .hidden(false) // Show hidden files by default
            .git_ignore(true) // Respect .gitignore
            .git_global(true) // Respect global .gitignore
            .git_exclude(true) // Respect .git/info/exclude
            .ignore(true) // Respect .ignore files
            .parents(true) // Check parent directories for ignore files
            .build();

        // First pass: collect and score all matches
        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            let path_str = path.to_string_lossy();

            // Skip the search root itself
            if path_str == search_path {
                continue;
            }

            // Perform fuzzy matching against the relative path
            let relative_path = if let Ok(rel_path) = path.strip_prefix(search_path) {
                rel_path.to_string_lossy().to_string()
            } else {
                path_str.to_string()
            };

            if let Some((score, indices)) = matcher.fuzzy_indices(&relative_path, query) {
                // Create a search result for the matched filepath
                let search_result = SearchResult {
                    filename: path_str.to_string(),
                    line: 1,              // Line 1 for filepaths
                    column: score as u32, // Use score as offset for sorting
                    content: Self::format_match_content(&relative_path, &indices, path.is_dir()),
                };
                scored_results.push(search_result);
            }
        }

        // Sort by fuzzy matching score (higher is better)
        scored_results.sort_by(|a, b| b.column.cmp(&a.column));

        // Stream results in order of relevance
        for result in scored_results {
            let message = FaeMessage::PushSearchResult(result);

            // Use blocking send since we're in a blocking context
            let send_result = tokio::runtime::Handle::current().block_on(async {
                controller
                    .send_message("pushSearchResult".to_string(), message)
                    .await
            });

            if let Err(e) = send_result {
                log::warn!("Failed to send search result: {}", e);
                break;
            }

            result_count += 1;
        }

        Ok(result_count)
    }

    /// Format the match content with highlighted characters
    fn format_match_content(path: &str, indices: &[usize], is_dir: bool) -> String {
        let mut content = String::new();
        let chars: Vec<char> = path.chars().collect();
        let mut last_idx = 0;

        // Add type indicator
        let type_indicator = if is_dir { "[DIR] " } else { "[FILE] " };
        content.push_str(type_indicator);

        // Highlight matched characters (simple version without actual terminal colors)
        for &idx in indices {
            if idx >= last_idx {
                // Add non-matched chars
                content.extend(chars[last_idx..idx].iter());
                // Add matched char (could be highlighted in a real terminal)
                if idx < chars.len() {
                    content.push(chars[idx]);
                }
                last_idx = idx + 1;
            }
        }

        // Add remaining chars
        if last_idx < chars.len() {
            content.extend(chars[last_idx..].iter());
        }

        content
    }
}

#[async_trait]
impl MessageHandler<FaeMessage> for FilepathSearchHandler {
    async fn on_message(
        &mut self,
        message: Message<FaeMessage>,
        controller: &ActorController<FaeMessage>,
    ) {
        match message.method.as_str() {
            "updateSearchParams" => {
                if let FaeMessage::UpdateSearchParams(query) = message.payload {
                    log::info!(
                        "Starting filepath search: {} (mode: {:?})",
                        query.query,
                        query.mode
                    );
                    let _ = controller
                        .send_message("clearResults".to_string(), FaeMessage::ClearResults)
                        .await;

                    // Only handle Filepath mode
                    match query.mode {
                        SearchMode::Filepath => {
                            // Continue with filepath search
                        }
                        _ => {
                            log::debug!(
                                "Filepath search skipping search for unsupported mode: {:?}",
                                query.mode
                            );
                            return;
                        }
                    }

                    // Perform the filepath search
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
}

/// Filepath search actor for fuzzy path matching
pub type FilepathSearchActor = Actor<FaeMessage, FilepathSearchHandler>;

impl FilepathSearchActor {
    /// Create a new FilepathSearchActor
    pub fn new_filepath_search_actor(
        message_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
        sender: mpsc::UnboundedSender<Message<FaeMessage>>,
        search_path: impl Into<String>,
    ) -> Self {
        let search_path_str = search_path.into();
        let handler = FilepathSearchHandler::new(search_path_str);

        Self::new(message_receiver, sender, handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn test_format_match_content() {
        // Test file formatting
        let content = FilepathSearchHandler::format_match_content("test.rs", &[0, 2, 5], false);
        assert!(content.contains("[FILE]"));
        assert!(content.contains("test.rs"));

        // Test directory formatting
        let content = FilepathSearchHandler::format_match_content("src/lib", &[0, 4], true);
        assert!(content.contains("[DIR]"));
        assert!(content.contains("src/lib"));
    }

    #[tokio::test]
    async fn test_filepath_search_actor_integration() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create FilepathSearchActor
        let mut actor = FilepathSearchActor::new_filepath_search_actor(
            actor_rx,
            external_tx,
            "./src", // Search in src directory
        );

        // Send search query for Filepath mode
        let search_query = SearchParams {
            query: "actor".to_string(),
            mode: SearchMode::Filepath,
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
                        println!("Found match: {} - {}", result.filename, result.content);
                        result_count += 1;
                    }
                }
            } else {
                break;
            }
        }

        println!("Total filepath search results: {}", result_count);
        // Should find files containing "actor" in their path
        assert!(
            result_count > 0,
            "Should find at least one match for 'actor' in filepaths"
        );

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_filepath_search_skips_unsupported_modes() {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create FilepathSearchActor
        let mut actor =
            FilepathSearchActor::new_filepath_search_actor(actor_rx, external_tx, "./src");

        // Test Literal mode - should be skipped
        let literal_query = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::Literal,
        };
        let literal_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(literal_query),
        );
        actor_tx.send(literal_message).expect("Should send message");

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
        assert_eq!(
            result_count, 0,
            "Filepath search should skip non-Filepath modes"
        );

        // Clean up
        actor.shutdown();
    }

    #[tokio::test]
    async fn test_search_filepaths_streaming_functionality() {
        // Test the core filepath search streaming functionality
        let (tx, mut rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let controller = ActorController::new(tx);

        let result_count = tokio::task::spawn_blocking(move || {
            FilepathSearchHandler::search_filepaths("./src", "rs", SearchMode::Filepath, controller)
        })
        .await
        .expect("Task should not panic")
        .expect("Search should succeed");

        println!("Found {} filepath matches for 'rs'", result_count);
        assert!(
            result_count > 0,
            "Should find matches for 'rs' in filepaths"
        );

        // Collect the streaming results
        let mut received_results = Vec::new();
        while let Ok(message) = rx.try_recv() {
            if message.method == "pushSearchResult" {
                if let FaeMessage::PushSearchResult(result) = message.payload {
                    received_results.push(result);
                }
            }
        }

        // Verify that results have correct structure
        assert_eq!(
            received_results.len(),
            result_count,
            "Result count should match streamed results"
        );
        for result in received_results.iter().take(3) {
            assert!(!result.filename.is_empty());
            assert_eq!(result.line, 1); // Filepaths should have line 1
            assert!(!result.content.is_empty());
            println!(
                "  {} (score: {}) - {}",
                result.filename, result.column, result.content
            );
        }
    }

    #[tokio::test]
    async fn test_search_filepaths_streaming_wrong_mode() {
        // Test that non-Filepath modes return empty results
        let (tx, mut rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let controller = ActorController::new(tx);

        let result_count = tokio::task::spawn_blocking(move || {
            FilepathSearchHandler::search_filepaths(
                "./src",
                "test",
                SearchMode::Literal,
                controller,
            )
        })
        .await
        .expect("Task should not panic")
        .expect("Search should succeed");

        assert_eq!(
            result_count, 0,
            "Should return no results for non-Filepath mode"
        );

        // Verify no messages were sent
        assert!(
            rx.try_recv().is_err(),
            "Should not receive any messages for unsupported mode"
        );
    }
}
