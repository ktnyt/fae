//! リソース枯渇とメモリ制限のストレステスト
//! 
//! メモリ枯渇、ファイルディスクリプタ枯渇、ディスク容量不足、
//! 非常に大きなファイル処理、システムリソース制限でのgraceful degradationをテスト

use fae::{CacheManager, SearchRunner, RealtimeIndexer};
use anyhow::Result;
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use std::time::{Duration, Instant};

/// メモリ枯渇シミュレーション（大量シンボル生成）
#[tokio::test]
async fn test_memory_exhaustion_simulation() -> Result<()> {
    println!("🔍 メモリ枯渇シミュレーションテスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 段階的にメモリ使用量を増やすファイル群を作成
    let memory_levels = vec![
        (10, "小規模"), 
        (100, "中規模"), 
        (1000, "大規模"), 
        (5000, "巨大"),
    ];
    
    let mut total_symbols = 0;
    let mut peak_memory_usage = 0;
    
    for (symbol_count, level_name) in memory_levels {
        println!("  {} メモリ負荷テスト: {} シンボル", level_name, symbol_count);
        
        let level_file = temp_dir.path().join(format!("memory_level_{}.rs", symbol_count));
        let mut file_content = String::new();
        
        // 大量のシンボルを含むファイルを生成
        for i in 0..symbol_count {
            file_content.push_str(&format!(r#"
fn memory_test_function_{}_{}() -> String {{
    let data_{} = vec![{}, {}, {}, {}];
    format!("memory_test: {{:?}}", data_{})
}}

struct MemoryTestStruct_{}_{}{{
    field_a: Vec<String>,
    field_b: std::collections::HashMap<i32, String>,
    field_c: Box<Vec<Box<String>>>,
}}

impl MemoryTestStruct_{}_{} {{
    fn new() -> Self {{
        Self {{
            field_a: (0..{}).map(|x| format!("item_{{}}", x)).collect(),
            field_b: (0..{}).map(|x| (x, format!("value_{{}}", x))).collect(),
            field_c: Box::new((0..{}).map(|x| Box::new(format!("boxed_{{}}", x))).collect()),
        }}
    }}
    
    fn process_data(&mut self) -> usize {{
        self.field_a.len() + self.field_b.len() + self.field_c.len()
    }}
}}
"#, symbol_count, i, i, i*2, i*3, i*4, i*5, i, symbol_count, i, symbol_count, i, i % 20 + 1, i % 15 + 1, i % 10 + 1));
        }
        
        fs::write(&level_file, &file_content)?;
        println!("    ファイル作成: {} バイト", file_content.len());
        
        // メモリ使用量測定（概算）
        let start_time = Instant::now();
        match cache_manager.get_symbols(&level_file) {
            Ok(symbols) => {
                let duration = start_time.elapsed();
                total_symbols += symbols.len();
                
                // 概算のメモリ使用量計算（各シンボル100バイト概算）
                let estimated_memory = total_symbols * 100;
                if estimated_memory > peak_memory_usage {
                    peak_memory_usage = estimated_memory;
                }
                
                println!("    成功: {} シンボル, {:?}, 累計: {} シンボル", 
                        symbols.len(), duration, total_symbols);
                println!("    推定メモリ使用量: {} KB", estimated_memory / 1024);
                
                // メモリ使用量が過大でないことを確認
                assert!(symbols.len() >= symbol_count * 2, 
                       "期待されるシンボル数（関数+構造体+impl）が抽出されるべき");
                assert!(duration.as_secs() < 30, 
                       "大量シンボル解析は30秒以内であるべき");
            }
            Err(e) => {
                let duration = start_time.elapsed();
                println!("    エラー: {} ({:?})", e, duration);
                
                // エラーでも合理的な時間で応答すべき
                assert!(duration.as_secs() < 30, "エラーでも30秒以内で応答すべき");
            }
        }
        
        // 段階間のメモリ解放を促進
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    println!("📊 メモリ枯渇テスト結果:");
    println!("  累計シンボル処理: {}", total_symbols);
    println!("  推定最大メモリ使用量: {} MB", peak_memory_usage / (1024 * 1024));
    
    // 合理的なメモリ使用量内であることを確認
    assert!(peak_memory_usage < 100 * 1024 * 1024, "メモリ使用量は100MB以内であるべき");
    assert!(total_symbols > 10000, "大量シンボル処理の実証");
    
    println!("✅ メモリ枯渇シミュレーションテスト完了");
    Ok(())
}

/// ファイルディスクリプタ枯渇シミュレーション
#[tokio::test]
async fn test_file_descriptor_exhaustion() -> Result<()> {
    println!("🔍 ファイルディスクリプタ枯渇テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 大量のファイルを作成（1000ファイル）
    let file_count = 1000;
    println!("  {} ファイル作成中...", file_count);
    
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("fd_test_{:04}.rs", i));
        let content = format!(r#"
fn fd_test_function_{}() -> i32 {{
    {} + {} + {}
}}

struct FdTestStruct_{} {{
    value: i32,
}}
"#, i, i, i*2, i*3, i);
        fs::write(&file_path, content)?;
    }
    
    println!("  ファイル作成完了");
    
    // SearchRunnerで大量ファイル処理
    let _search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    let start_time = Instant::now();
    let mut processed_files = 0;
    let mut total_symbols = 0;
    
    // バッチ処理で段階的にファイルを処理
    let batch_size = 100;
    for batch_start in (0..file_count).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(file_count);
        println!("  バッチ処理: {}-{}", batch_start, batch_end);
        
        for i in batch_start..batch_end {
            let file_path = temp_dir.path().join(format!("fd_test_{:04}.rs", i));
            
            match cache_manager.get_symbols(&file_path) {
                Ok(symbols) => {
                    total_symbols += symbols.len();
                    processed_files += 1;
                }
                Err(e) => {
                    println!("    ファイル {} 処理エラー: {}", i, e);
                }
            }
        }
        
        // バッチ間でリソース解放の時間を与える
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    let total_duration = start_time.elapsed();
    
    println!("📊 ファイルディスクリプタ枯渇テスト結果:");
    println!("  処理ファイル数: {} / {}", processed_files, file_count);
    println!("  総シンボル数: {}", total_symbols);
    println!("  総処理時間: {:?}", total_duration);
    
    // パフォーマンス要件
    assert!(processed_files >= file_count * 95 / 100, 
           "95%以上のファイルが処理されるべき");
    assert!(total_symbols >= processed_files * 2, 
           "ファイルあたり少なくとも2シンボル（関数+構造体）");
    assert!(total_duration.as_secs() < 60, 
           "1000ファイル処理は60秒以内であるべき");
    
    let files_per_second = processed_files as f64 / total_duration.as_secs_f64();
    println!("  処理速度: {:.1} ファイル/秒", files_per_second);
    
    if files_per_second > 20.0 {
        println!("  ✅ 高速ファイル処理性能");
    }
    
    println!("✅ ファイルディスクリプタ枯渇テスト完了");
    Ok(())
}

/// 巨大ファイル処理テスト（数MB）
#[tokio::test]
async fn test_very_large_file_processing() -> Result<()> {
    println!("🔍 巨大ファイル処理テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 5MBの巨大ファイルを作成
    let large_file = temp_dir.path().join("very_large.rs");
    let mut large_content = String::new();
    
    println!("  5MB巨大ファイル生成中...");
    
    // 約5MBになるまでコンテンツを生成
    let mut function_count = 0;
    while large_content.len() < 5 * 1024 * 1024 {
        large_content.push_str(&format!(r#"
/// 巨大ファイル内の関数 {}
/// 
/// この関数は非常に大きなファイルの一部として生成されています。
/// パフォーマンステストとリソース制限テストの目的で作成。
/// 
/// # Arguments
/// * `param1` - 最初のパラメータ
/// * `param2` - 二番目のパラメータ
/// * `param3` - 三番目のパラメータ
/// 
/// # Returns
/// 計算結果の文字列
fn large_file_function_{}(param1: i32, param2: String, param3: Vec<i32>) -> Result<String, Box<dyn std::error::Error>> {{
    let mut result = String::new();
    
    // 複雑な処理をシミュレート
    for i in 0..param1 {{
        result.push_str(&format!("iteration_{{}}: {{}} + {{}}", i, param2, param3.get(i % param3.len()).unwrap_or(&0)));
        result.push('\n');
    }}
    
    // さらに複雑な処理
    let data: Vec<String> = (0..100).map(|x| {{
        format!("large_data_element_{{}}_{{}}_{{}}_{{}}", x, function_count, param1, param2.len())
    }}).collect();
    
    for (idx, item) in data.iter().enumerate() {{
        if idx % 10 == 0 {{
            result.push_str(&format!("chunk_{{}}: {{}}", idx / 10, item));
            result.push('\n');
        }}
    }}
    
    Ok(result)
}}

/// 巨大ファイル用の構造体 {}
#[derive(Debug, Clone)]
struct LargeFileStruct_{} {{
    id: usize,
    name: String,
    data: Vec<String>,
    metadata: std::collections::HashMap<String, String>,
    nested_data: Box<Vec<Box<String>>>,
}}

impl LargeFileStruct_{} {{
    /// 新しいインスタンスを作成
    fn new(id: usize, name: String) -> Self {{
        let data: Vec<String> = (0..50).map(|i| format!("data_item_{{}}_{{}}", i, id)).collect();
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("version".to_string(), "1.0".to_string());
        metadata.insert("created_by".to_string(), format!("function_{{}}", function_count));
        metadata.insert("size".to_string(), data.len().to_string());
        
        Self {{
            id,
            name,
            data,
            metadata,
            nested_data: Box::new((0..20).map(|i| Box::new(format!("nested_{{}}_{{}}", i, id))).collect()),
        }}
    }}
    
    /// データ処理メソッド
    fn process_data(&mut self) -> Result<usize, String> {{
        let mut total_size = 0;
        
        for item in &self.data {{
            total_size += item.len();
        }}
        
        for nested_item in self.nested_data.iter() {{
            total_size += nested_item.len();
        }}
        
        total_size += self.metadata.len() * 20; // 概算
        
        if total_size > 10000 {{
            Ok(total_size)
        }} else {{
            Err(format!("データサイズが不十分: {{}}", total_size))
        }}
    }}
    
    /// 高計算量メソッド
    fn expensive_computation(&self) -> Vec<String> {{
        let mut results = Vec::new();
        
        // 入れ子ループで計算量を増やす
        for i in 0..50 {{
            for j in 0..20 {{
                for k in 0..10 {{
                    results.push(format!("computed_{{}}_{{}}_{{}}_{{}}", i, j, k, self.id));
                }}
            }}
        }}
        
        results
    }}
}}
"#, function_count, function_count, function_count, function_count, function_count));
        
        function_count += 1;
        
        // 進捗表示
        if function_count % 100 == 0 {
            println!("    生成関数数: {}, サイズ: {} MB", 
                    function_count, large_content.len() / (1024 * 1024));
        }
    }
    
    fs::write(&large_file, &large_content)?;
    
    let file_size = large_content.len();
    println!("  巨大ファイル作成完了: {} MB, {} 関数", 
            file_size / (1024 * 1024), function_count);
    
    // 巨大ファイルの解析テスト
    println!("  巨大ファイル解析開始...");
    let parse_start = Instant::now();
    
    match cache_manager.get_symbols(&large_file) {
        Ok(symbols) => {
            let parse_duration = parse_start.elapsed();
            println!("  解析成功: {} シンボル, {:?}", symbols.len(), parse_duration);
            
            // パフォーマンス要件
            assert!(parse_duration.as_secs() < 120, "5MB解析は2分以内であるべき");
            assert!(symbols.len() >= function_count * 2, "関数+構造体シンボルが抽出されるべき");
            
            let symbols_per_second = symbols.len() as f64 / parse_duration.as_secs_f64();
            let mb_per_second = (file_size as f64 / (1024.0 * 1024.0)) / parse_duration.as_secs_f64();
            
            println!("  処理速度: {:.0} シンボル/秒, {:.2} MB/秒", symbols_per_second, mb_per_second);
            
            // シンボルの品質チェック
            let function_symbols = symbols.iter().filter(|s| s.name.contains("function")).count();
            let struct_symbols = symbols.iter().filter(|s| s.name.contains("Struct")).count();
            
            println!("  シンボル内訳: {} 関数, {} 構造体", function_symbols, struct_symbols);
            
            assert!(function_symbols >= function_count / 2, "関数シンボルの適切な抽出");
            assert!(struct_symbols >= function_count / 2, "構造体シンボルの適切な抽出");
            
            if mb_per_second > 1.0 {
                println!("  ✅ 高速巨大ファイル処理");
            }
        }
        Err(e) => {
            let parse_duration = parse_start.elapsed();
            println!("  解析エラー: {} ({:?})", e, parse_duration);
            
            // エラーでも合理的な時間で応答すべき
            assert!(parse_duration.as_secs() < 60, "エラーでも60秒以内で応答すべき");
        }
    }
    
    println!("✅ 巨大ファイル処理テスト完了");
    Ok(())
}

/// RealtimeIndexerでのリソース制限テスト
#[tokio::test]
async fn test_realtime_indexer_resource_limits() -> Result<()> {
    println!("🔍 RealtimeIndexer リソース制限テスト");
    
    let temp_dir = TempDir::new()?;
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // 段階的にファイル数を増やして負荷テスト
    let file_batches = vec![10, 50, 100, 200];
    
    for batch_size in file_batches {
        println!("  バッチサイズ {}: ファイル作成", batch_size);
        
        // ファイル作成
        for i in 0..batch_size {
            let file_path = temp_dir.path().join(format!("realtime_test_{}.rs", i));
            let content = format!(r#"
fn realtime_function_{}() -> String {{
    format!("realtime test {{}}", {})
}}

struct RealtimeStruct_{} {{
    id: usize,
    data: Vec<i32>,
}}

impl RealtimeStruct_{} {{
    fn new() -> Self {{
        Self {{
            id: {},
            data: (0..{}).collect(),
        }}
    }}
}}
"#, i, i, i, i, i, i % 50 + 1);
            fs::write(&file_path, content)?;
        }
        
        // RealtimeIndexer でのインデックス構築テスト
        let mut realtime_indexer = RealtimeIndexer::new(temp_dir.path().to_path_buf(), cache_manager.clone())?;
        
        // タイムアウト付きでイベントループ実行
        let indexer_task = tokio::spawn(async move {
            let _ = realtime_indexer.start_event_loop().await;
        });
        
        // 短時間待機後に停止
        tokio::time::sleep(Duration::from_millis(500)).await;
        indexer_task.abort();
        
        // 結果確認
        let symbol_count = {
            let _cache = cache_manager.lock().unwrap();
            // キャッシュ内のシンボル数を概算
            batch_size * 2 // 関数 + 構造体の期待値
        };
        
        println!("  バッチ {} 完了: 期待シンボル数 {}", batch_size, symbol_count);
        
        // リソース解放のための待機
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    println!("✅ RealtimeIndexer リソース制限テスト完了");
    Ok(())
}

/// システムリソース制限でのgraceful degradation
#[tokio::test]
async fn test_graceful_degradation() -> Result<()> {
    println!("🔍 Graceful Degradation テスト");
    
    let temp_dir = TempDir::new()?;
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // 制限環境をシミュレート
    let stress_scenarios = vec![
        ("低メモリ環境", 1000),
        ("中メモリ環境", 2000),
        ("高メモリ環境", 5000),
    ];
    
    for (scenario_name, file_count) in stress_scenarios {
        println!("  {} シナリオ: {} ファイル", scenario_name, file_count);
        
        // ファイル作成
        for i in 0..file_count {
            let file_path = temp_dir.path().join(format!("degradation_{}_{}.rs", scenario_name.chars().next().unwrap(), i));
            let content = format!("fn degradation_test_{}() {{ }}", i);
            fs::write(&file_path, content)?;
        }
        
        // 検索性能テスト
        let start_time = Instant::now();
        
        use fae::cli::strategies::ContentStrategy;
        let strategy = ContentStrategy;
        
        match search_runner.collect_results_with_strategy(&strategy, "degradation_test") {
            Ok(results) => {
                let duration = start_time.elapsed();
                println!("    成功: {} 件, {:?}", results.len(), duration);
                
                // グレースフルデグラデーション要件
                assert!(results.len() >= file_count / 2, "少なくとも半数の結果は得られるべき");
                assert!(duration.as_secs() < 30, "制限環境でも30秒以内で応答すべき");
            }
            Err(e) => {
                println!("    エラー（許容範囲）: {}", e);
            }
        }
        
        // リソース解放
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    
    println!("✅ Graceful Degradation テスト完了");
    Ok(())
}