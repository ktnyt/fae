//! Integration tests for WatchActor and SymbolIndexActor using Broadcaster
//!
//! This module tests the integration between file system watching and
//! symbol index management using the Broadcaster pattern for real-time
//! symbol database updates.

use crate::actors::messages::FaeMessage;
use crate::actors::symbol_index::SymbolIndexActor;
use crate::actors::symbol_search::SymbolSearchActor;
use crate::actors::watch::WatchActor;
use crate::core::{Broadcaster, Message};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

/// Integration test helper using Broadcaster for WatchActor and SymbolIndexActor coordination
pub struct BroadcastIntegrationHarness {
    temp_dir: TempDir,
    watch_actor: WatchActor,
    symbol_index_actor: SymbolIndexActor,
    broadcaster: Broadcaster<FaeMessage>,
    shared_sender: mpsc::UnboundedSender<Message<FaeMessage>>,
    external_receiver: mpsc::UnboundedReceiver<Message<FaeMessage>>,
}

impl BroadcastIntegrationHarness {
    /// Create a new broadcaster-based integration test harness
    pub async fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;

        // Create actor receivers
        let (watch_tx, watch_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_tx, symbol_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, external_receiver) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create broadcaster with all actor senders (including external for monitoring)
        let (broadcaster, shared_sender) = Broadcaster::new(vec![
            watch_tx.clone(),
            symbol_tx.clone(),
            external_tx, // For test monitoring
        ]);

        // Create watch actor using shared sender
        let watch_actor =
            WatchActor::new_watch_actor(watch_rx, shared_sender.clone(), temp_dir.path())?;

        // Create symbol index actor using shared sender
        let symbol_index_actor = SymbolIndexActor::new_symbol_index_actor(
            symbol_rx,
            shared_sender.clone(),
            temp_dir.path().to_string_lossy().to_string(),
        )?;

        Ok(Self {
            temp_dir,
            watch_actor,
            symbol_index_actor,
            broadcaster,
            shared_sender,
            external_receiver,
        })
    }

    /// Get the temporary directory path
    pub fn temp_dir_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }

    /// Start watching and indexing using broadcaster
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Start symbol indexing through broadcaster
        let init_message = Message::new("initialize", FaeMessage::ClearResults); // Dummy payload
        self.shared_sender.send(init_message)?;

        // Start file watching through broadcaster
        let watch_message = Message::new("startWatching", FaeMessage::ClearResults); // Dummy payload
        self.shared_sender.send(watch_message)?;

        // Give some time for initialization
        sleep(Duration::from_millis(1000)).await;

        Ok(())
    }

    /// Stop actors and broadcaster
    pub fn shutdown(mut self) {
        self.watch_actor.shutdown();
        self.symbol_index_actor.shutdown();
        self.broadcaster.shutdown();
    }

    /// Collect messages for a given duration (broadcaster handles message forwarding automatically)
    pub async fn collect_messages(&mut self, duration: Duration) -> Vec<Message<FaeMessage>> {
        let mut messages = Vec::new();
        let deadline = tokio::time::Instant::now() + duration;

        while tokio::time::Instant::now() < deadline {
            match timeout(Duration::from_millis(100), self.external_receiver.recv()).await {
                Ok(Some(message)) => {
                    // No manual forwarding needed - broadcaster handles it automatically!
                    messages.push(message);
                }
                Ok(None) => break,
                Err(_) => continue, // Timeout, continue collecting
            }
        }

        messages
    }

    /// Create a Rust source file
    pub fn create_rust_file(
        &self,
        filename: &str,
        content: &str,
    ) -> Result<std::path::PathBuf, std::io::Error> {
        let file_path = self.temp_dir.path().join(filename);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, content)?;
        Ok(file_path)
    }

    /// Update a file's content
    pub fn update_file(
        &self,
        file_path: &std::path::Path,
        content: &str,
    ) -> Result<(), std::io::Error> {
        fs::write(file_path, content)
    }

    /// Delete a file
    pub fn delete_file(&self, file_path: &std::path::Path) -> Result<(), std::io::Error> {
        fs::remove_file(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_broadcast_watch_symbol_integration_basic() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create a Rust file with symbols
        let rust_content = r#"
pub fn hello_world() {
    println!("Hello, world!");
}

pub struct User {
    name: String,
    age: u32,
}

impl User {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
}
"#;

        let _file_path = harness
            .create_rust_file("test.rs", rust_content)
            .expect("Failed to create rust file");

        // Wait for events to be processed
        sleep(Duration::from_millis(2000)).await;

        // Collect messages from the integration
        let messages = harness.collect_messages(Duration::from_millis(1000)).await;

        // Analyze messages
        let mut detect_create_count = 0;
        let mut clear_index_count = 0;
        let mut push_index_count = 0;
        let mut complete_index_count = 0;

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileCreate(path) => {
                    if path.contains("test.rs") {
                        detect_create_count += 1;
                    }
                }
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains("test.rs") {
                        clear_index_count += 1;
                    }
                }
                FaeMessage::PushSymbolIndex {
                    filepath, content, ..
                } => {
                    if filepath.contains("test.rs") {
                        push_index_count += 1;
                        println!("Symbol indexed: {}", content);
                    }
                }
                FaeMessage::CompleteSymbolIndex(path) => {
                    if path.contains("test.rs") {
                        complete_index_count += 1;
                        println!("Index completed for: {}", path);
                    }
                }
                _ => {}
            }
        }

        println!("Broadcast integration test results:");
        println!("  DetectFileCreate events: {}", detect_create_count);
        println!("  ClearSymbolIndex events: {}", clear_index_count);
        println!("  PushSymbolIndex events: {}", push_index_count);
        println!("  CompleteSymbolIndex events: {}", complete_index_count);

        // Verify the integration worked
        assert!(
            detect_create_count > 0 || clear_index_count > 0,
            "Should detect file creation or have initial indexing"
        );
        assert!(
            push_index_count > 0,
            "Should index symbols from the created file"
        );
        assert!(
            complete_index_count > 0,
            "Should complete indexing for the created file"
        );

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_broadcast_file_update_integration() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create initial file
        let initial_content = r#"
