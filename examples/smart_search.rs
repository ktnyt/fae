//! Smart search example
//!
//! This example demonstrates intelligent search tool selection based on availability
//! and automatically falls back to native search when external tools are unavailable.

use fae::actors::messages::FaeMessage;
use fae::actors::types::{SearchMode, SearchParams};
use fae::actors::{AgActor, NativeSearchActor, RipgrepActor};
use fae::core::Message;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[derive(Debug, Clone)]
enum SearchTool {
    Ripgrep,
    Ag,
    Native,
}

impl SearchTool {
    fn name(&self) -> &'static str {
        match self {
            SearchTool::Ripgrep => "Ripgrep",
            SearchTool::Ag => "Ag",
            SearchTool::Native => "Native",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            SearchTool::Ripgrep => "blazingly fast external tool",
            SearchTool::Ag => "fast external tool",
            SearchTool::Native => "pure Rust implementation",
        }
    }
}

/// Check tool availability and select the best available tool
async fn select_best_search_tool() -> SearchTool {
    // Check ripgrep availability
    let rg_available = tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false);

    // Check ag availability
    let ag_available = tokio::process::Command::new("ag")
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false);

    // Select best tool based on preference and availability
    if rg_available {
        SearchTool::Ripgrep
    } else if ag_available {
        SearchTool::Ag
    } else {
        SearchTool::Native
    }
}

/// Check all tool availability for reporting
async fn check_all_tools() -> (bool, bool) {
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

    (rg_available, ag_available)
}

/// Perform search with the selected tool
async fn search_with_tool(
    tool: SearchTool,
    search_path: &str,
    query: &str,
    mode: SearchMode,
) -> Result<Vec<(String, u32, u32, String)>, Box<dyn std::error::Error>> {
    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    // Create the appropriate actor based on tool selection
    let actor: Box<dyn std::any::Any + Send> = match tool {
        SearchTool::Ripgrep => {
            let actor = RipgrepActor::new_ripgrep_actor(actor_rx, external_tx, search_path);
            Box::new(actor)
        }
        SearchTool::Ag => {
            let actor = AgActor::new_ag_actor(actor_rx, external_tx, search_path);
            Box::new(actor)
        }
        SearchTool::Native => {
            let actor =
                NativeSearchActor::new_native_search_actor(actor_rx, external_tx, search_path);
            Box::new(actor)
        }
    };

    // Send search query
    let search_query = SearchParams {
        query: query.to_string(),
        mode,
    };
    let search_message = Message::new(
        "updateSearchParams",
        FaeMessage::UpdateSearchParams(search_query),
    );

    actor_tx
        .send(search_message)
        .map_err(|e| format!("Failed to send search message: {}", e))?;

    // Collect results
    let mut results = Vec::new();
    let start_time = std::time::Instant::now();
    let max_wait = Duration::from_secs(5);

    while start_time.elapsed() < max_wait {
        match timeout(Duration::from_millis(100), external_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        results.push((result.filename, result.line, result.column, result.content));
                    }
                } else if message.method == "clearResults" {
                    results.clear();
                }
            }
            Ok(None) => break,
            Err(_) => {
                // Timeout - check if we have results or should wait more
                if !results.is_empty() {
                    break;
                }
            }
        }
    }

    // Clean up actor
    match tool {
        SearchTool::Ripgrep => {
            if let Ok(mut ripgrep_actor) = actor.downcast::<RipgrepActor>() {
                ripgrep_actor.shutdown();
            }
        }
        SearchTool::Ag => {
            if let Ok(mut ag_actor) = actor.downcast::<AgActor>() {
                ag_actor.shutdown();
            }
        }
        SearchTool::Native => {
            if let Ok(mut native_actor) = actor.downcast::<NativeSearchActor>() {
                native_actor.shutdown();
            }
        }
    }

    Ok(results)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🧠 Smart Search Demo");
    println!("===================");
    println!();

    // Check tool availability
    let (rg_available, ag_available) = check_all_tools().await;

    println!("🔧 Tool Status:");
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

    // Select best tool
    let selected_tool = select_best_search_tool().await;
    println!(
        "🎯 Selected Tool: {} ({})",
        selected_tool.name(),
        selected_tool.description()
    );
    println!();

    // Search parameters
    let search_path = "./src";
    let query = "async fn";
    let mode = SearchMode::Literal;

    println!(
        "🔍 Searching for '{}' in {} using {}",
        query,
        search_path,
        selected_tool.name()
    );

    // Perform search
    let start_time = std::time::Instant::now();
    let results = search_with_tool(selected_tool.clone(), search_path, query, mode).await?;
    let search_duration = start_time.elapsed();

    println!();
    println!(
        "📊 Search Results: {} matches found in {:?}",
        results.len(),
        search_duration
    );
    println!();

    // Display first 10 results
    for (i, (filename, line, offset, content)) in results.iter().take(10).enumerate() {
        println!(
            "  {}. {}:{}:{} | {}",
            i + 1,
            filename,
            line,
            offset,
            content.trim()
        );
    }

    if results.len() > 10 {
        println!("  ... and {} more matches", results.len() - 10);
    }

    println!();

    // Performance comparison if multiple tools are available
    if rg_available || ag_available {
        println!("⚡ Performance Comparison:");

        let tools_to_test = vec![
            (SearchTool::Native, true),
            (SearchTool::Ripgrep, rg_available),
            (SearchTool::Ag, ag_available),
        ];

        for (tool, available) in tools_to_test {
            if !available {
                continue;
            }

            let start = std::time::Instant::now();
            let tool_results = search_with_tool(tool.clone(), search_path, query, mode).await?;
            let duration = start.elapsed();

            println!(
                "  {}: {} matches in {:?}",
                tool.name(),
                tool_results.len(),
                duration
            );
        }
    }

    println!();
    println!("🎉 Smart search demo completed!");

    Ok(())
}
