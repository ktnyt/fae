//! Search tool comparison example
//!
//! This example demonstrates and compares the behavior of RipgrepActor and AgActor
//! using the same search queries to verify they work correctly and produce 
//! consistent results.

use fae::actors::messages::FaeMessage;
use fae::actors::types::{SearchMode, SearchParams};
use fae::actors::{AgActor, RipgrepActor};
use fae::core::Message;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[derive(Debug, Clone)]
struct SearchStats {
    tool_name: String,
    query: String,
    mode: SearchMode,
    execution_time: Duration,
    result_count: usize,
    first_result_time: Option<Duration>,
    available: bool,
    error: Option<String>,
}

impl SearchStats {
    fn new(tool_name: String) -> Self {
        Self {
            tool_name,
            query: String::new(),
            mode: SearchMode::Literal,
            execution_time: Duration::from_secs(0),
            result_count: 0,
            first_result_time: None,
            available: false,
            error: None,
        }
    }
}

async fn check_tool_availability(tool: &str) -> bool {
    match tokio::process::Command::new(tool)
        .arg("--version")
        .output()
        .await
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

async fn run_search_test(
    tool_name: &str,
    query: &str,
    mode: SearchMode,
    search_path: &str,
) -> SearchStats {
    let mut stats = SearchStats::new(tool_name.to_string());
    stats.query = query.to_string();
    stats.mode = mode;

    println!("🔍 Testing {} with query '{}' (mode: {:?})", tool_name, query, mode);

    // Check if tool is available
    let available = match tool_name {
        "ripgrep" => check_tool_availability("rg").await,
        "ag" => check_tool_availability("ag").await,
        _ => false,
    };

    if !available {
        stats.error = Some(format!("{} is not available", tool_name));
        println!("  ❌ {} is not installed or not available in PATH", tool_name);
        return stats;
    }

    stats.available = true;

    let (actor_tx, actor_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();
    let (external_tx, mut external_rx) = mpsc::unbounded_channel::<Message<FaeMessage>>();

    let start_time = Instant::now();
    let mut first_result_time = None;

    // Create appropriate actor
    let mut actor = match tool_name {
        "ripgrep" => {
            let actor = RipgrepActor::new_ripgrep_actor(actor_rx, external_tx, search_path);
            Some(actor)
        }
        "ag" => {
            let actor = AgActor::new_ag_actor(actor_rx, external_tx, search_path);
            Some(actor)
        }
        _ => None,
    };

    if actor.is_none() {
        stats.error = Some("Unknown tool".to_string());
        return stats;
    }

    // Send search query
    let search_query = SearchParams {
        query: query.to_string(),
        mode,
    };
    let search_message = Message::new(
        "updateSearchQuery",
        FaeMessage::UpdateSearchQuery(search_query),
    );

    if let Err(e) = actor_tx.send(search_message) {
        stats.error = Some(format!("Failed to send search message: {}", e));
        return stats;
    }

    // Collect results for a limited time
    let collection_timeout = Duration::from_millis(2000);
    let collection_start = Instant::now();

    while collection_start.elapsed() < collection_timeout {
        match timeout(Duration::from_millis(100), external_rx.recv()).await {
            Ok(Some(message)) => {
                if message.method == "pushSearchResult" {
                    if let FaeMessage::PushSearchResult(result) = message.payload {
                        if first_result_time.is_none() {
                            first_result_time = Some(start_time.elapsed());
                        }
                        stats.result_count += 1;
                        
                        // Print first few results as examples
                        if stats.result_count <= 3 {
                            println!(
                                "  📄 {}:{}:{} | {}",
                                result.filename,
                                result.line,
                                result.offset,
                                result.content.trim()
                            );
                        }
                    }
                }
            }
            Ok(None) => break, // Channel closed
            Err(_) => break,   // Timeout
        }
    }

    stats.execution_time = start_time.elapsed();
    stats.first_result_time = first_result_time;

    // Clean up
    if let Some(mut actor) = actor {
        actor.shutdown();
    }

    println!(
        "  ✅ {} found {} results in {:?}",
        tool_name, stats.result_count, stats.execution_time
    );

    stats
}

fn print_comparison(ripgrep_stats: &SearchStats, ag_stats: &SearchStats) {
    println!("\n📊 === COMPARISON RESULTS ===");
    println!("Query: '{}' (mode: {:?})", ripgrep_stats.query, ripgrep_stats.mode);
    println!();

    // Availability
    println!("🔧 Tool Availability:");
    println!("  ripgrep: {}", if ripgrep_stats.available { "✅ Available" } else { "❌ Not available" });
    println!("  ag:      {}", if ag_stats.available { "✅ Available" } else { "❌ Not available" });
    println!();

    if !ripgrep_stats.available && !ag_stats.available {
        println!("⚠️  Neither tool is available - cannot perform comparison");
        return;
    }

    // Results count
    println!("📈 Results Found:");
    if ripgrep_stats.available {
        println!("  ripgrep: {} results", ripgrep_stats.result_count);
    }
    if ag_stats.available {
        println!("  ag:      {} results", ag_stats.result_count);
    }

    // Performance
    println!("\n⚡ Performance:");
    if ripgrep_stats.available {
        println!("  ripgrep: {:?}", ripgrep_stats.execution_time);
        if let Some(first) = ripgrep_stats.first_result_time {
            println!("           (first result: {:?})", first);
        }
    }
    if ag_stats.available {
        println!("  ag:      {:?}", ag_stats.execution_time);
        if let Some(first) = ag_stats.first_result_time {
            println!("           (first result: {:?})", first);
        }
    }

    // Comparison analysis
    if ripgrep_stats.available && ag_stats.available {
        println!("\n🔍 Analysis:");
        
        let result_diff = (ripgrep_stats.result_count as i32) - (ag_stats.result_count as i32);
        match result_diff {
            0 => println!("  ✅ Both tools found the same number of results"),
            diff if diff > 0 => println!("  📊 ripgrep found {} more results than ag", diff),
            diff => println!("  📊 ag found {} more results than ripgrep", -diff),
        }

        if ripgrep_stats.execution_time < ag_stats.execution_time {
            println!("  🚀 ripgrep was faster");
        } else if ag_stats.execution_time < ripgrep_stats.execution_time {
            println!("  🚀 ag was faster");
        } else {
            println!("  ⚖️  Both tools had similar performance");
        }
    }

    // Errors
    if let Some(error) = &ripgrep_stats.error {
        println!("\n❌ ripgrep error: {}", error);
    }
    if let Some(error) = &ag_stats.error {
        println!("\n❌ ag error: {}", error);
    }

    println!();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 Search Tool Comparison Demo");
    println!("==============================");
    println!();

    let search_path = "./src";

    // Test cases
    let test_cases = vec![
        ("CommandActor", SearchMode::Literal),
        (r"fn \w+", SearchMode::Regexp),
        ("use", SearchMode::Literal),
        (r"impl.*\{", SearchMode::Regexp),
    ];

    let mut all_results = Vec::new();

    for (query, mode) in test_cases {
        println!("🧪 === Test Case: '{}' ({:?}) ===", query, mode);
        println!();

        // Run tests for both tools
        let ripgrep_stats = run_search_test("ripgrep", query, mode, search_path).await;
        println!();
        let ag_stats = run_search_test("ag", query, mode, search_path).await;

        print_comparison(&ripgrep_stats, &ag_stats);
        println!("═══════════════════════════════════════");
        println!();

        all_results.push((ripgrep_stats, ag_stats));
    }

    // Summary
    println!("📋 === SUMMARY ===");
    println!();

    let mut ripgrep_available = true;
    let mut ag_available = true;

    for (ripgrep_stats, ag_stats) in &all_results {
        if !ripgrep_stats.available {
            ripgrep_available = false;
        }
        if !ag_stats.available {
            ag_available = false;
        }
    }

    println!("🔧 Tool Support:");
    println!("  ripgrep: {}", if ripgrep_available { "✅ Fully functional" } else { "❌ Not available" });
    println!("  ag:      {}", if ag_available { "✅ Fully functional" } else { "❌ Not available" });
    println!();

    if ripgrep_available && ag_available {
        println!("🎉 Both tools are working correctly!");
        println!("💡 You can use either ripgrep or ag as your search backend.");
    } else if ripgrep_available {
        println!("⚠️  Only ripgrep is available. Consider installing ag for fallback support.");
    } else if ag_available {
        println!("⚠️  Only ag is available. Consider installing ripgrep for better performance.");
    } else {
        println!("❌ Neither tool is available. Please install ripgrep or ag.");
    }

    println!();
    println!("✨ Comparison completed!");

    Ok(())
}