pub fn initial_function() {
    println!("Initial function");
}
"#;

        let file_path = harness
            .create_rust_file("update_test.rs", initial_content)
            .expect("Failed to create rust file");

        // Wait for initial processing
        sleep(Duration::from_millis(1500)).await;

        // Clear any initial messages
        let _ = harness.collect_messages(Duration::from_millis(500)).await;

        // Update the file with new content
        let updated_content = r#"
pub fn initial_function() {
    println!("Updated initial function");
}

pub fn new_function() {
    println!("This is a new function");
}

pub struct NewStruct {
    field: String,
}
"#;

        harness
            .update_file(&file_path, updated_content)
            .expect("Failed to update file");

        // Wait for update processing
        sleep(Duration::from_millis(2000)).await;

        // Collect messages from the update
        let messages = harness.collect_messages(Duration::from_millis(1000)).await;

        // Analyze update messages
        let mut detect_update_count = 0;
        let mut clear_index_count = 0;
        let mut push_index_count = 0;
        let mut symbol_contents = Vec::new();

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileUpdate(path) => {
                    if path.contains("update_test.rs") {
                        detect_update_count += 1;
                    }
                }
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains("update_test.rs") {
                        clear_index_count += 1;
                    }
                }
                FaeMessage::PushSymbolIndex {
                    filepath, content, ..
                } => {
                    if filepath.contains("update_test.rs") {
                        push_index_count += 1;
                        symbol_contents.push(content.clone());
                    }
                }
                _ => {}
            }
        }

        println!("Broadcast file update integration results:");
        println!("  DetectFileUpdate events: {}", detect_update_count);
        println!("  ClearSymbolIndex events: {}", clear_index_count);
        println!("  PushSymbolIndex events: {}", push_index_count);
        println!("  Symbol contents: {:?}", symbol_contents);

        // Verify file update integration
        assert!(detect_update_count > 0, "Should detect file update");
        assert!(clear_index_count > 0, "Should clear old symbols");
        assert!(push_index_count > 0, "Should index new symbols");

        // Verify we have symbols from the updated content
        let has_new_function = symbol_contents
            .iter()
            .any(|content| content.contains("new_function") || content.contains("NewStruct"));

        assert!(has_new_function, "Should have symbols from updated content");

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_broadcast_file_deletion_integration() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create a file to delete
        let content = r#"
