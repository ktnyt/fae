//! Example demonstrating different search modes (Literal vs Regexp)
//!
//! This example shows the difference between literal and regex search modes
//! when using RipgrepActor.

use fae::actors::RipgrepActor;
use fae::messages::{SearchMessage, SearchMode};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

async fn demonstrate_search_mode(mode: SearchMode, query: &str, description: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 === {} ===", description);
    println!("Mode: {:?}", mode);
    println!("Query: '{}'", query);

    // Create channels for actor communication
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_actor_tx, actor_rx) = mpsc::unbounded_channel();

    // Create the RipgrepActor with specified mode
    let actor = RipgrepActor::create(actor_rx, tx, mode);

    // Start a task to listen for search results
    let result_listener = tokio::spawn(async move {
        let mut results_count = 0;
        while let Some(message) = rx.recv().await {
            if let Some(search_msg) = message.payload.as_search() {
                match search_msg {
                        SearchMessage::PushSearchResult { result } => {
                        results_count += 1;
                        println!(
                            "  📄 {}:{}:{} | {}",
                            result.filename,
                            result.line,
                            result.offset,
                            result.content.trim()
                        );
                        
                        // Stop after receiving 5 results for demo purposes
                        if results_count >= 5 {
                            println!("  🎯 Stopping after {} results", results_count);
                            break;
                        }
                    }
                    _ => {
                        // Ignore other messages
                    }
                }
            }
        }
        results_count
    });

    // Execute the search
    match actor.search(query.to_string()).await {
        Ok(_) => {
            println!("✅ Search executed successfully");
        }
        Err(e) => {
            println!("❌ Search failed: {}", e);
            return Err(e);
        }
    }

    // Wait for results
    sleep(Duration::from_millis(300)).await;

    // Wait for the listener to finish
    let results_count = result_listener.await?;
    println!("📊 Total results: {}", results_count);

    // Cleanup
    let _ = actor.kill().await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Demonstrating RipgrepActor Search Modes");

    // Example 1: Literal search for exact string matching
    demonstrate_search_mode(
        SearchMode::Literal,
        "fn ",
        "Literal Search: Find exact 'fn ' strings"
    ).await?;

    // Example 2: Regex search for pattern matching
    demonstrate_search_mode(
        SearchMode::Regexp,
        r"fn \w+\(",
        "Regex Search: Find function definitions with names"
    ).await?;

    // Example 3: Literal search for special characters
    demonstrate_search_mode(
        SearchMode::Literal,
        ".*",
        "Literal Search: Find literal '.*' (not regex)"
    ).await?;

    // Example 4: Regex search for complex patterns
    demonstrate_search_mode(
        SearchMode::Regexp,
        r"(pub|async)\s+fn",
        "Regex Search: Find public or async functions"
    ).await?;

    // Example 5: Literal search for brackets
    demonstrate_search_mode(
        SearchMode::Literal,
        "[",
        "Literal Search: Find literal '[' character"
    ).await?;

    // Example 6: Regex search for error patterns
    demonstrate_search_mode(
        SearchMode::Regexp,
        r"(Error|Result)<",
        "Regex Search: Find Error or Result types"
    ).await?;

    println!("\n✨ Search mode demonstration completed!");
    println!("💡 Key differences:");
    println!("  - Literal mode: Searches for exact string matches (faster, safer)");
    println!("  - Regexp mode: Searches using regular expression patterns (powerful, flexible)");

    Ok(())
}