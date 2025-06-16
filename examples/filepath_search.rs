//! Filepath Search Example
//!
//! This example demonstrates how to use the FilepathSearchActor for fuzzy file and directory
//! path matching. It shows how to perform fuzzy searches against file paths in a project.

use fae::actors::filepath::FilepathSearchActor;
use fae::actors::messages::FaeMessage;
use fae::actors::types::{SearchMode, SearchParams};
use fae::core::Message;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🔍 Filepath Search Example");
    println!("==========================");
    println!();

    // Create channels for actor communication
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    // Create FilepathSearchActor for searching in current directory
    let mut filepath_actor = FilepathSearchActor::new_filepath_search_actor(
        actor_rx, result_tx, "./src", // Search in src directory
    );

    println!("🚀 Starting filepath search actor...");
    println!();

    // Example 1: Search for files containing "actor"
    println!("📁 Example 1: Searching for files containing 'actor'");
    let search_query = SearchParams {
        query: "actor".to_string(),
        mode: SearchMode::Filepath,
    };

    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_query),
    );

    actor_tx.send(search_message)?;

    // Collect results for a few seconds
    let mut results = Vec::new();
    let timeout_duration = Duration::from_millis(2000);
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        match timeout(Duration::from_millis(100), result_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        results.push(result);
                    }
                }
            }
            _ => break,
        }
    }

    // Display results
    println!("   Found {} matches:", results.len());
    for (i, result) in results.iter().take(10).enumerate() {
        println!(
            "   {}. {} (score: {})",
            i + 1,
            result.filename,
            result.column
        );
        println!("      {}", result.content);
    }
    println!();

    // Example 2: Search for Rust files
    println!("🦀 Example 2: Searching for Rust files (.rs)");
    let rust_search = SearchParams {
        query: "rs".to_string(),
        mode: SearchMode::Filepath,
    };

    let rust_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(rust_search),
    );

    actor_tx.send(rust_message)?;

    // Collect Rust file results
    let mut rust_results = Vec::new();
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        match timeout(Duration::from_millis(100), result_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        rust_results.push(result);
                    }
                }
            }
            _ => break,
        }
    }

    println!("   Found {} Rust files:", rust_results.len());
    for (i, result) in rust_results.iter().take(10).enumerate() {
        println!(
            "   {}. {} (score: {})",
            i + 1,
            result.filename,
            result.column
        );
        println!("      {}", result.content);
    }
    println!();

    // Example 3: Search for specific patterns
    println!("🎯 Example 3: Searching for 'native' files");
    let pattern_search = SearchParams {
        query: "native".to_string(),
        mode: SearchMode::Filepath,
    };

    let pattern_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(pattern_search),
    );

    actor_tx.send(pattern_message)?;

    // Collect pattern results
    let mut pattern_results = Vec::new();
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        match timeout(Duration::from_millis(100), result_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        pattern_results.push(result);
                    }
                }
            }
            _ => break,
        }
    }

    println!("   Found {} matches for 'native':", pattern_results.len());
    for (i, result) in pattern_results.iter().take(5).enumerate() {
        println!(
            "   {}. {} (score: {})",
            i + 1,
            result.filename,
            result.column
        );
        println!("      {}", result.content);
    }
    println!();

    // Example 4: Search for directories
    println!("📂 Example 4: Searching for directories containing 'core'");
    let dir_search = SearchParams {
        query: "core".to_string(),
        mode: SearchMode::Filepath,
    };

    let dir_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(dir_search),
    );

    actor_tx.send(dir_message)?;

    // Collect directory results
    let mut dir_results = Vec::new();
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        match timeout(Duration::from_millis(100), result_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        // Only show directories
                        if result.content.contains("[DIR]") {
                            dir_results.push(result);
                        }
                    }
                }
            }
            _ => break,
        }
    }

    println!("   Found {} directories:", dir_results.len());
    for (i, result) in dir_results.iter().take(5).enumerate() {
        println!(
            "   {}. {} (score: {})",
            i + 1,
            result.filename,
            result.column
        );
        println!("      {}", result.content);
    }
    println!();

    // Example 5: Fuzzy matching demonstration
    println!("✨ Example 5: Fuzzy matching demonstration");
    println!("   Searching for 'msg' (should match 'messages', etc.)");

    let fuzzy_search = SearchParams {
        query: "msg".to_string(),
        mode: SearchMode::Filepath,
    };

    let fuzzy_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(fuzzy_search),
    );

    actor_tx.send(fuzzy_message)?;

    // Collect fuzzy results
    let mut fuzzy_results = Vec::new();
    let start_time = std::time::Instant::now();

    while start_time.elapsed() < timeout_duration {
        match timeout(Duration::from_millis(100), result_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        fuzzy_results.push(result);
                    }
                }
            }
            _ => break,
        }
    }

    println!("   Found {} fuzzy matches:", fuzzy_results.len());
    for (i, result) in fuzzy_results.iter().take(5).enumerate() {
        println!(
            "   {}. {} (score: {})",
            i + 1,
            result.filename,
            result.column
        );
        println!("      {}", result.content);
    }
    println!();

    // Shutdown
    println!("🛑 Shutting down filepath search actor...");
    filepath_actor.shutdown();

    println!("✅ Filepath search example completed!");
    println!();
    println!("💡 Tips:");
    println!("   - Higher scores indicate better matches");
    println!("   - [FILE] indicates a file, [DIR] indicates a directory");
    println!("   - Fuzzy matching allows partial character matches");
    println!("   - .gitignore and .ignore files are respected");
    println!("   - Results are automatically sorted by relevance score");

    Ok(())
}
