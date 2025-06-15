//! Example demonstrating NativeSearchActor usage
//!
//! This example shows how to use the NativeSearchActor to perform
//! real-time code search using pure Rust implementation.
//! Serves as a fallback when neither ripgrep nor ag are available.

use fae::actors::messages::{SearchMessage, SearchMode};
use fae::actors::NativeSearchActor;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 Starting NativeSearchActor search example");

    // Create channels for actor communication
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (actor_tx, actor_rx) = mpsc::unbounded_channel();

    // Create the NativeSearchActor with regex mode for function pattern matching
    let mut actor = NativeSearchActor::create(actor_rx, tx, SearchMode::Regexp);

    println!("✨ NativeSearchActor created successfully with Regexp mode");

    // Start a task to listen for search results
    let result_listener = tokio::spawn(async move {
        let mut results_count = 0;
        while let Some(message) = rx.recv().await {
            if let Some(search_msg) = message.payload.as_search() {
                match search_msg {
                    SearchMessage::PushSearchResult { result } => {
                        results_count += 1;
                        println!(
                            "📄 {}:{}:{} | {}",
                            result.filename,
                            result.line,
                            result.offset,
                            result.content.trim()
                        );

                        // Stop after receiving 15 results for demo purposes
                        if results_count >= 15 {
                            println!("🎯 Stopping after {} results", results_count);
                            break;
                        }
                    }
                    _ => {
                        println!("📨 Received other message: {:?}", message.method);
                    }
                }
            } else {
                println!("📨 Received non-search message: {:?}", message.method);
            }
        }
        results_count
    });

    // Perform a regex search for Rust function definitions with patterns
    println!("🚀 Searching for 'fn \\\\w+.*\\\\{{' (Rust function patterns)...");

    let query_message = fae::core::Message::new(
        "updateQuery",
        fae::actors::messages::FaeMessage::update_query(r"fn \w+.*\{".to_string(), SearchMode::Regexp),
    );

    if let Err(e) = actor_tx.send(query_message) {
        println!("❌ Failed to send regex search message: {}", e);
    } else {
        println!("✅ Native regex search command executed successfully");
    }

    // Wait a bit for all results to come in
    println!("⏳ Waiting for search results...");
    sleep(Duration::from_millis(1000)).await;

    // Perform a literal search for demonstration
    println!("\n🔍 Now performing literal search for 'use tokio'...");

    let literal_message = fae::core::Message::new(
        "updateQuery",
        fae::actors::messages::FaeMessage::update_query("use tokio".to_string(), SearchMode::Literal),
    );

    if let Err(e) = actor_tx.send(literal_message) {
        println!("❌ Failed to send literal search message: {}", e);
    } else {
        println!("✅ Native literal search command executed successfully");
    }

    // Wait a bit more for literal search results
    sleep(Duration::from_millis(800)).await;

    // Wait for the listener to finish or timeout
    let results_count =
        match tokio::time::timeout(Duration::from_millis(500), result_listener).await {
            Ok(count) => count?,
            Err(_) => {
                println!("🕐 Search timed out, some results may still be processing");
                0
            }
        };

    println!(
        "\n🏁 Search completed. Total results processed: {}",
        results_count
    );

    // Shutdown the actor properly to clean up resources
    println!("🔄 Shutting down actor...");
    actor.shutdown();
    
    println!("✨ Example completed successfully!");

    Ok(())
}
