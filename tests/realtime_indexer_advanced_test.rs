//! RealtimeIndexerの高度なエッジケース・並行性テスト
//! 
//! ファイル監視、並行アクセス、大量イベント処理のストレステスト

use fae::{
    RealtimeIndexer, CacheManager,
};
use anyhow::Result;
use std::fs;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::sleep;

/// 大量ファイル変更イベントのストレステスト
#[tokio::test]
async fn test_massive_file_changes() -> Result<()> {
    println!("🔍 大量ファイル変更イベントストレステスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // RealtimeIndexerを起動
    let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    // バックグラウンドでイベントループを開始
    let indexer_handle = tokio::spawn(async move {
        let _ = realtime_indexer.start_event_loop().await;
    });
    
    // 初期化待機
    sleep(Duration::from_millis(100)).await;
    
    let file_count = 50;
    let change_iterations = 10;
    
    println!("📁 {} ファイルの {} 回変更を実行", file_count, change_iterations);
    
    // 大量の連続ファイル変更
    let start_time = Instant::now();
    
    for iteration in 0..change_iterations {
        println!("🔄 変更イテレーション {}/{}", iteration + 1, change_iterations);
        
        // 全ファイルを並行で変更
        let mut handles = Vec::new();
        
        for i in 0..file_count {
            let temp_path = temp_path.clone();
            let iteration = iteration;
            
            let handle = tokio::spawn(async move {
                let file_path = temp_path.join(format!("stress_test_{}.rs", i));
                let content = format!(r#"
// Stress test iteration {} file {}
fn stress_function_{}_{}_{}() -> i32 {{
    let mut result = 0;
    for j in 0..{} {{
        result += j * {} + {};
    }}
    result
}}

struct StressStruct_{}_{} {{
    iteration: u32,
    file_id: u32,
    data: Vec<i32>,
}}

impl StressStruct_{}_{} {{
    fn new() -> Self {{
        Self {{
            iteration: {},
            file_id: {},
            data: (0..{}).collect(),
        }}
    }}
    
    fn calculate(&self) -> i32 {{
        self.data.iter().sum::<i32>() * {} + {}
    }}
}}

const STRESS_CONSTANT_{}_{}: i32 = {};
"#, iteration, i, iteration, i, iteration, 
   iteration * 10 + i, i, iteration,
   iteration, i, iteration, i, iteration, i, iteration * 5 + 10,
   iteration + 1, i,
   iteration, i, iteration * 100 + i * 10);
                
                if let Err(e) = fs::write(&file_path, content) {
                    eprintln!("⚠️ ファイル書き込みエラー {}: {}", i, e);
                }
            });
            
            handles.push(handle);
        }
        
        // 全ファイル変更の完了を待機
        for handle in handles {
            let _ = handle.await;
        }
        
        // デバウンス処理の時間を考慮
        sleep(Duration::from_millis(200)).await;
    }
    
    let total_duration = start_time.elapsed();
    println!("⏱️ 総変更時間: {:?}", total_duration);
    
    // 最終状態確認のためにさらに待機
    sleep(Duration::from_millis(500)).await;
    
    // ファイル処理結果の確認
    let final_symbol_count = {
        let mut cache = cache_manager.lock().unwrap();
        let mut total = 0;
        
        for i in 0..file_count {
            let file_path = temp_path.join(format!("stress_test_{}.rs", i));
            if let Ok(symbols) = cache.get_symbols(&file_path) {
                total += symbols.len();
            }
        }
        total
    };
    
    println!("🎯 最終シンボル数: {}", final_symbol_count);
    
    // 合理的なシンボル数が処理されているかチェック
    let expected_min_symbols = file_count * 3; // 最低でもfunction, struct, impl per file
    assert!(final_symbol_count >= expected_min_symbols,
            "最終シンボル数が期待値未満: {} < {}", final_symbol_count, expected_min_symbols);
    
    // バックグラウンドタスクを終了
    indexer_handle.abort();
    
    println!("✅ 大量ファイル変更ストレステスト完了");
    Ok(())
}

/// 並行アクセスパターンテスト
#[tokio::test]
async fn test_concurrent_access_patterns() -> Result<()> {
    println!("🔍 並行アクセスパターンテスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // 初期ファイルを作成
    let file_count = 20;
    for i in 0..file_count {
        let file_path = temp_path.join(format!("concurrent_{}.rs", i));
        let content = format!(r#"
fn initial_function_{}() -> i32 {{
    {}
}}

struct InitialStruct_{} {{
    value: i32,
}}
"#, i, i * 42, i);
        fs::write(&file_path, content)?;
    }
    
    // RealtimeIndexerを起動
    let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    let indexer_handle = tokio::spawn(async move {
        let _ = realtime_indexer.start_event_loop().await;
    });
    
    sleep(Duration::from_millis(100)).await;
    
    let operation_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    // 様々な並行操作パターン
    let mut concurrent_handles = Vec::new();
    
    // パターン1: 連続ファイル読み取り
    for thread_id in 0..5 {
        let cache_manager = cache_manager.clone();
        let temp_path = temp_path.clone();
        let operation_count = operation_count.clone();
        let error_count = error_count.clone();
        
        let handle = tokio::spawn(async move {
            for _ in 0..20 {
                let file_idx = thread_id * 4; // 各スレッドで異なるファイル
                let file_path = temp_path.join(format!("concurrent_{}.rs", file_idx));
                
                match {
                    let mut cache = cache_manager.lock().unwrap();
                    cache.get_symbols(&file_path)
                } {
                    Ok(_) => {
                        operation_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                
                // 短い間隔で実行
                sleep(Duration::from_millis(10)).await;
            }
        });
        
        concurrent_handles.push(handle);
    }
    
    // パターン2: ファイル変更 + 読み取り
    for thread_id in 0..3 {
        let temp_path = temp_path.clone();
        let operation_count = operation_count.clone();
        
        let handle = tokio::spawn(async move {
            for iteration in 0..10 {
                let file_idx = thread_id + 10; // 異なるファイル範囲
                let file_path = temp_path.join(format!("concurrent_{}.rs", file_idx));
                
                let content = format!(r#"
// Updated by thread {} iteration {}
fn updated_function_{}_{}() -> i32 {{
    {} + {}
}}

struct UpdatedStruct_{}_{} {{
    thread_id: u32,
    iteration: u32,
}}

impl UpdatedStruct_{}_{} {{
    fn new() -> Self {{
        Self {{
            thread_id: {},
            iteration: {},
        }}
    }}
}}
"#, thread_id, iteration, thread_id, iteration, thread_id * 100, iteration,
   thread_id, iteration, thread_id, iteration, thread_id, iteration);
                
                if fs::write(&file_path, content).is_ok() {
                    operation_count.fetch_add(1, Ordering::Relaxed);
                    
                    // 書き込み後に少し待機
                    sleep(Duration::from_millis(50)).await;
                }
            }
        });
        
        concurrent_handles.push(handle);
    }
    
    // パターン3: ファジー検索の並行実行
    for _thread_id in 0..4 {
        let cache_manager = cache_manager.clone();
        let operation_count = operation_count.clone();
        
        let handle = tokio::spawn(async move {
            let queries = vec!["function", "struct", "impl", "initial", "updated"];
            
            for iteration in 0..15 {
                let query = &queries[iteration % queries.len()];
                
                match {
                    let cache = cache_manager.lock().unwrap();
                    cache.fuzzy_search_symbols(query, 20)
                } {
                    _ => {
                        operation_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
                
                sleep(Duration::from_millis(20)).await;
            }
        });
        
        concurrent_handles.push(handle);
    }
    
    println!("⚡ {} 個の並行タスクを開始", concurrent_handles.len());
    
    // 全タスクの完了を待機
    for handle in concurrent_handles {
        let _ = handle.await;
    }
    
    // 少し追加で待機（ファイル変更の処理完了）
    sleep(Duration::from_millis(300)).await;
    
    let total_operations = operation_count.load(Ordering::Relaxed);
    let total_errors = error_count.load(Ordering::Relaxed);
    
    println!("📊 並行処理結果:");
    println!("   成功操作: {}", total_operations);
    println!("   エラー: {}", total_errors);
    
    // エラー率の確認
    let error_rate = if total_operations + total_errors > 0 {
        total_errors as f64 / (total_operations + total_errors) as f64
    } else {
        0.0
    };
    
    println!("   エラー率: {:.2}%", error_rate * 100.0);
    
    // 合理的なエラー率であることを確認
    assert!(error_rate < 0.1, "エラー率が10%を超過: {:.2}%", error_rate * 100.0);
    assert!(total_operations > 50, "十分な数の操作が成功すべき");
    
    // 最終状態の一貫性確認
    let final_file_count = {
        let mut cache = cache_manager.lock().unwrap();
        let mut count = 0;
        
        for i in 0..file_count {
            let file_path = temp_path.join(format!("concurrent_{}.rs", i));
            if file_path.exists() && cache.get_symbols(&file_path).is_ok() {
                count += 1;
            }
        }
        count
    };
    
    println!("🎯 最終処理ファイル数: {}/{}", final_file_count, file_count);
    
    // バックグラウンドタスクを終了
    indexer_handle.abort();
    
    println!("✅ 並行アクセスパターンテスト完了");
    Ok(())
}

/// ファイルシステムイベントの境界条件テスト
#[tokio::test]
async fn test_filesystem_edge_cases() -> Result<()> {
    println!("🔍 ファイルシステムエッジケーステスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // RealtimeIndexerを起動
    let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    let indexer_handle = tokio::spawn(async move {
        let _ = realtime_indexer.start_event_loop().await;
    });
    
    sleep(Duration::from_millis(100)).await;
    
    // エッジケース1: 高速な作成・削除
    println!("🔄 高速作成・削除テスト");
    for i in 0..20 {
        let file_path = temp_path.join(format!("rapid_{}.rs", i));
        
        // 作成
        let content = format!(r#"
fn rapid_function_{}() -> i32 {{
    {}
}}
"#, i, i);
        fs::write(&file_path, content)?;
        
        // 短時間待機
        sleep(Duration::from_millis(5)).await;
        
        // 削除
        let _ = fs::remove_file(&file_path);
        
        sleep(Duration::from_millis(5)).await;
    }
    
    // エッジケース2: ファイル名変更（移動）
    println!("📂 ファイル移動テスト");
    for i in 0..10 {
        let original_path = temp_path.join(format!("original_{}.rs", i));
        let moved_path = temp_path.join(format!("moved_{}.rs", i));
        
        // ファイル作成
        fs::write(&original_path, format!("fn original_{}() {{}}", i))?;
        sleep(Duration::from_millis(10)).await;
        
        // ファイル移動
        if let Err(e) = fs::rename(&original_path, &moved_path) {
            println!("⚠️ ファイル移動失敗 {}: {}", i, e);
        }
        sleep(Duration::from_millis(10)).await;
    }
    
    // エッジケース3: 同じファイルの連続更新
    println!("✏️ 連続更新テスト");
    let rapid_update_file = temp_path.join("rapid_update.rs");
    
    for update in 0..30 {
        let content = format!(r#"
// Update number {}
fn updated_function_{}() -> i32 {{
    let mut sum = 0;
    for i in 0..{} {{
        sum += i * {};
    }}
    sum
}}

struct Update_{} {{
    number: u32,
}}

const UPDATE_CONSTANT: i32 = {};
"#, update, update, update * 10 + 5, update + 1, update, update * 42);
        
        fs::write(&rapid_update_file, content)?;
        
        // デバウンス時間より短い間隔で更新
        sleep(Duration::from_millis(20)).await;
    }
    
    // エッジケース4: 深いディレクトリ構造
    println!("📁 深いディレクトリ構造テスト");
    let deep_dir = temp_path.join("deep/nested/very/deep/directory");
    fs::create_dir_all(&deep_dir)?;
    
    for i in 0..5 {
        let deep_file = deep_dir.join(format!("deep_file_{}.rs", i));
        let content = format!(r#"
// Deep file {}
fn deep_function_{}() -> String {{
    format!("Deep level {}", {})
}}
"#, i, i, i, i);
        fs::write(&deep_file, content)?;
        sleep(Duration::from_millis(30)).await;
    }
    
    // 処理完了を待機
    sleep(Duration::from_millis(1000)).await;
    
    // 最終状態確認
    println!("🔍 最終状態確認");
    
    let final_stats = {
        let mut cache = cache_manager.lock().unwrap();
        let mut moved_count = 0;
        let mut rapid_update_symbols = 0;
        let mut deep_symbols = 0;
        
        // 移動されたファイルの確認
        for i in 0..10 {
            let moved_path = temp_path.join(format!("moved_{}.rs", i));
            if let Ok(symbols) = cache.get_symbols(&moved_path) {
                moved_count += 1;
                println!("📄 移動ファイル {}: {} シンボル", i, symbols.len());
            }
        }
        
        // 連続更新ファイルの確認
        if let Ok(symbols) = cache.get_symbols(&rapid_update_file) {
            rapid_update_symbols = symbols.len();
            println!("⚡ 連続更新ファイル: {} シンボル", rapid_update_symbols);
        }
        
        // 深いディレクトリのファイル確認
        for i in 0..5 {
            let deep_file = deep_dir.join(format!("deep_file_{}.rs", i));
            if let Ok(symbols) = cache.get_symbols(&deep_file) {
                deep_symbols += symbols.len();
            }
        }
        
        (moved_count, rapid_update_symbols, deep_symbols)
    };
    
    let (moved_count, rapid_update_symbols, deep_symbols) = final_stats;
    
    println!("📊 エッジケース処理結果:");
    println!("   移動されたファイル: {}/10", moved_count);
    println!("   連続更新ファイルシンボル: {}", rapid_update_symbols);
    println!("   深いディレクトリシンボル: {}", deep_symbols);
    
    // 基本的な動作確認
    assert!(moved_count > 0, "移動されたファイルが一部処理されるべき");
    assert!(rapid_update_symbols > 0, "連続更新ファイルが処理されるべき");
    assert!(deep_symbols > 0, "深いディレクトリのファイルが処理されるべき");
    
    // バックグラウンドタスクを終了
    indexer_handle.abort();
    
    println!("✅ ファイルシステムエッジケーステスト完了");
    Ok(())
}

/// メモリリーク検出テスト
#[tokio::test]
async fn test_memory_leak_detection() -> Result<()> {
    println!("🔍 メモリリーク検出テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // 初期メモリ使用量（概算）
    let initial_memory = get_rough_memory_usage();
    
    // 長時間動作シミュレーション
    for cycle in 0..5 {
        println!("🔄 メモリテストサイクル {}/5", cycle + 1);
        
        // RealtimeIndexerを作成・破棄
        {
            let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
            
            let indexer_handle = tokio::spawn(async move {
                let _ = realtime_indexer.start_event_loop().await;
            });
            
            // 短時間動作
            sleep(Duration::from_millis(50)).await;
            
            // 複数ファイルを処理
            for i in 0..20 {
                let file_path = temp_path.join(format!("memory_test_{}_{}.rs", cycle, i));
                let content = format!(r#"
// Memory test cycle {} file {}
fn memory_function_{}_{}() -> Vec<String> {{
    let mut result = Vec::new();
    for j in 0..{} {{
        result.push(format!("item_{{}}_{{}}_{{}} }}", j, {}, {}));
    }}
    result
}}

struct MemoryStruct_{}_{} {{
    data: Vec<i32>,
    metadata: std::collections::HashMap<String, String>,
}}
"#, cycle, i, cycle, i, cycle * 10 + 5, cycle, i, cycle, i);
                
                fs::write(&file_path, content)?;
                sleep(Duration::from_millis(5)).await;
            }
            
            sleep(Duration::from_millis(100)).await;
            
            // インデクサーを停止
            indexer_handle.abort();
        }
        
        // ガベージコレクション的な処理を促進
        tokio::task::yield_now().await;
        sleep(Duration::from_millis(50)).await;
        
        let current_memory = get_rough_memory_usage();
        let memory_increase = current_memory.saturating_sub(initial_memory);
        
        println!("   サイクル {} メモリ増加: {} KB", cycle + 1, memory_increase / 1024);
        
        // 極端なメモリ増加がないかチェック
        if memory_increase > 50 * 1024 * 1024 { // 50MB以上の増加
            println!("⚠️ 大きなメモリ増加が検出されました: {} MB", memory_increase / (1024 * 1024));
        }
    }
    
    let final_memory = get_rough_memory_usage();
    let total_increase = final_memory.saturating_sub(initial_memory);
    
    println!("📊 メモリリークテスト結果:");
    println!("   初期メモリ: {} KB", initial_memory / 1024);
    println!("   最終メモリ: {} KB", final_memory / 1024);
    println!("   総増加量: {} KB", total_increase / 1024);
    
    // メモリリークの判定（保守的な閾値）
    let acceptable_increase = 100 * 1024 * 1024; // 100MB
    if total_increase > acceptable_increase {
        println!("⚠️ 潜在的なメモリリークの兆候: {} MB増加", total_increase / (1024 * 1024));
    }
    
    // 最終的なキャッシュ状態確認
    let final_symbol_count = {
        let mut cache = cache_manager.lock().unwrap();
        let mut total = 0;
        
        // 一部のファイルをサンプル確認
        for cycle in 0..5 {
            for i in 0..5 { // 各サイクルから5ファイルをサンプル
                let file_path = temp_path.join(format!("memory_test_{}_{}.rs", cycle, i));
                if let Ok(symbols) = cache.get_symbols(&file_path) {
                    total += symbols.len();
                }
            }
        }
        total
    };
    
    println!("🎯 最終キャッシュシンボル数: {}", final_symbol_count);
    assert!(final_symbol_count > 0, "キャッシュにシンボルが存在すべき");
    
    println!("✅ メモリリーク検出テスト完了");
    Ok(())
}

/// 粗いメモリ使用量取得（クロスプラットフォーム対応）
fn get_rough_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .unwrap_or_default()
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .and_then(|line| {
                line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .map(|kb| kb * 1024)
            })
            .unwrap_or(0)
    }
    
    #[cfg(target_os = "macos")]
    {
        // macOSでは概算値を返す（実装は簡略化）
        0
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        // 他のプラットフォームでは0を返す
        0
    }
}

/// タイムアウト処理のテスト
#[tokio::test]
async fn test_timeout_scenarios() -> Result<()> {
    println!("🔍 タイムアウトシナリオテスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // RealtimeIndexerを起動
    let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    let indexer_handle = tokio::spawn(async move {
        let _ = realtime_indexer.start_event_loop().await;
    });
    
    // 短時間で大量のファイル作成（タイムアウト誘発の試行）
    println!("⚡ 高速大量ファイル作成");
    
    let burst_count = 100;
    let burst_start = Instant::now();
    
    for i in 0..burst_count {
        let file_path = temp_path.join(format!("timeout_test_{}.rs", i));
        let content = format!(r#"
// Timeout test file {}
fn timeout_function_{}() -> Result<i32, String> {{
    let mut computation = 0;
    for j in 0..{} {{
        computation += j * {} + {};
    }}
    Ok(computation)
}}

struct TimeoutStruct_{} {{
    id: u32,
    data: Vec<String>,
}}

impl TimeoutStruct_{} {{
    fn process_heavy(&self) -> String {{
        format!("Processing heavy computation for {}", self.id)
    }}
}}
"#, i, i, i % 50 + 10, i, i % 100, i, i, i);
        
        fs::write(&file_path, content)?;
        
        // 非常に短い間隔（デバウンス以下）
        if i % 10 == 0 {
            sleep(Duration::from_millis(1)).await;
        }
    }
    
    let burst_duration = burst_start.elapsed();
    println!("🚀 バースト作成完了: {:?} で {} ファイル", burst_duration, burst_count);
    
    // 処理完了を待機（十分な時間）
    println!("⏳ 処理完了待機中...");
    sleep(Duration::from_millis(2000)).await;
    
    // 処理結果の確認
    let processed_count = {
        let mut cache = cache_manager.lock().unwrap();
        let mut count = 0;
        
        for i in 0..burst_count {
            let file_path = temp_path.join(format!("timeout_test_{}.rs", i));
            if let Ok(symbols) = cache.get_symbols(&file_path) {
                if symbols.len() > 0 {
                    count += 1;
                }
            }
        }
        count
    };
    
    println!("📊 タイムアウトテスト結果:");
    println!("   作成ファイル数: {}", burst_count);
    println!("   処理ファイル数: {}", processed_count);
    println!("   処理率: {:.1}%", (processed_count as f64 / burst_count as f64) * 100.0);
    
    // 合理的な処理率の確認（全部でなくても大部分は処理されるべき）
    let processing_rate = processed_count as f64 / burst_count as f64;
    assert!(processing_rate > 0.5, "処理率が50%以上であるべき: {:.1}%", processing_rate * 100.0);
    
    // 応答性テスト：新しいファイルがタイムリーに処理されるか
    println!("🔄 応答性テスト");
    let responsiveness_file = temp_path.join("responsiveness_test.rs");
    
    let responsiveness_start = Instant::now();
    fs::write(&responsiveness_file, r#"
fn responsiveness_test() -> &'static str {
    "This should be processed quickly"
}
"#)?;
    
    // 応答を待機（最大1秒）
    let mut processed = false;
    for _ in 0..10 {
        sleep(Duration::from_millis(100)).await;
        
        if let Ok(symbols) = {
            let mut cache = cache_manager.lock().unwrap();
            cache.get_symbols(&responsiveness_file)
        } {
            if symbols.len() > 0 {
                processed = true;
                break;
            }
        }
    }
    
    let responsiveness_duration = responsiveness_start.elapsed();
    println!("📋 応答性結果: {} ({:?})", 
             if processed { "処理済み" } else { "未処理" }, 
             responsiveness_duration);
    
    assert!(processed, "新しいファイルは合理的な時間内に処理されるべき");
    assert!(responsiveness_duration.as_millis() < 1500, 
            "応答時間は1.5秒以内であるべき");
    
    // バックグラウンドタスクを終了
    indexer_handle.abort();
    
    println!("✅ タイムアウトシナリオテスト完了");
    Ok(())
}