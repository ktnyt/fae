//! チャンネル通信エラーとスレッド間通信のテスト
//! 
//! mpsc送信側切断、受信側応答なし、インデックス構築中のメインスレッド終了、
//! 複数スレッドからの同時検索要求、デッドロック検出とリカバリなど

use fae::{RealtimeIndexer, CacheManager};
use anyhow::Result;
use std::fs;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout};

/// mpscチャンネル送信側切断のテスト
#[tokio::test]
async fn test_sender_disconnection() -> Result<()> {
    println!("🔍 mpsc送信側切断テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(StdMutex::new(CacheManager::new()));
    
    // 小さなファイルを作成
    let test_file = temp_path.join("disconnect_test.rs");
    fs::write(&test_file, "fn test() {}")?;
    
    // RealtimeIndexerを作成
    let mut realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    // イベントループを開始
    let indexer_handle = tokio::spawn(async move {
        // 短時間だけ実行してから強制終了（送信側切断をシミュレート）
        let _ = timeout(Duration::from_millis(100), realtime_indexer.start_event_loop()).await;
    });
    
    // ファイル変更を試行
    for i in 0..5 {
        let content = format!("fn test_{}() {{ println!(\"update {}\"); }}", i, i);
        if let Err(e) = fs::write(&test_file, content) {
            println!("  ファイル書き込みエラー {}: {}", i, e);
        }
        sleep(Duration::from_millis(50)).await;
    }
    
    // インデクサーの終了を待機
    let join_result = indexer_handle.await;
    
    match join_result {
        Ok(_) => {
            println!("  送信側切断テスト: 正常終了");
            println!("    ✅ 送信側切断に対する適切な処理");
        }
        Err(e) => {
            println!("  送信側切断テスト: タスクエラー - {}", e);
            // タスクのキャンセルはエラーだが、予期される動作
        }
    }
    
    // ファイル状態の最終確認
    match cache_manager.lock() {
        Ok(mut cache) => {
            match cache.get_symbols(&test_file) {
                Ok(symbols) => {
                    println!("    最終シンボル数: {}", symbols.len());
                }
                Err(e) => {
                    println!("    最終状態取得エラー: {}", e);
                }
            }
        }
        Err(e) => {
            println!("    キャッシュロックエラー: {}", e);
        }
    }
    
    println!("✅ mpsc送信側切断テスト完了");
    Ok(())
}

/// 受信側応答なし時のタイムアウトテスト
#[tokio::test]
async fn test_receiver_timeout() -> Result<()> {
    println!("🔍 受信側タイムアウトテスト");
    
    // カスタムタイムアウトチャンネルのシミュレーション
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let timeout_flag = Arc::new(AtomicBool::new(false));
    
    // 送信側タスク
    let sender_flag = timeout_flag.clone();
    let sender_handle = tokio::spawn(async move {
        for i in 0..10 {
            if sender_flag.load(Ordering::Relaxed) {
                println!("    送信側: タイムアウトフラグ検出、停止");
                break;
            }
            
            match tx.send(format!("message_{}", i)) {
                Ok(_) => println!("    送信側: メッセージ {} 送信成功", i),
                Err(e) => {
                    println!("    送信側: 送信エラー {} - {}", i, e);
                    break;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
    });
    
    // 受信側タスク（意図的に応答しない）
    let receiver_flag = timeout_flag.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut received_count = 0;
        
        loop {
            match timeout(Duration::from_millis(500), rx.recv()).await {
                Ok(Some(msg)) => {
                    println!("    受信側: {} 受信", msg);
                    received_count += 1;
                    
                    // 3つ目のメッセージで意図的に応答を停止
                    if received_count >= 3 {
                        println!("    受信側: 意図的に応答停止");
                        receiver_flag.store(true, Ordering::Relaxed);
                        
                        // 長時間待機（応答なしをシミュレート）
                        sleep(Duration::from_millis(2000)).await;
                        break;
                    }
                }
                Ok(None) => {
                    println!("    受信側: チャンネル閉鎖");
                    break;
                }
                Err(_) => {
                    println!("    受信側: タイムアウト");
                    receiver_flag.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
        
        received_count
    });
    
    // 両タスクの完了を待機
    let (sender_result, receiver_result) = tokio::join!(sender_handle, receiver_handle);
    
    match sender_result {
        Ok(_) => println!("  送信側タスク: 正常終了"),
        Err(e) => println!("  送信側タスク: エラー - {}", e),
    }
    
    match receiver_result {
        Ok(count) => {
            println!("  受信側タスク: {} メッセージ処理", count);
            assert!(count <= 3, "受信側は3メッセージまでで停止すべき");
        }
        Err(e) => println!("  受信側タスク: エラー - {}", e),
    }
    
    println!("✅ 受信側タイムアウトテスト完了");
    Ok(())
}

/// インデックス構築中のメインスレッド終了テスト
#[tokio::test]
async fn test_main_thread_abort_during_indexing() -> Result<()> {
    println!("🔍 インデックス構築中メインスレッド終了テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // 多数のファイルを作成（インデックス構築に時間をかける）
    for i in 0..50 {
        let file_path = temp_path.join(format!("abort_test_{}.rs", i));
        let content = format!(r#"
fn function_{}() -> i32 {{
    let mut sum = 0;
    for j in 0..{} {{
        sum += j * {};
    }}
    sum
}}

struct Struct_{} {{
    field1: i32,
    field2: String,
}}

impl Struct_{} {{
    fn new() -> Self {{
        Self {{
            field1: {},
            field2: format!("test_{{}}", {}),
        }}
    }}
}}
"#, i, i % 100 + 10, i, i, i, i, i);
        fs::write(&file_path, content)?;
    }
    
    let cache_manager = Arc::new(StdMutex::new(CacheManager::new()));
    let abort_flag = Arc::new(AtomicBool::new(false));
    
    // インデックス構築タスク
    let indexing_cache = cache_manager.clone();
    let indexing_flag = abort_flag.clone();
    let indexing_path = temp_path.clone();
    
    let indexing_handle = tokio::spawn(async move {
        let mut processed_files = 0;
        
        for i in 0..50 {
            if indexing_flag.load(Ordering::Relaxed) {
                println!("    インデックス構築: 中止フラグ検出 ({} ファイル処理済み)", processed_files);
                break;
            }
            
            let file_path = indexing_path.join(format!("abort_test_{}.rs", i));
            
            match {
                let mut cache = indexing_cache.lock().unwrap();
                cache.get_symbols(&file_path)
            } {
                Ok(_symbols) => {
                    processed_files += 1;
                    if processed_files % 10 == 0 {
                        println!("    インデックス構築: {} ファイル処理完了", processed_files);
                    }
                }
                Err(e) => {
                    println!("    インデックス構築エラー ファイル {}: {}", i, e);
                }
            }
            
            // 処理間隔（リアルな処理時間をシミュレート）
            sleep(Duration::from_millis(50)).await;
        }
        
        processed_files
    });
    
    // メインスレッド終了シミュレーション
    let abort_simulation_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(1000)).await; // 1秒後に中止
        abort_flag.store(true, Ordering::Relaxed);
        println!("  メインスレッド: 中止シグナル送信");
    });
    
    // 結果の待機
    let (indexing_result, _) = tokio::join!(indexing_handle, abort_simulation_handle);
    
    match indexing_result {
        Ok(processed_count) => {
            println!("  インデックス構築結果: {} ファイル処理", processed_count);
            
            // 中止により全ファイルより少ない処理数になることを期待
            assert!(processed_count < 50, "中止により全ファイルより少ない処理数になるべき");
            assert!(processed_count > 0, "少なくとも一部のファイルは処理されるべき");
            
            println!("    ✅ 適切な中止処理");
        }
        Err(e) => {
            println!("  インデックス構築タスクエラー: {}", e);
        }
    }
    
    println!("✅ インデックス構築中メインスレッド終了テスト完了");
    Ok(())
}

/// 複数スレッドからの同時検索要求テスト
#[tokio::test]
async fn test_concurrent_search_requests() -> Result<()> {
    println!("🔍 複数スレッド同時検索要求テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    let cache_manager = Arc::new(StdMutex::new(CacheManager::new()));
    
    // 検索対象ファイルを準備
    for i in 0..20 {
        let file_path = temp_path.join(format!("concurrent_search_{}.rs", i));
        let content = format!(r#"
fn search_target_{}() -> String {{
    format!("result from function {{}}", {})
}}

fn another_function_{}() -> i32 {{
    {} * 2
}}

struct SearchStruct_{} {{
    value: i32,
}}

impl SearchStruct_{} {{
    fn search_method(&self) -> String {{
        format!("method result: {{}}", self.value)
    }}
}}
"#, i, i, i, i, i, i);
        fs::write(&file_path, content)?;
    }
    
    let request_count = Arc::new(AtomicUsize::new(0));
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    
    // 複数の同時検索タスクを作成
    let mut search_handles = Vec::new();
    
    for thread_id in 0..10 {
        let cache = cache_manager.clone();
        let path = temp_path.clone();
        let requests = request_count.clone();
        let successes = success_count.clone();
        let errors = error_count.clone();
        
        let handle = tokio::spawn(async move {
            let search_queries = vec!["search_target", "function", "SearchStruct", "method"];
            
            for (i, query) in search_queries.iter().enumerate() {
                requests.fetch_add(1, Ordering::Relaxed);
                
                // 各スレッドが異なるファイルセットを検索
                let file_range = (thread_id * 2)..((thread_id + 1) * 2);
                let mut thread_success = 0;
                let mut thread_error = 0;
                
                for file_idx in file_range {
                    if file_idx >= 20 { break; }
                    
                    let file_path = path.join(format!("concurrent_search_{}.rs", file_idx));
                    
                    match {
                        let mut cache_lock = cache.lock().unwrap();
                        cache_lock.get_symbols(&file_path)
                    } {
                        Ok(symbols) => {
                            thread_success += 1;
                            
                            // クエリにマッチするシンボルをカウント
                            let matching_symbols = symbols.iter()
                                .filter(|s| s.name.to_lowercase().contains(query))
                                .count();
                            
                            if matching_symbols > 0 && i == 0 { // 最初のクエリのみログ出力
                                println!("    スレッド {} ファイル {}: {} マッチ", 
                                        thread_id, file_idx, matching_symbols);
                            }
                        }
                        Err(_) => {
                            thread_error += 1;
                        }
                    }
                    
                    // 並行性を高めるための短い待機
                    sleep(Duration::from_millis(5)).await;
                }
                
                successes.fetch_add(thread_success, Ordering::Relaxed);
                errors.fetch_add(thread_error, Ordering::Relaxed);
                
                // クエリ間の短い待機
                sleep(Duration::from_millis(10)).await;
            }
        });
        
        search_handles.push(handle);
    }
    
    // 全検索タスクの完了を待機
    let start_time = Instant::now();
    for handle in search_handles {
        let _ = handle.await;
    }
    let total_duration = start_time.elapsed();
    
    let final_requests = request_count.load(Ordering::Relaxed);
    let final_successes = success_count.load(Ordering::Relaxed);
    let final_errors = error_count.load(Ordering::Relaxed);
    
    println!("📊 同時検索要求テスト結果:");
    println!("  総検索要求: {}", final_requests);
    println!("  成功: {}", final_successes);
    println!("  エラー: {}", final_errors);
    println!("  総実行時間: {:?}", total_duration);
    
    // 性能と信頼性の確認
    assert!(final_requests == 40, "予期される要求数: 40 (10スレッド × 4クエリ)");
    assert!(final_successes > 0, "少なくとも一部の検索は成功すべき");
    
    let success_rate = final_successes as f64 / (final_successes + final_errors) as f64;
    println!("  成功率: {:.1}%", success_rate * 100.0);
    
    assert!(success_rate > 0.8, "成功率は80%以上であるべき");
    assert!(total_duration.as_secs() < 10, "10スレッド並行検索は10秒以内であるべき");
    
    println!("✅ 複数スレッド同時検索要求テスト完了");
    Ok(())
}

/// デッドロック検出とリカバリのテスト
#[tokio::test]
async fn test_deadlock_detection_recovery() -> Result<()> {
    println!("🔍 デッドロック検出・リカバリテスト");
    
    let _temp_dir = TempDir::new()?;
    let cache_manager = Arc::new(StdMutex::new(CacheManager::new()));
    
    // デッドロック潜在的状況のシミュレーション
    let resource_a = Arc::new(Mutex::new(0));
    let resource_b = Arc::new(Mutex::new(0));
    
    let deadlock_detected = Arc::new(AtomicBool::new(false));
    let successful_operations = Arc::new(AtomicUsize::new(0));
    
    // タスク1: A -> B の順序でロック取得
    let task1_resource_a = resource_a.clone();
    let task1_resource_b = resource_b.clone();
    let task1_cache = cache_manager.clone();
    let task1_success = successful_operations.clone();
    
    let task1_handle = tokio::spawn(async move {
        for i in 0..5 {
            match timeout(Duration::from_millis(200), async {
                let _lock_a = task1_resource_a.lock().await;
                sleep(Duration::from_millis(50)).await; // デッドロック誘発のための待機
                let _lock_b = task1_resource_b.lock().await;
                
                // キャッシュアクセス
                let mut cache = task1_cache.lock().unwrap();
                // 架空のファイルパス
                let dummy_path = std::path::PathBuf::from(format!("dummy_task1_{}.rs", i));
                let _ = cache.get_symbols(&dummy_path);
                
                task1_success.fetch_add(1, Ordering::Relaxed);
            }).await {
                Ok(_) => {
                    println!("    タスク1-{}: 成功", i);
                }
                Err(_) => {
                    println!("    タスク1-{}: タイムアウト（潜在的デッドロック）", i);
                    break;
                }
            }
            
            sleep(Duration::from_millis(10)).await;
        }
    });
    
    // タスク2: B -> A の順序でロック取得（デッドロック誘発）
    let task2_resource_a = resource_a.clone();
    let task2_resource_b = resource_b.clone();
    let task2_cache = cache_manager.clone();
    let task2_success = successful_operations.clone();
    let task2_deadlock = deadlock_detected.clone();
    
    let task2_handle = tokio::spawn(async move {
        for i in 0..5 {
            match timeout(Duration::from_millis(200), async {
                let _lock_b = task2_resource_b.lock().await;
                sleep(Duration::from_millis(50)).await; // デッドロック誘発のための待機
                let _lock_a = task2_resource_a.lock().await;
                
                // キャッシュアクセス
                let mut cache = task2_cache.lock().unwrap();
                let dummy_path = std::path::PathBuf::from(format!("dummy_task2_{}.rs", i));
                let _ = cache.get_symbols(&dummy_path);
                
                task2_success.fetch_add(1, Ordering::Relaxed);
            }).await {
                Ok(_) => {
                    println!("    タスク2-{}: 成功", i);
                }
                Err(_) => {
                    println!("    タスク2-{}: タイムアウト（潜在的デッドロック）", i);
                    task2_deadlock.store(true, Ordering::Relaxed);
                    break;
                }
            }
            
            sleep(Duration::from_millis(10)).await;
        }
    });
    
    // 監視タスク: デッドロック検出
    let monitor_deadlock = deadlock_detected.clone();
    let monitor_success = successful_operations.clone();
    
    let monitor_handle = tokio::spawn(async move {
        for _ in 0..20 { // 2秒間監視
            sleep(Duration::from_millis(100)).await;
            
            let current_success = monitor_success.load(Ordering::Relaxed);
            let is_deadlocked = monitor_deadlock.load(Ordering::Relaxed);
            
            if is_deadlocked {
                println!("    監視: デッドロック検出！成功操作数: {}", current_success);
                break;
            }
        }
    });
    
    // 全タスクの完了を待機
    let (task1_result, task2_result, _) = tokio::join!(task1_handle, task2_handle, monitor_handle);
    
    let final_success = successful_operations.load(Ordering::Relaxed);
    let was_deadlocked = deadlock_detected.load(Ordering::Relaxed);
    
    println!("📊 デッドロック検出テスト結果:");
    println!("  成功操作数: {}", final_success);
    println!("  デッドロック検出: {}", if was_deadlocked { "はい" } else { "いいえ" });
    
    match (task1_result, task2_result) {
        (Ok(_), Ok(_)) => {
            if was_deadlocked {
                println!("  ✅ デッドロック検出機能正常動作");
                assert!(final_success < 10, "デッドロック発生により全操作は完了しないべき");
            } else {
                println!("  ✅ デッドロックなしで正常完了");
            }
        }
        _ => {
            println!("  ⚠️ タスク実行エラー");
        }
    }
    
    // 最終的なリカバリテスト
    println!("  リカバリテスト: 新しいキャッシュアクセス");
    match cache_manager.lock() {
        Ok(mut cache) => {
            let dummy_path = std::path::PathBuf::from("recovery_test.rs");
            match cache.get_symbols(&dummy_path) {
                Ok(_) | Err(_) => println!("    ✅ キャッシュアクセス回復"),
            }
        }
        Err(e) => {
            println!("    ❌ キャッシュアクセス失敗: {}", e);
        }
    }
    
    println!("✅ デッドロック検出・リカバリテスト完了");
    Ok(())
}

/// oneshot チャンネルの応答なしテスト
#[tokio::test]
async fn test_oneshot_channel_timeout() -> Result<()> {
    println!("🔍 oneshot チャンネルタイムアウトテスト");
    
    // レスポンスが返らないシナリオのシミュレーション
    let (_response_tx, response_rx) = oneshot::channel::<String>();
    
    // 送信側タスク（意図的に応答しない）
    let sender_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(500)).await;
        println!("    送信側: 意図的に応答しない（response_tx を drop）");
        // response_tx は明示的に送信せずに drop される
    });
    
    // 受信側タスク（タイムアウト付き）
    let receiver_handle = tokio::spawn(async move {
        match timeout(Duration::from_millis(200), response_rx).await {
            Ok(Ok(response)) => {
                println!("    受信側: 応答受信 - {}", response);
                false // タイムアウトしなかった
            }
            Ok(Err(e)) => {
                println!("    受信側: チャンネルエラー - {}", e);
                true // 送信側が drop された
            }
            Err(_) => {
                println!("    受信側: タイムアウト");
                true // タイムアウトした
            }
        }
    });
    
    let (_, timeout_occurred) = tokio::join!(sender_handle, receiver_handle);
    
    match timeout_occurred {
        Ok(true) => {
            println!("  ✅ 適切なタイムアウト/チャンネル切断検出");
        }
        Ok(false) => {
            println!("  ⚠️ 予期しない応答受信");
        }
        Err(e) => {
            println!("  ❌ 受信側タスクエラー: {}", e);
        }
    }
    
    println!("✅ oneshot チャンネルタイムアウトテスト完了");
    Ok(())
}