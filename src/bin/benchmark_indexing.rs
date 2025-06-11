use sfs::indexer::TreeSitterIndexer;
use std::env;
use std::path::Path;
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let directory = if args.len() > 1 { &args[1] } else { "." };

    let path = Path::new(directory);
    println!(
        "🚀 Benchmarking indexing performance for: {}",
        path.display()
    );
    println!("==========================================");

    // Phase 1: File discovery (similar to TUI quick discovery)
    println!("\n📁 Phase 1: File Discovery");
    let start = Instant::now();

    use ignore::WalkBuilder;
    let mut builder = WalkBuilder::new(path);
    builder
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .hidden(false)
        .parents(true)
        .ignore(true)
        .add_custom_ignore_filename(".ignore");

    let mut files = Vec::new();
    for dir_entry in builder.build().flatten() {
        let file_path = dir_entry.path();

        // Skip .git directory
        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/.git/") || path_str.ends_with("/.git") {
                continue;
            }
        }

        if file_path.is_file() && should_include_file(file_path) {
            files.push(file_path.to_path_buf());
        }
    }

    let discovery_time = start.elapsed();
    println!(
        "⏱️  Discovered {} files in {:.3}s",
        files.len(),
        discovery_time.as_secs_f64()
    );

    // Phase 2: Symbol extraction
    println!("\n🔍 Phase 2: Symbol Extraction");
    let start = Instant::now();

    let mut indexer = TreeSitterIndexer::new();
    indexer.initialize_sync()?;

    let mut total_symbols = 0;
    let mut processed_files = 0;
    let mut slow_files = Vec::new();

    for (i, file_path) in files.iter().enumerate() {
        let file_start = Instant::now();
        match indexer.extract_symbols_sync(file_path, false) {
            Ok(symbols) => {
                let file_time = file_start.elapsed();
                let symbol_count = symbols.len();
                total_symbols += symbol_count;
                processed_files += 1;

                // Track files that take more than 500ms
                if file_time.as_millis() > 500 {
                    slow_files.push((file_path.clone(), file_time, symbol_count));
                }
            }
            Err(_) => {
                // Continue with next file on error
            }
        }

        // Progress update every 10% of files
        if (i + 1) % (files.len() / 10).max(1) == 0 {
            let progress = ((i + 1) as f64 / files.len() as f64 * 100.0) as u32;
            let elapsed = start.elapsed();
            println!(
                "   Progress: {}% ({}/{} files, {:.1}s elapsed)",
                progress,
                i + 1,
                files.len(),
                elapsed.as_secs_f64()
            );
        }
    }

    let indexing_time = start.elapsed();
    println!(
        "⏱️  Extracted {} symbols from {} files in {:.3}s",
        total_symbols,
        processed_files,
        indexing_time.as_secs_f64()
    );

    // Phase 3: Total time
    let total_time = discovery_time + indexing_time;
    println!("\n📊 Performance Summary:");
    println!(
        "   File discovery: {:.3}s ({:.1}%)",
        discovery_time.as_secs_f64(),
        discovery_time.as_secs_f64() / total_time.as_secs_f64() * 100.0
    );
    println!(
        "   Symbol extraction: {:.3}s ({:.1}%)",
        indexing_time.as_secs_f64(),
        indexing_time.as_secs_f64() / total_time.as_secs_f64() * 100.0
    );
    println!("   Total time: {:.3}s", total_time.as_secs_f64());
    println!(
        "   Symbols per second: {:.1}",
        total_symbols as f64 / indexing_time.as_secs_f64()
    );
    println!(
        "   Files per second: {:.1}",
        processed_files as f64 / indexing_time.as_secs_f64()
    );

    // Performance categories
    if total_time.as_secs_f64() < 1.0 {
        println!("   Performance rating: ⚡ Excellent (< 1s)");
    } else if total_time.as_secs_f64() < 5.0 {
        println!("   Performance rating: ✅ Good (< 5s)");
    } else if total_time.as_secs_f64() < 15.0 {
        println!("   Performance rating: ⚠️  Acceptable (< 15s)");
    } else {
        println!("   Performance rating: 🐌 Slow (> 15s)");
    }

    // Show slow files analysis
    if !slow_files.is_empty() {
        println!("\n🐌 Slow Files Analysis (> 500ms):");
        slow_files.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by time desc
        for (i, (path, duration, symbols)) in slow_files.iter().take(10).enumerate() {
            println!(
                "   {}. {} - {:.1}s ({} symbols, {:.1} symbols/s)",
                i + 1,
                path.file_name().unwrap_or_default().to_string_lossy(),
                duration.as_secs_f64(),
                symbols,
                *symbols as f64 / duration.as_secs_f64()
            );
        }
        if slow_files.len() > 10 {
            println!("   ... and {} more slow files", slow_files.len() - 10);
        }

        let slow_total_time: f64 = slow_files.iter().map(|(_, d, _)| d.as_secs_f64()).sum();
        let slow_percentage = slow_total_time / indexing_time.as_secs_f64() * 100.0;
        println!(
            "   Slow files account for {:.1}% of total indexing time",
            slow_percentage
        );
    }

    Ok(())
}

fn should_include_file(path: &Path) -> bool {
    // Basic file size check
    const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
    if let Ok(metadata) = path.metadata() {
        if metadata.len() > MAX_FILE_SIZE {
            return false;
        }
    }

    // Skip binary files
    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        let binary_extensions = [
            "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp", "zip", "tar", "gz", "bz2",
            "7z", "rar", "exe", "bin", "so", "dylib", "dll", "app", "mp3", "mp4", "avi", "mov",
            "wmv", "flv", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "db", "sqlite",
            "sqlite3", "ttf", "otf", "woff", "woff2", "o", "obj", "pyc", "class", "jar", "lock",
        ];

        if binary_extensions.contains(&extension.to_lowercase().as_str()) {
            return false;
        }
    }

    true
}
