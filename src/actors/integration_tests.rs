//! Integration tests for WatchActor and SymbolIndexActor using Broadcaster
//!
//! This module tests the integration between file system watching and
//! symbol index management using the Broadcaster pattern for real-time
//! symbol database updates.

use crate::actors::messages::FaeMessage;
use crate::actors::symbol_index::SymbolIndexActor;
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
        let watch_actor = WatchActor::new_watch_actor(
            watch_rx,
            shared_sender.clone(),
            temp_dir.path(),
        )?;
        
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
    pub fn create_rust_file(&self, filename: &str, content: &str) -> Result<std::path::PathBuf, std::io::Error> {
        let file_path = self.temp_dir.path().join(filename);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, content)?;
        Ok(file_path)
    }
    
    /// Update a file's content
    pub fn update_file(&self, file_path: &std::path::Path, content: &str) -> Result<(), std::io::Error> {
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
        
        let _file_path = harness.create_rust_file("test.rs", rust_content)
            .expect("Failed to create rust file");
        
        // Wait for events to be processed
        sleep(Duration::from_millis(2000)).await;
        
        // Collect messages from the integration
        let messages = harness.collect_messages(Duration::from_millis(1000)).await;
        
        // Analyze messages
        let mut detect_create_count = 0;
        let mut clear_index_count = 0;
        let mut push_index_count = 0;
        
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
                FaeMessage::PushSymbolIndex { filepath, content, .. } => {
                    if filepath.contains("test.rs") {
                        push_index_count += 1;
                        println!("Symbol indexed: {}", content);
                    }
                }
                _ => {}
            }
        }
        
        println!("Broadcast integration test results:");
        println!("  DetectFileCreate events: {}", detect_create_count);
        println!("  ClearSymbolIndex events: {}", clear_index_count);
        println!("  PushSymbolIndex events: {}", push_index_count);
        
        // Verify the integration worked
        assert!(detect_create_count > 0 || clear_index_count > 0, 
               "Should detect file creation or have initial indexing");
        assert!(push_index_count > 0, 
               "Should index symbols from the created file");
        
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
        
        let file_path = harness.create_rust_file("update_test.rs", initial_content)
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
        
        harness.update_file(&file_path, updated_content)
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
                FaeMessage::PushSymbolIndex { filepath, content, .. } => {
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
        let has_new_function = symbol_contents.iter()
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
        
        let file_path = harness.create_rust_file("delete_test.rs", content)
            .expect("Failed to create rust file");
        
        // Wait for initial processing
        sleep(Duration::from_millis(1500)).await;
        
        // Clear initial messages
        let _ = harness.collect_messages(Duration::from_millis(500)).await;
        
        // Delete the file
        harness.delete_file(&file_path)
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
        assert!(clear_index_count > 0, "Should clear symbols for deleted file");
        
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
        
        let _file1_path = harness.create_rust_file("src/file1.rs", file1_content)
            .expect("Failed to create file1");
        
        let _file2_path = harness.create_rust_file("src/file2.rs", file2_content)
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
            println!("  {}: detect={}, clear={}, push={}", filename, detect, clear, push);
        }
        
        // Verify multiple files integration
        assert!(file_stats.len() >= 2, "Should process multiple files");
        
        // Check that each file has been processed
        for (filename, (detect, clear, push)) in &file_stats {
            assert!(*detect > 0 || *clear > 0, 
                   "File {} should be detected or cleared", filename);
            assert!(*push > 0, 
                   "File {} should have symbols indexed", filename);
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
        
        let _file_path = harness.create_rust_file("broadcaster_test.rs", rust_content)
            .expect("Failed to create rust file");
        
        // Wait for file system event processing
        sleep(Duration::from_millis(2000)).await;
        
        // Manually trigger a symbol query to test SymbolIndexActor → broadcast communication
        let query_message = Message::new("query", FaeMessage::QuerySymbols {
            pattern: "broadcaster".to_string(),
            limit: Some(10),
        });
        harness.shared_sender.send(query_message)
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
        assert!(messages.len() > 0, "Should have received messages through broadcaster");
        
        harness.shutdown();
    }
    
    fn extract_filename(path: &str) -> String {
        std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}