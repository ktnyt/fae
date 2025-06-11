use sfs::cache_manager::MemoryEfficientCacheManager;
use sfs::indexer::TreeSitterIndexer;
use std::env;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: memory_efficient_test <directory>");
        std::process::exit(1);
    }

    let directory = Path::new(&args[1]);
    println!(
        "🔍 Testing Memory-Efficient Cache for: {}",
        directory.display()
    );

    // Step 1: 従来のキャッシュでシンボル数を確認
    let mut old_indexer = TreeSitterIndexer::with_options(true, true);
    old_indexer.initialize_sync()?;

    match old_indexer.load_cache(directory) {
        Ok(stats) => {
            println!(
                "📊 Existing cache: {} files, {} symbols",
                stats.total_files, stats.total_symbols
            );

            // Step 2: メモリ効率的キャッシュマネージャーをテスト
            let cache_dir = directory.to_path_buf();
            let mut cache_manager = MemoryEfficientCacheManager::new(cache_dir, 50); // 50MB制限

            println!("🔄 Converting to memory-efficient format...");

            // 全シンボルを取得
            let all_symbols = old_indexer.get_all_symbols();
            println!("📦 Total symbols in memory: {}", all_symbols.len());

            // メモリ効率化テスト用: 少数のファイルでテスト
            let test_files = vec!["test_file_1.rs", "test_file_2.py", "test_file_3.ts"];

            // テストシンボルを生成
            for (i, file_name) in test_files.iter().enumerate() {
                let start_idx = i * 1000;
                let end_idx = ((i + 1) * 1000).min(all_symbols.len());

                if start_idx < all_symbols.len() {
                    let file_symbols = all_symbols[start_idx..end_idx].to_vec();
                    let hash = format!("test_hash_{}", i);

                    println!(
                        "💾 Updating cache for {}: {} symbols",
                        file_name,
                        file_symbols.len()
                    );
                    cache_manager.update_file_cache(file_name, hash, file_symbols)?;
                }
            }

            // インデックス保存
            cache_manager.save_index(directory)?;
            println!("✅ Memory-efficient cache saved");

            // Step 3: メモリ使用量比較
            println!("\n📈 Memory Usage Comparison:");
            println!(
                "  Traditional cache: ~{:.2} MB (estimated)",
                all_symbols.len() as f64 * 568.0 / 1024.0 / 1024.0
            );
            println!(
                "  Memory-efficient cache: {:.2} MB",
                cache_manager.memory_usage_mb()
            );

            // Step 4: ランダムアクセステスト
            println!("\n🎯 Random Access Test:");
            for file_name in &test_files {
                match cache_manager.get_file_symbols(file_name) {
                    Ok(symbols) => {
                        println!("  {} → {} symbols loaded", file_name, symbols.len());
                    }
                    Err(e) => {
                        println!("  {} → Error: {}", file_name, e);
                    }
                }
            }

            println!(
                "  Current memory usage: {:.2} MB",
                cache_manager.memory_usage_mb()
            );
        }
        Err(e) => {
            println!("❌ No existing cache found: {}", e);
        }
    }

    Ok(())
}
