//! Example demonstrating RipgrepActor usage
//!
//! This example shows how to use the RipgrepActor to perform
//! real-time code search with ripgrep integration.

use fae::actors::RipgrepActor;
use fae::actors::messages::{FaeMessage, SearchMessage, SearchMode, SearchResult};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 Starting RipgrepActor search example");

    // Create channels for actor communication
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

    // Create the RipgrepActor with regex mode for function pattern matching
    let actor = RipgrepActor::create(actor_rx, tx, SearchMode::Regexp);

    println!("✨ RipgrepActor created successfully with Regexp mode");

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

                        // Stop after receiving 10 results for demo purposes
                        if results_count >= 10 {
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
    println!("🚀 Searching for 'fn \\w+.*\\{{' (Rust function patterns)...");

    match actor.search(r"fn \w+.*\{".to_string(), SearchMode::Regexp).await {
        Ok(_) => {
            println!("✅ Regex search command executed successfully");
        }
        Err(e) => {
            println!("❌ Search failed: {}", e);
            println!("💡 Make sure 'rg' (ripgrep) is installed and available in PATH");
        }
    }

    // Send a manual search result for demonstration before waiting
    println!("\n📤 Sending manual search result...");
    let manual_result = SearchResult {
        filename: "examples/demo.rs".to_string(),
        line: 42,
        offset: 8,
        content: "    fn example_function() {".to_string(),
    };

    actor
        .actor()
        .send_message(
            "pushSearchResult".to_string(),
            FaeMessage::push_search_result(manual_result),
        )
        .await?;

    // Wait a bit for all results to come in
    sleep(Duration::from_millis(300)).await;

    // Wait for the listener to finish
    let results_count = result_listener.await?;
    println!(
        "\n🏁 Search completed. Total results processed: {}",
        results_count
    );

    // Clean up
    println!("🧹 Cleaning up...");
    let _ = actor.kill().await;

    println!("✨ Example completed successfully!");

    Ok(())
}
