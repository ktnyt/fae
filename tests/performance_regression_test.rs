// パフォーマンス回帰検出テスト
// インデックシング性能、メモリ使用量、レスポンス時間の監視

use sfs::indexer::TreeSitterIndexer;
use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;
// use std::sync::atomic::{AtomicUsize, Ordering};
// use std::sync::Arc;

#[cfg(test)]
mod performance_regression_tests {
    use super::*;

    /// Create a large project structure to test performance characteristics
    fn create_large_test_project(dir: &TempDir, num_files: usize) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // Create multiple directories
        for i in 0..10 {
            fs::create_dir_all(dir_path.join(format!("module_{}", i)))?;
        }

        // Generate files with realistic content
        for i in 0..num_files {
            let module_dir = dir_path.join(format!("module_{}", i % 10));
            let file_path = module_dir.join(format!("file_{}.ts", i));

            // Generate realistic TypeScript content
            let content = format!(
                "// File {} - Auto-generated for performance testing\n\
                 export interface Data{} {{\n\
                     id: number;\n\
                     value: string;\n\
                     timestamp: Date;\n\
                 }}\n\n\
                 export class Processor{} {{\n\
                     private data: Data{}[] = [];\n\n\
                     constructor(private config: string) {{}}\n\n\
                     async process(item: Data{}): Promise<void> {{\n\
                         this.data.push(item);\n\
                         await this.validate(item);\n\
                     }}\n\n\
                     private async validate(item: Data{}): Promise<boolean> {{\n\
                         return item.id > 0 && item.value.length > 0;\n\
                     }}\n\n\
                     get count(): number {{\n\
                         return this.data.length;\n\
                     }}\n\
                 }}\n\n\
                 export function helper{}(input: string): string {{\n\
                     return input.toUpperCase();\n\
                 }}\n\n\
                 export const constant{} = 'VALUE_{}';\n\
                 const internal{} = () => Math.random();\n",
                i, i, i, i, i, i, i, i, i, i
            );

            fs::write(file_path, content)?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn should_maintain_indexing_performance_standards() {
        let temp_dir = TempDir::new().unwrap();
        let num_files = 50; // Reasonable test size
        create_large_test_project(&temp_dir, num_files).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Measure indexing performance
        let start_time = Instant::now();
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // Performance assertions
        assert!(
            indexing_duration < Duration::from_secs(10),
            "Indexing {} files should complete within 10 seconds, took {:?}",
            num_files,
            indexing_duration
        );

        // Should extract a reasonable number of symbols per file
        let symbols_per_file = all_symbols.len() as f64 / num_files as f64;
        assert!(
            symbols_per_file > 5.0,
            "Should extract at least 5 symbols per file on average, got {:.2}",
            symbols_per_file
        );

        // Calculate performance metrics
        let files_per_second = num_files as f64 / indexing_duration.as_secs_f64();
        let symbols_per_second = all_symbols.len() as f64 / indexing_duration.as_secs_f64();

        println!("Performance metrics:");
        println!("  Files processed: {}", num_files);
        println!("  Total symbols: {}", all_symbols.len());
        println!("  Indexing time: {:?}", indexing_duration);
        println!("  Files/second: {:.2}", files_per_second);
        println!("  Symbols/second: {:.2}", symbols_per_second);

        // Baseline performance expectations (can be adjusted based on hardware)
        assert!(
            files_per_second > 5.0,
            "Should process at least 5 files per second, got {:.2}",
            files_per_second
        );
        assert!(
            symbols_per_second > 50.0,
            "Should extract at least 50 symbols per second, got {:.2}",
            symbols_per_second
        );
    }

    #[tokio::test]
    async fn should_maintain_search_performance_standards() {
        let temp_dir = TempDir::new().unwrap();
        create_large_test_project(&temp_dir, 30).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let symbols = indexer.get_all_symbols();
        let searcher = FuzzySearcher::new(symbols);

        // Test search performance with various query types
        let test_queries = vec![
            "process",   // Common term
            "Processor", // Class name
            "helper",    // Function name
            "Data",      // Interface name
            "const",     // Partial match
            "xyz",       // No matches
        ];

        let mut total_search_time = Duration::new(0, 0);
        let mut total_results = 0;

        for query in &test_queries {
            let start_time = Instant::now();
            let results = searcher.search(query, &SearchOptions::default());
            let search_duration = start_time.elapsed();

            total_search_time += search_duration;
            total_results += results.len();

            // Individual search should be very fast
            assert!(
                search_duration < Duration::from_millis(100),
                "Search for '{}' should complete within 100ms, took {:?}",
                query,
                search_duration
            );

            println!(
                "Search '{}': {} results in {:?}",
                query,
                results.len(),
                search_duration
            );
        }

        let avg_search_time = total_search_time / test_queries.len() as u32;
        println!("Average search time: {:?}", avg_search_time);
        println!("Total results across all queries: {}", total_results);

        // Average search time should be very responsive
        assert!(
            avg_search_time < Duration::from_millis(50),
            "Average search time should be under 50ms, got {:?}",
            avg_search_time
        );
    }

    #[test]
    fn should_handle_memory_usage_efficiently() {
        let temp_dir = TempDir::new().unwrap();

        // Create a file with many symbols to test memory efficiency
        let large_content = (0..1000)
            .map(|i| {
                format!(
                    "function func{}() {{ return {}; }}\nconst var{} = {};",
                    i, i, i, i
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(temp_dir.path().join("large.ts"), large_content).unwrap();

        // Use a memory tracking approach
        let memory_before = get_memory_usage();

        let symbols = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut indexer = TreeSitterIndexer::with_verbose(false);
            indexer.initialize().await.unwrap();
            let patterns = vec!["**/*.ts".to_string()];
            indexer
                .index_directory(temp_dir.path(), &patterns)
                .await
                .unwrap();
            indexer.get_all_symbols()
        });

        let memory_after = get_memory_usage();
        let memory_used = memory_after.saturating_sub(memory_before);

        println!(
            "Memory usage: {} bytes for {} symbols",
            memory_used,
            symbols.len()
        );

        // Should extract many symbols (1000 functions + 1000 variables + filename + dirname)
        assert!(
            symbols.len() > 1000,
            "Should extract over 1000 symbols from large file"
        );

        // Memory usage should be reasonable (less than 10MB for this test)
        // Note: This is a rough heuristic and may vary by platform
        assert!(
            memory_used < 10_000_000,
            "Memory usage should be reasonable: {} bytes",
            memory_used
        );
    }

    #[tokio::test]
    async fn should_maintain_consistent_performance_across_runs() {
        let temp_dir = TempDir::new().unwrap();
        create_large_test_project(&temp_dir, 20).unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let mut durations = Vec::new();
        let mut symbol_counts = Vec::new();

        // Run warmup first, then multiple times to check consistency
        {
            // Warmup run (not included in measurements)
            let mut warmup_indexer = TreeSitterIndexer::with_verbose(false);
            warmup_indexer.initialize().await.unwrap();
            warmup_indexer
                .index_directory(temp_dir.path(), &patterns)
                .await
                .unwrap();
        }

        // Actual measurement runs
        for run in 0..5 {
            let mut indexer = TreeSitterIndexer::with_verbose(false);
            indexer.initialize().await.unwrap();

            let start_time = Instant::now();
            indexer
                .index_directory(temp_dir.path(), &patterns)
                .await
                .unwrap();
            let duration = start_time.elapsed();

            let symbols = indexer.get_all_symbols();

            durations.push(duration);
            symbol_counts.push(symbols.len());

            println!("Run {}: {:?}, {} symbols", run + 1, duration, symbols.len());
        }

        // Check consistency in symbol counts
        assert!(
            symbol_counts.iter().all(|&count| count == symbol_counts[0]),
            "Symbol counts should be consistent across runs: {:?}",
            symbol_counts
        );

        // Check that performance doesn't degrade significantly
        let min_duration = durations.iter().min().unwrap();
        let max_duration = durations.iter().max().unwrap();

        // Max duration shouldn't be more than 5x min duration (allowing for initialization overhead)
        assert!(
            max_duration.as_millis() <= min_duration.as_millis() * 5,
            "Performance variance too high: min {:?}, max {:?}",
            min_duration,
            max_duration
        );

        let avg_duration = Duration::from_millis(
            (durations.iter().map(|d| d.as_millis()).sum::<u128>() / durations.len() as u128)
                as u64,
        );

        println!(
            "Performance consistency: avg {:?}, range {:?} - {:?}",
            avg_duration, min_duration, max_duration
        );
    }

    #[test]
    fn should_handle_regex_pattern_compilation_efficiently() {
        // Test that regex patterns are pre-compiled (not recompiled each time)
        let temp_dir = TempDir::new().unwrap();

        // Create a file to test regex performance
        fs::write(
            temp_dir.path().join("test.ts"),
            "function test() {}\nclass Test {}\nconst value = 42;",
        )
        .unwrap();

        let indexer = TreeSitterIndexer::with_verbose(false);

        // Measure time for multiple symbol extractions
        let iterations = 100;
        let start_time = Instant::now();

        for _ in 0..iterations {
            let _ = indexer.extract_symbols_sync(&temp_dir.path().join("test.ts"), false);
        }

        let total_duration = start_time.elapsed();
        let avg_per_extraction = total_duration / iterations;

        println!(
            "Regex performance: {} iterations in {:?} (avg {:?} per extraction)",
            iterations, total_duration, avg_per_extraction
        );

        // With pre-compiled patterns, each extraction should be reasonably fast
        assert!(
            avg_per_extraction < Duration::from_millis(50),
            "Average extraction time should be under 50ms with pre-compiled patterns, got {:?}",
            avg_per_extraction
        );

        // Total time for 100 iterations should be reasonable
        assert!(
            total_duration < Duration::from_secs(5),
            "100 extractions should complete within 5 seconds, took {:?}",
            total_duration
        );
    }

    /// Simple memory usage estimation (platform-specific, rough approximation)
    fn get_memory_usage() -> usize {
        // This is a simplified approach for testing
        // In a real scenario, you might use platform-specific APIs
        #[cfg(target_os = "linux")]
        {
            use std::fs;
            if let Ok(status) = fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return kb * 1024; // Convert KB to bytes
                            }
                        }
                    }
                }
            }
        }

        // Fallback: return 0 (memory tracking not available)
        0
    }
}
