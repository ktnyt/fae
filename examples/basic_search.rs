//! Basic search example
//!
//! This example demonstrates basic usage of RipgrepActor and AgActor
//! with simple search operations.

use fae::actors::messages::FaeMessage;
use fae::actors::types::{SearchMode, SearchParams};
use fae::actors::{AgActor, NativeSearchActor, RipgrepActor};
use fae::core::Message;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

async fn demonstrate_search_actor<T>(
    actor_name: &str,
    actor: T,
    actor_tx: mpsc::UnboundedSender<Message<FaeMessage>>,
    mut external_rx: mpsc::UnboundedReceiver<Message<FaeMessage>>,
    query: &str,
    mode: SearchMode,
) where
    T: Send + 'static,
{
    println!("🔍 {} Search Demo", actor_name);
    println!("Query: '{}' (mode: {:?})", query, mode);
    println!();

    // Send search query
    let search_query = SearchParams {
        query: query.to_string(),
        mode,
    };
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_query),
    );

    if let Err(e) = actor_tx.send(search_message) {
        println!("❌ Failed to send search message: {}", e);
        return;
    }

    println!("🚀 Search started, collecting results...");
    println!();

    // Collect results for a short time
    let mut result_count = 0;
    let max_results = 5; // Show only first 5 results for demo

    while result_count < max_results {
        match timeout(Duration::from_millis(300), external_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        result_count += 1;
                        println!(
                            "  {}. {}:{}:{} | {}",
                            result_count,
                            result.filename,
                            result.line,
                            result.offset,
                            result.content.trim()
                        );
                    }
                } else if message.method == "clearResults" {
                    println!("  🧹 Results cleared");
                }
            }
            Ok(None) => {
                println!("  📪 No more results");
                break;
            }
            Err(_) => {
                if result_count == 0 {
                    println!("  ⏰ No results found within timeout");
                }
                break;
            }
        }
    }

    if result_count == max_results {
        println!("  ... (showing first {} results only)", max_results);
    }

    println!();
    println!(
        "✅ {} search completed with {} results",
        actor_name, result_count
    );

    // Clean up - this requires the actor to have a shutdown method
    // Since we can't call shutdown on a generic T, we'll just let it drop
    drop(actor);

    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 Basic Search Demo");
    println!("===================");
    println!();

    let search_path = "./src";
    let query = "Command";
    let mode = SearchMode::Literal;

    // Check if external tools are available
    let rg_available = tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false);

    let ag_available = tokio::process::Command::new("ag")
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false);

    println!("🔧 Tool availability:");
    println!(
        "  ripgrep (rg):     {}",
        if rg_available { "✅" } else { "❌" }
    );
    println!(
        "  ag:               {}",
        if ag_available { "✅" } else { "❌" }
    );
    println!("  native (Rust):    ✅ (always available)");
    println!();

    // Test RipgrepActor
    if rg_available {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = RipgrepActor::new_ripgrep_actor(actor_rx, external_tx, search_path);

        demonstrate_search_actor("RipgrepActor", actor, actor_tx, external_rx, query, mode).await;
    } else {
        println!("⚠️  Skipping RipgrepActor demo - ripgrep not available");
        println!();
    }

    // Test AgActor
    if ag_available {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = AgActor::new_ag_actor(actor_rx, external_tx, search_path);

        demonstrate_search_actor("AgActor", actor, actor_tx, external_rx, query, mode).await;
    } else {
        println!("⚠️  Skipping AgActor demo - ag not available");
        println!();
    }

    // Test NativeSearchActor (always available)
    {
        let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
        let (external_tx, external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

        let actor = NativeSearchActor::new_native_search_actor(actor_rx, external_tx, search_path);

        demonstrate_search_actor("Native", actor, actor_tx, external_rx, query, mode).await;
    }

    // Summary
    if !rg_available && !ag_available {
        println!("ℹ️  Only native search is available (which is always sufficient!)");
        println!("💡 For faster search on large codebases, consider installing:");
        println!("   ripgrep: cargo install ripgrep");
        println!("   ag: brew install the_silver_searcher (macOS)");
        println!("       apt-get install silversearcher-ag (Ubuntu)");
    } else {
        println!("🎉 Demo completed successfully!");
    }

    Ok(())
}
