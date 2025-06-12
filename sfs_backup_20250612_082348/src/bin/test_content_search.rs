use sfs::indexer::TreeSitterIndexer;
use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("🔍 Testing Content Search Performance...\n");

    // Index current directory
    let mut indexer = TreeSitterIndexer::with_verbose(true);
    indexer.initialize().await?;

    let patterns = vec!["**/*".to_string()];
    let start_indexing = Instant::now();
    indexer
        .index_directory(std::path::Path::new("."), &patterns)
        .await?;
    let indexing_duration = start_indexing.elapsed();

    let symbols = indexer.get_all_symbols();
    println!(
        "📚 Indexed {} symbols in {:?}\n",
        symbols.len(),
        indexing_duration
    );

    let searcher = FuzzySearcher::new(symbols);

    // Test queries
    let test_queries = vec!["use", "function", "test", "println", "struct", "impl"];

    for query in test_queries {
        println!("🔍 Testing content search for: '{}'", query);

        let start_search = Instant::now();
        let results = searcher.search_content(query, &SearchOptions::default());
        let search_duration = start_search.elapsed();

        println!(
            "   ⚡ Found {} results in {:?}",
            results.len(),
            search_duration
        );

        // Show first few results
        for (i, result) in results.iter().take(3).enumerate() {
            println!(
                "   {}. {} ({}:{})",
                i + 1,
                result.symbol.name.chars().take(60).collect::<String>(),
                result.symbol.file.display(),
                result.symbol.line
            );
        }
        println!();
    }

    // Performance comparison test
    println!("🚀 Performance comparison test:");
    let perf_query = "println";

    // Test multiple times for consistency
    let mut durations = Vec::new();
    for i in 0..5 {
        let start = Instant::now();
        let results = searcher.search_content(perf_query, &SearchOptions::default());
        let duration = start.elapsed();
        durations.push(duration);
        println!(
            "   Run {}: {} results in {:?}",
            i + 1,
            results.len(),
            duration
        );
    }

    let avg_duration = durations.iter().sum::<std::time::Duration>() / durations.len() as u32;
    println!("   📊 Average: {:?}", avg_duration);

    Ok(())
}