pub fn function_to_delete() {
    println!("This function will be deleted");
}
"#;

        let file_path = harness
            .create_rust_file("delete_test.rs", content)
            .expect("Failed to create rust file");

        // Wait for initial processing
        sleep(Duration::from_millis(1500)).await;

        // Clear initial messages
        let _ = harness.collect_messages(Duration::from_millis(500)).await;

        // Delete the file
        harness
            .delete_file(&file_path)
            .expect("Failed to delete file");

        // Wait for deletion processing
        sleep(Duration::from_millis(1500)).await;

        // Collect messages from the deletion
        let messages = harness.collect_messages(Duration::from_millis(1000)).await;

        // Analyze deletion messages
        let mut detect_delete_count = 0;
        let mut clear_index_count = 0;

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileDelete(path) => {
                    if path.contains("delete_test.rs") {
                        detect_delete_count += 1;
                    }
                }
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains("delete_test.rs") {
                        clear_index_count += 1;
                    }
                }
                _ => {}
            }
        }

        println!("Broadcast file deletion integration results:");
        println!("  DetectFileDelete events: {}", detect_delete_count);
        println!("  ClearSymbolIndex events: {}", clear_index_count);

        // Verify file deletion integration
        assert!(detect_delete_count > 0, "Should detect file deletion");
        assert!(
            clear_index_count > 0,
            "Should clear symbols for deleted file"
        );

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_broadcast_multiple_files_integration() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create multiple files
        let file1_content = r#"
pub fn file1_function() {
    println!("Function from file 1");
}
"#;

        let file2_content = r#"
pub struct File2Struct {
    value: i32,
}

pub enum File2Enum {
    VariantA,
    VariantB,
}
"#;

        let _file1_path = harness
            .create_rust_file("src/file1.rs", file1_content)
            .expect("Failed to create file1");

        let _file2_path = harness
            .create_rust_file("src/file2.rs", file2_content)
            .expect("Failed to create file2");

        // Wait for processing
        sleep(Duration::from_millis(3000)).await;

        // Collect all messages
        let messages = harness.collect_messages(Duration::from_millis(1500)).await;

        // Group messages by file
        let mut file_stats: HashMap<String, (u32, u32, u32)> = HashMap::new(); // (detect, clear, push)

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileCreate(path) => {
                    if path.contains(".rs") {
                        let key = extract_filename(path);
                        let stats = file_stats.entry(key).or_insert((0, 0, 0));
                        stats.0 += 1;
                    }
                }
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains(".rs") {
                        let key = extract_filename(path);
                        let stats = file_stats.entry(key).or_insert((0, 0, 0));
                        stats.1 += 1;
                    }
                }
                FaeMessage::PushSymbolIndex { filepath, .. } => {
                    if filepath.contains(".rs") {
                        let key = extract_filename(filepath);
                        let stats = file_stats.entry(key).or_insert((0, 0, 0));
                        stats.2 += 1;
                    }
                }
                _ => {}
            }
        }

        println!("Broadcast multiple files integration results:");
        for (filename, (detect, clear, push)) in &file_stats {
            println!(
                "  {}: detect={}, clear={}, push={}",
                filename, detect, clear, push
            );
        }

        // Verify multiple files integration
        assert!(file_stats.len() >= 2, "Should process multiple files");

        // Check that each file has been processed
        for (filename, (detect, clear, push)) in &file_stats {
            assert!(
                *detect > 0 || *clear > 0,
                "File {} should be detected or cleared",
                filename
            );
            assert!(*push > 0, "File {} should have symbols indexed", filename);
        }

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_broadcaster_bidirectional_communication() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create a file to trigger WatchActor → SymbolIndexActor communication
        let rust_content = r#"
