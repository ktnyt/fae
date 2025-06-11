use sfs::indexer::TreeSitterIndexer;
use std::env;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: memory_analyzer <directory>");
        std::process::exit(1);
    }

    let directory = Path::new(&args[1]);
    println!("🔍 Memory Analysis for: {}", directory.display());

    // メモリ使用量を取得する関数
    fn get_memory_usage() -> Option<usize> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            let output = Command::new("ps")
                .args(["-o", "rss=", "-p", &std::process::id().to_string()])
                .output()
                .ok()?;
            let rss_str = String::from_utf8(output.stdout).ok()?;
            let rss_kb: usize = rss_str.trim().parse().ok()?;
            Some(rss_kb * 1024) // Convert KB to bytes
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    // 初期メモリ使用量
    let initial_memory = get_memory_usage();
    println!(
        "📊 Initial memory: {} MB",
        initial_memory.map_or("N/A".to_string(), |m| format!(
            "{:.2}",
            m as f64 / 1024.0 / 1024.0
        ))
    );

    // インデクサー作成
    let mut indexer = TreeSitterIndexer::with_options(true, true);
    indexer.initialize_sync()?;

    // キャッシュロード前
    let pre_cache_memory = get_memory_usage();

    // キャッシュロード
    match indexer.load_cache(directory) {
        Ok(stats) => {
            println!(
                "📦 Cache loaded: {} files, {} symbols",
                stats.total_files, stats.total_symbols
            );

            // キャッシュロード後のメモリ
            let post_cache_memory = get_memory_usage();

            if let (Some(pre), Some(post)) = (pre_cache_memory, post_cache_memory) {
                let cache_memory = post - pre;
                println!(
                    "📈 Cache memory usage: {:.2} MB",
                    cache_memory as f64 / 1024.0 / 1024.0
                );

                // 1シンボルあたりのメモリ使用量
                if stats.total_symbols > 0 {
                    let bytes_per_symbol = cache_memory / stats.total_symbols;
                    println!("📏 Memory per symbol: {} bytes", bytes_per_symbol);
                }
            }

            // 全シンボル取得
            let all_symbols = indexer.get_all_symbols();
            let final_memory = get_memory_usage();

            if let (Some(post), Some(final_mem)) = (post_cache_memory, final_memory) {
                let symbols_memory = final_mem - post;
                println!(
                    "📈 Symbols vector memory: {:.2} MB",
                    symbols_memory as f64 / 1024.0 / 1024.0
                );
            }

            println!("🔢 Symbols in memory: {}", all_symbols.len());

            // サンプルシンボルのサイズ分析
            if let Some(symbol) = all_symbols.first() {
                let symbol_size = std::mem::size_of_val(symbol)
                    + symbol.name.capacity()
                    + symbol.file.as_os_str().len()
                    + symbol.context.as_ref().map_or(0, |c| c.capacity());
                println!("📏 Sample symbol size: {} bytes", symbol_size);
            }
        }
        Err(e) => {
            println!("❌ Failed to load cache: {}", e);
        }
    }

    Ok(())
}
