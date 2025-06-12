use sfs::indexer::TreeSitterIndexer;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        std::process::exit(1);
    }

    let file_path = Path::new(&args[1]);
    println!("🔍 Profiling file: {}", file_path.display());
    println!("==========================================");

    // Basic file info
    if let Ok(metadata) = file_path.metadata() {
        println!(
            "📄 File size: {} bytes ({:.1} KB)",
            metadata.len(),
            metadata.len() as f64 / 1024.0
        );
    }

    if let Ok(content) = fs::read_to_string(file_path) {
        let lines = content.lines().count();
        let chars = content.chars().count();
        println!("📊 Content: {} lines, {} characters", lines, chars);

        // Check for problematic patterns
        let use_statements = content
            .lines()
            .filter(|line| line.trim().starts_with("use "))
            .count();
        let functions = content.matches("fn ").count();
        let structs = content.matches("struct ").count();
        let impl_blocks = content.matches("impl ").count();
        let comments = content
            .lines()
            .filter(|line| line.trim().starts_with("//"))
            .count();

        println!("🏗️  Rust structures:");
        println!("   - use statements: {}", use_statements);
        println!("   - functions: {}", functions);
        println!("   - structs: {}", structs);
        println!("   - impl blocks: {}", impl_blocks);
        println!("   - comment lines: {}", comments);

        // Look for very long lines
        let long_lines: Vec<_> = content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.len() > 200)
            .collect();

        if !long_lines.is_empty() {
            println!("⚠️  Very long lines (> 200 chars):");
            for (line_num, line) in long_lines.iter().take(5) {
                println!("   Line {}: {} chars", line_num + 1, line.len());
            }
            if long_lines.len() > 5 {
                println!("   ... and {} more long lines", long_lines.len() - 5);
            }
        }

        // Look for deeply nested structures
        let max_indent = content
            .lines()
            .map(|line| line.len() - line.trim_start().len())
            .max()
            .unwrap_or(0);
        println!("📏 Max indentation: {} spaces", max_indent);

        // Check for specific patterns that might be slow
        let match_statements = content.matches("match ").count();
        let if_statements = content.matches(" if ").count();
        let loops = content.matches("for ").count() + content.matches("while ").count();

        println!("🔄 Control flow:");
        println!("   - match statements: {}", match_statements);
        println!("   - if statements: {}", if_statements);
        println!("   - loops: {}", loops);
    }

    println!("\n🚀 Starting indexing performance test...");

    // Phase 1: Indexer initialization
    let start = Instant::now();
    let mut indexer = TreeSitterIndexer::new();
    indexer.initialize_sync()?;
    let init_time = start.elapsed();
    println!(
        "⏱️  Indexer initialization: {:.3}s",
        init_time.as_secs_f64()
    );

    // Phase 2: File content reading
    let start = Instant::now();
    let content = fs::read_to_string(file_path)?;
    let read_time = start.elapsed();
    println!("⏱️  File reading: {:.3}s", read_time.as_secs_f64());

    // Phase 3: Symbol extraction with detailed timing
    println!("\n🔍 Symbol extraction detailed timing:");

    let start = Instant::now();
    let symbols = indexer.extract_symbols_sync(file_path, true)?;
    let total_extraction_time = start.elapsed();

    println!(
        "⏱️  Total symbol extraction: {:.3}s",
        total_extraction_time.as_secs_f64()
    );
    println!("📊 Extracted {} symbols", symbols.len());

    if !symbols.is_empty() {
        println!(
            "   Symbols per second: {:.1}",
            symbols.len() as f64 / total_extraction_time.as_secs_f64()
        );
        println!(
            "   Time per symbol: {:.1}ms",
            total_extraction_time.as_millis() as f64 / symbols.len() as f64
        );
    }

    // Show symbol breakdown
    use std::collections::HashMap;
    let mut symbol_counts = HashMap::new();
    for symbol in &symbols {
        *symbol_counts
            .entry(format!("{:?}", symbol.symbol_type))
            .or_insert(0) += 1;
    }

    println!("\n📋 Symbol breakdown:");
    for (symbol_type, count) in symbol_counts {
        println!("   {}: {}", symbol_type, count);
    }

    // Performance analysis
    let total_time = init_time + read_time + total_extraction_time;
    println!("\n📈 Performance breakdown:");
    println!(
        "   Initialization: {:.3}s ({:.1}%)",
        init_time.as_secs_f64(),
        init_time.as_secs_f64() / total_time.as_secs_f64() * 100.0
    );
    println!(
        "   File reading: {:.3}s ({:.1}%)",
        read_time.as_secs_f64(),
        read_time.as_secs_f64() / total_time.as_secs_f64() * 100.0
    );
    println!(
        "   Symbol extraction: {:.3}s ({:.1}%)",
        total_extraction_time.as_secs_f64(),
        total_extraction_time.as_secs_f64() / total_time.as_secs_f64() * 100.0
    );
    println!("   Total: {:.3}s", total_time.as_secs_f64());

    // Performance rating specific to this file
    if total_extraction_time.as_millis() < 50 {
        println!("   Rating for this file: ⚡ Excellent (< 50ms)");
    } else if total_extraction_time.as_millis() < 200 {
        println!("   Rating for this file: ✅ Good (< 200ms)");
    } else if total_extraction_time.as_millis() < 1000 {
        println!("   Rating for this file: ⚠️  Acceptable (< 1s)");
    } else {
        println!("   Rating for this file: 🐌 Slow (> 1s)");
    }

    // Expected vs actual performance comparison
    let expected_time_ms = content.len() as f64 / 1000.0; // Rough heuristic: 1ms per 1KB
    let actual_time_ms = total_extraction_time.as_millis() as f64;
    let performance_ratio = actual_time_ms / expected_time_ms;

    println!("\n🎯 Performance analysis:");
    println!("   Expected time (heuristic): {:.1}ms", expected_time_ms);
    println!("   Actual time: {:.1}ms", actual_time_ms);
    println!(
        "   Performance ratio: {:.1}x {} expected",
        performance_ratio,
        if performance_ratio > 1.0 {
            "slower than"
        } else {
            "faster than"
        }
    );

    if performance_ratio > 10.0 {
        println!("   🚨 SEVERE PERFORMANCE ISSUE DETECTED!");
        println!(
            "   This file is taking {}x longer than expected.",
            performance_ratio as u32
        );
        println!(
            "   Investigate: complex regex patterns, deep nesting, or tree-sitter parsing issues."
        );
    } else if performance_ratio > 3.0 {
        println!(
            "   ⚠️  Performance issue detected ({}x slower than expected)",
            performance_ratio as u32
        );
    }

    Ok(())
}