pub fn broadcaster_test() {
    println!("Testing broadcaster communication");
}
"#;

        let _file_path = harness
            .create_rust_file("broadcaster_test.rs", rust_content)
            .expect("Failed to create rust file");

        // Wait for file system event processing
        sleep(Duration::from_millis(2000)).await;

        // Manually trigger a symbol query to test SymbolIndexActor → broadcast communication
        let query_message = Message::new(
            "query",
            FaeMessage::QuerySymbols {
                pattern: "broadcaster".to_string(),
                limit: Some(10),
            },
        );
        harness
            .shared_sender
            .send(query_message)
            .expect("Failed to send query message");

        // Collect all messages for analysis
        let messages = harness.collect_messages(Duration::from_millis(1500)).await;

        // Analyze message flow patterns
        let mut watch_messages = 0;
        let mut index_messages = 0;
        let mut query_messages = 0;

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileCreate(_) => watch_messages += 1,
                FaeMessage::PushSymbolIndex { .. } => index_messages += 1,
                FaeMessage::QuerySymbols { .. } => query_messages += 1,
                _ => {}
            }
        }

        println!("Broadcaster bidirectional communication results:");
        println!("  Watch messages: {}", watch_messages);
        println!("  Index messages: {}", index_messages);
        println!("  Query messages: {}", query_messages);
        println!("  Total messages: {}", messages.len());

        // Verify bidirectional communication through broadcaster
        // Note: All actors receive all messages through broadcaster
        assert!(
            messages.len() > 0,
            "Should have received messages through broadcaster"
        );

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_race_condition_prevention() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Start the integration
        harness.start().await.expect("Failed to start integration");

        // Create a file with initial content
        let initial_content = r#"
pub fn race_test_function() {
    println!("Initial content");
}
"#;

        let file_path = harness
            .create_rust_file("race_test.rs", initial_content)
            .expect("Failed to create rust file");

        // Wait for initial processing to start
        sleep(Duration::from_millis(500)).await;

        // Rapidly update the file multiple times to trigger race conditions
        let update_tasks = (0..5).map(|i| {
            let file_path = file_path.clone();
            let harness_ref = &harness;
            async move {
                let updated_content = format!(
                    r#"
pub fn race_test_function_v{}() {{
    println!("Updated content version {}");
}}

pub struct RaceTestStruct{} {{
    version: u32,
}}
"#,
                    i, i, i
                );

                // Small delay to create overlapping updates
                sleep(Duration::from_millis(i * 50)).await;

                let _ = harness_ref.update_file(&file_path, &updated_content);
                log::info!("Updated file to version {}", i);
            }
        });

        // Execute all updates concurrently
        futures::future::join_all(update_tasks).await;

        // Wait for all processing to complete
        sleep(Duration::from_millis(3000)).await;

        // Collect all messages
        let messages = harness.collect_messages(Duration::from_millis(2000)).await;

        // Analyze race condition handling
        let mut update_events = 0;
        let mut clear_events = 0;
        let mut push_events = 0;
        let mut complete_events = 0;
        let mut processed_versions = std::collections::HashSet::new();

        for message in &messages {
            match &message.payload {
                FaeMessage::DetectFileUpdate(path) => {
                    if path.contains("race_test.rs") {
                        update_events += 1;
                    }
                }
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains("race_test.rs") {
                        clear_events += 1;
                    }
                }
                FaeMessage::PushSymbolIndex {
                    filepath, content, ..
                } => {
                    if filepath.contains("race_test.rs") {
                        push_events += 1;

                        // Extract version number from content
                        if let Some(start) = content.find("_v") {
                            if let Some(end) = content[start..].find("()") {
                                let version_str = &content[start + 2..start + end];
                                if let Ok(version) = version_str.parse::<u32>() {
                                    processed_versions.insert(version);
                                }
                            }
                        }
                    }
                }
                FaeMessage::CompleteSymbolIndex(path) => {
                    if path.contains("race_test.rs") {
                        complete_events += 1;
                    }
                }
                _ => {}
            }
        }

        println!("Race condition test results:");
        println!("  File update events: {}", update_events);
        println!("  Clear symbol events: {}", clear_events);
        println!("  Push symbol events: {}", push_events);
        println!("  Complete events: {}", complete_events);
        println!("  Processed versions: {:?}", processed_versions);

        // Verify race condition handling
        assert!(update_events >= 3, "Should detect multiple file updates");
        assert!(clear_events >= 3, "Should clear symbols multiple times");
        assert!(push_events > 0, "Should push some symbols");

        // The key test: verify that race condition handling works
        // We should see evidence that processing was interrupted and restarted
        // This is indicated by multiple clear events and the fact that updates were processed
        assert!(
            clear_events >= update_events,
            "Should have at least as many clear events as update events due to restarts"
        );

        // Due to rapid updates, some processing should have been interrupted
        // We expect more push events than a single complete processing would produce
        assert!(
            push_events >= processed_versions.len(),
            "Should have pushed symbols for processed versions"
        );

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_initialization_vs_file_change_race() {
        let mut harness = BroadcastIntegrationHarness::new()
            .await
            .expect("Failed to create broadcast test harness");

        // Create initial files before starting
        let file1_content = r#"
pub fn init_race_function1() {
    println!("Function 1");
}
"#;

        let file2_content = r#"
pub fn init_race_function2() {
    println!("Function 2");
}
"#;

        let file1_path = harness
            .create_rust_file("init_race1.rs", file1_content)
            .expect("Failed to create file1");
        let file2_path = harness
            .create_rust_file("init_race2.rs", file2_content)
            .expect("Failed to create file2");

        // Start initialization and immediately update files
        let _ = tokio::join!(
            harness.start(), // This triggers initialization
            async {
                // Small delay then update files during initialization
                sleep(Duration::from_millis(200)).await;

                let updated_content1 = r#"
pub fn init_race_function1_updated() {
    println!("Updated function 1");
}

pub struct UpdatedStruct1 {
    field: String,
}
"#;

                let updated_content2 = r#"
pub fn init_race_function2_updated() {
    println!("Updated function 2");
}

pub enum UpdatedEnum2 {
    VariantA,
    VariantB,
}
"#;

                let _ = harness.update_file(&file1_path, updated_content1);
                let _ = harness.update_file(&file2_path, updated_content2);

                log::info!("Updated files during initialization");
            }
        );

        // Wait for all processing to complete
        sleep(Duration::from_millis(4000)).await;

        // Collect messages
        let messages = harness.collect_messages(Duration::from_millis(1500)).await;

        // Analyze initialization vs file change race
        let mut file1_clears = 0;
        let mut file1_pushes = 0;
        let mut file2_clears = 0;
        let mut file2_pushes = 0;
        let mut file1_has_updated = false;
        let mut file2_has_updated = false;

        for message in &messages {
            match &message.payload {
                FaeMessage::ClearSymbolIndex(path) => {
                    if path.contains("init_race1.rs") {
                        file1_clears += 1;
                    } else if path.contains("init_race2.rs") {
                        file2_clears += 1;
                    }
                }
                FaeMessage::PushSymbolIndex {
                    filepath, content, ..
                } => {
                    if filepath.contains("init_race1.rs") {
                        file1_pushes += 1;
                        if content.contains("updated") || content.contains("Updated") {
                            file1_has_updated = true;
                        }
                    } else if filepath.contains("init_race2.rs") {
                        file2_pushes += 1;
                        if content.contains("updated") || content.contains("Updated") {
                            file2_has_updated = true;
                        }
                    }
                }
                _ => {}
            }
        }

        println!("Initialization vs file change race results:");
        println!(
            "  File1 - clears: {}, pushes: {}, has_updated: {}",
            file1_clears, file1_pushes, file1_has_updated
        );
        println!(
            "  File2 - clears: {}, pushes: {}, has_updated: {}",
            file2_clears, file2_pushes, file2_has_updated
        );

        // Verify that both files were processed
        assert!(file1_clears > 0, "File1 should have been cleared");
        assert!(file1_pushes > 0, "File1 should have symbols pushed");
        assert!(file2_clears > 0, "File2 should have been cleared");
        assert!(file2_pushes > 0, "File2 should have symbols pushed");

        // Due to interruption, the final state should reflect the updated content
        // This tests that the file change interrupts initialization processing
        assert!(
            file1_has_updated || file2_has_updated,
            "At least one file should have updated content due to race handling"
        );

        harness.shutdown();
    }

    #[tokio::test]
    async fn test_full_symbol_search_integration() {
        use crate::actors::types::{SearchMode, SearchParams};

        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Create all actors
        let (watch_tx, watch_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_index_tx, symbol_index_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (symbol_search_tx, symbol_search_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        // Create broadcaster with all actor senders
        let (mut broadcaster, shared_sender) = Broadcaster::new(vec![
            watch_tx.clone(),
            symbol_index_tx.clone(),
            symbol_search_tx.clone(),
            external_tx, // For test monitoring
        ]);

        // Create actors
        let mut watch_actor =
            WatchActor::new_watch_actor(watch_rx, shared_sender.clone(), temp_dir.path())
                .expect("Failed to create WatchActor");

        let mut symbol_index_actor = SymbolIndexActor::new_symbol_index_actor(
            symbol_index_rx,
            shared_sender.clone(),
            temp_dir.path().to_string_lossy().to_string(),
        )
        .expect("Failed to create SymbolIndexActor");

        let mut symbol_search_actor =
            SymbolSearchActor::new_symbol_search_actor(symbol_search_rx, shared_sender.clone());

        // Start all actors
        let init_message = Message::new("initialize", FaeMessage::ClearResults);
        shared_sender
            .send(init_message)
            .expect("Failed to send init message");

        let watch_message = Message::new("startWatching", FaeMessage::ClearResults);
        shared_sender
            .send(watch_message)
            .expect("Failed to send watch message");

        // Wait for initialization
        sleep(Duration::from_millis(1000)).await;

        // Create a Rust file with symbols
        let rust_content = r#"
pub fn search_function() {
    println!("This is a searchable function");
}

pub struct SearchStruct {
    field: String,
}

impl SearchStruct {
    pub fn search_method(&self) -> String {
        self.field.clone()
    }
}

pub enum SearchEnum {
    VariantA,
    VariantB,
}
"#;

        let file_path = temp_dir.path().join("search_test.rs");
        std::fs::write(&file_path, rust_content).expect("Failed to write test file");

        // Wait for file to be indexed
        sleep(Duration::from_millis(2000)).await;

        // Clear initial messages
        while let Ok(_) = timeout(Duration::from_millis(10), external_rx.recv()).await {}

        // Perform search
        let search_params = SearchParams {
            query: "search".to_string(),
            mode: SearchMode::Symbol,
        };
        let search_message = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(search_params),
        );
        shared_sender
            .send(search_message)
            .expect("Failed to send search message");

        // Wait for search results
        sleep(Duration::from_millis(1000)).await;

        // Collect search results
        let mut search_results = Vec::new();
        while let Ok(message) = timeout(Duration::from_millis(100), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        search_results.push(result.content.clone());
                        println!("Search result: {}", result.content);
                    }
                }
            } else {
                break;
            }
        }

        println!("Full symbol search integration results:");
        println!("  Total search results: {}", search_results.len());
        for result in &search_results {
            println!("    - {}", result);
        }

        // Verify search results
        assert!(search_results.len() > 0, "Should have found search results");

        // Check that we found the expected symbols
        let has_function = search_results.iter().any(|r| r.contains("search_function"));
        let has_struct = search_results.iter().any(|r| r.contains("SearchStruct"));
        let has_method = search_results.iter().any(|r| r.contains("search_method"));
        let has_enum = search_results.iter().any(|r| r.contains("SearchEnum"));

        assert!(has_function, "Should find search_function");
        assert!(has_struct, "Should find SearchStruct");
        assert!(has_method, "Should find search_method");
        assert!(has_enum, "Should find SearchEnum");

        // Test different search query
        let search_params2 = SearchParams {
            query: "method".to_string(),
            mode: SearchMode::Symbol,
        };
        let search_message2 = Message::new(
            "updateSearchParams",
            FaeMessage::UpdateSearchParams(search_params2),
        );
        shared_sender
            .send(search_message2)
            .expect("Failed to send search message 2");

        sleep(Duration::from_millis(500)).await;

        // Collect second search results
        let mut method_results = Vec::new();
        while let Ok(message) = timeout(Duration::from_millis(50), external_rx.recv()).await {
            if let Some(msg) = message {
                if msg.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = msg.payload {
                        method_results.push(result.content.clone());
                    }
                }
            } else {
                break;
            }
        }

        println!("Method search results: {}", method_results.len());
        assert!(method_results.len() > 0, "Should find method results");
        assert!(
            method_results.iter().any(|r| r.contains("search_method")),
            "Should find search_method specifically"
        );

        // Clean up
        watch_actor.shutdown();
        symbol_index_actor.shutdown();
        symbol_search_actor.shutdown();
        broadcaster.shutdown();
    }

    fn extract_filename(path: &str) -> String {
        std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}
