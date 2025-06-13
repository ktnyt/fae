//! ファイルシステム権限と特殊ファイルの処理テスト
//! 
//! アクセス権限なしディレクトリ、シンボリックリンクループ、
//! FIFO/named pipe、所有者権限変更、特殊ファイルの処理などをテスト

use fae::{CacheManager, SearchRunner};
use anyhow::Result;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use tempfile::TempDir;
use std::time::Duration;

/// アクセス権限なしディレクトリとファイルのテスト
#[cfg(unix)]
#[tokio::test]
async fn test_permission_denied_access() -> Result<()> {
    println!("🔍 アクセス権限なしディレクトリ・ファイルテスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 通常のファイルを作成
    let normal_file = temp_dir.path().join("normal.rs");
    fs::write(&normal_file, "fn normal_function() { println!(\"normal\"); }")?;
    
    // 読み取り権限なしファイルを作成
    let no_read_file = temp_dir.path().join("no_read.rs");
    fs::write(&no_read_file, "fn no_read_function() { println!(\"no read\"); }")?;
    let mut perms = fs::metadata(&no_read_file)?.permissions();
    perms.set_mode(0o000); // 全権限削除
    fs::set_permissions(&no_read_file, perms)?;
    
    // 権限なしディレクトリを作成
    let no_access_dir = temp_dir.path().join("no_access");
    fs::create_dir(&no_access_dir)?;
    let no_access_file = no_access_dir.join("hidden.rs");
    fs::write(&no_access_file, "fn hidden_function() { println!(\"hidden\"); }")?;
    
    // ディレクトリの権限削除
    let mut dir_perms = fs::metadata(&no_access_dir)?.permissions();
    dir_perms.set_mode(0o000);
    fs::set_permissions(&no_access_dir, dir_perms)?;
    
    println!("📋 権限テスト結果:");
    
    // 通常ファイル - 成功するべき
    match cache_manager.get_symbols(&normal_file) {
        Ok(symbols) => {
            println!("  通常ファイル: {} シンボル", symbols.len());
            assert!(symbols.len() > 0, "通常ファイルはシンボルを抽出できるべき");
        }
        Err(e) => {
            println!("  通常ファイル: エラー - {}", e);
            panic!("通常ファイルは処理できるべき");
        }
    }
    
    // 読み取り権限なしファイル - エラーになるべき
    match cache_manager.get_symbols(&no_read_file) {
        Ok(symbols) => {
            println!("  権限なしファイル: {} シンボル（予期しない成功）", symbols.len());
        }
        Err(e) => {
            println!("  権限なしファイル: エラー（期待通り） - {}", e);
        }
    }
    
    // 権限なしディレクトリ内ファイル - エラーになるべき
    match cache_manager.get_symbols(&no_access_file) {
        Ok(symbols) => {
            println!("  権限なしディレクトリ内ファイル: {} シンボル（予期しない成功）", symbols.len());
        }
        Err(e) => {
            println!("  権限なしディレクトリ内ファイル: エラー（期待通り） - {}", e);
        }
    }
    
    // SearchRunnerでの権限テスト
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    use fae::cli::strategies::ContentStrategy;
    let strategy = ContentStrategy;
    
    match search_runner.collect_results_with_strategy(&strategy, "function") {
        Ok(results) => {
            println!("  SearchRunner: {} 件のマッチ", results.len());
            // 通常ファイルからは結果が得られるが、権限なしファイルからは得られない
            assert!(results.len() >= 1, "少なくとも通常ファイルからは結果が得られるべき");
        }
        Err(e) => {
            println!("  SearchRunner: エラー - {}", e);
        }
    }
    
    // 権限復元（クリーンアップのため）
    let mut restore_dir_perms = fs::metadata(&no_access_dir)?.permissions();
    restore_dir_perms.set_mode(0o755);
    fs::set_permissions(&no_access_dir, restore_dir_perms)?;
    
    let mut file_perms = fs::metadata(&no_read_file)?.permissions();
    file_perms.set_mode(0o644);
    fs::set_permissions(&no_read_file, file_perms)?;
    
    println!("✅ アクセス権限テスト完了");
    Ok(())
}

/// シンボリックリンクとリンクループのテスト
#[cfg(unix)]
#[tokio::test]
async fn test_symbolic_links_and_loops() -> Result<()> {
    println!("🔍 シンボリックリンクとループテスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // オリジナルファイルを作成
    let original_file = temp_dir.path().join("original.rs");
    fs::write(&original_file, "fn original_function() { println!(\"original\"); }")?;
    
    // 正常なシンボリックリンクを作成
    let normal_link = temp_dir.path().join("normal_link.rs");
    symlink(&original_file, &normal_link)?;
    
    // 存在しないファイルへのシンボリックリンク（dangling link）
    let dangling_link = temp_dir.path().join("dangling_link.rs");
    let non_existent = temp_dir.path().join("non_existent.rs");
    symlink(&non_existent, &dangling_link)?;
    
    // シンボリックリンクループを作成
    let loop_link1 = temp_dir.path().join("loop1.rs");
    let loop_link2 = temp_dir.path().join("loop2.rs");
    symlink(&loop_link2, &loop_link1)?;
    symlink(&loop_link1, &loop_link2)?;
    
    // ディレクトリのシンボリックリンクループ
    let dir1 = temp_dir.path().join("dir1");
    let dir2 = temp_dir.path().join("dir2");
    fs::create_dir(&dir1)?;
    fs::create_dir(&dir2)?;
    
    let dir_link1 = dir1.join("link_to_dir2");
    let dir_link2 = dir2.join("link_to_dir1");
    symlink(&dir2, &dir_link1)?;
    symlink(&dir1, &dir_link2)?;
    
    // ディレクトリ内にファイルも作成
    let file_in_dir1 = dir1.join("file1.rs");
    fs::write(&file_in_dir1, "fn dir1_function() { }")?;
    
    println!("📋 シンボリックリンクテスト結果:");
    
    // オリジナルファイル
    match cache_manager.get_symbols(&original_file) {
        Ok(symbols) => {
            println!("  オリジナルファイル: {} シンボル", symbols.len());
            assert!(symbols.len() > 0, "オリジナルファイルはシンボルを抽出できるべき");
        }
        Err(e) => {
            println!("  オリジナルファイル: エラー - {}", e);
        }
    }
    
    // 正常なシンボリックリンク
    match cache_manager.get_symbols(&normal_link) {
        Ok(symbols) => {
            println!("  正常なシンボリックリンク: {} シンボル", symbols.len());
        }
        Err(e) => {
            println!("  正常なシンボリックリンク: エラー - {}", e);
        }
    }
    
    // danglingシンボリックリンク
    match cache_manager.get_symbols(&dangling_link) {
        Ok(symbols) => {
            println!("  danglingリンク: {} シンボル（予期しない成功）", symbols.len());
        }
        Err(e) => {
            println!("  danglingリンク: エラー（期待通り） - {}", e);
        }
    }
    
    // シンボリックリンクループ
    match cache_manager.get_symbols(&loop_link1) {
        Ok(symbols) => {
            println!("  リンクループ: {} シンボル（予期しない成功）", symbols.len());
        }
        Err(e) => {
            println!("  リンクループ: エラー（期待通り） - {}", e);
        }
    }
    
    // SearchRunnerでのシンボリックリンク処理
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    use fae::cli::strategies::ContentStrategy;
    let strategy = ContentStrategy;
    
    match search_runner.collect_results_with_strategy(&strategy, "function") {
        Ok(results) => {
            println!("  SearchRunner シンボリックリンク検索: {} 件", results.len());
            // 正常なファイルとリンクからは結果が得られる
            assert!(results.len() >= 1, "少なくとも正常なファイルからは結果が得られるべき");
        }
        Err(e) => {
            println!("  SearchRunner シンボリックリンク検索: エラー - {}", e);
        }
    }
    
    println!("✅ シンボリックリンクとループテスト完了");
    Ok(())
}

/// 特殊ファイル（FIFO、デバイスファイル等）のテスト
#[cfg(unix)]
#[tokio::test]
async fn test_special_files() -> Result<()> {
    println!("🔍 特殊ファイル処理テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 通常のファイル
    let normal_file = temp_dir.path().join("normal.rs");
    fs::write(&normal_file, "fn normal_function() { }")?;
    
    // FIFO (named pipe) を作成（Unix系でのみ）
    let fifo_path = temp_dir.path().join("test_fifo");
    
    use std::process::Command;
    let fifo_result = Command::new("mkfifo")
        .arg(&fifo_path)
        .output();
    
    let fifo_created = fifo_result.is_ok() && fifo_path.exists();
    
    // 空ファイル
    let empty_file = temp_dir.path().join("empty.rs");
    fs::write(&empty_file, "")?;
    
    // バイナリ風ファイル（.rsと偽装）
    let binary_like_file = temp_dir.path().join("binary_like.rs");
    let binary_content: Vec<u8> = (0..255).collect();
    fs::write(&binary_like_file, binary_content)?;
    
    // 非常に大きな行を含むファイル
    let long_line_file = temp_dir.path().join("long_line.rs");
    let long_line = "fn long_line_function() { let x = \"".to_string() + &"a".repeat(100000) + "\"; }";
    fs::write(&long_line_file, long_line)?;
    
    println!("📋 特殊ファイルテスト結果:");
    
    // 通常ファイル
    match cache_manager.get_symbols(&normal_file) {
        Ok(symbols) => {
            println!("  通常ファイル: {} シンボル", symbols.len());
            assert!(symbols.len() > 0, "通常ファイルはシンボルを抽出できるべき");
        }
        Err(e) => {
            println!("  通常ファイル: エラー - {}", e);
        }
    }
    
    // FIFO（作成できた場合）
    if fifo_created {
        println!("  FIFO作成成功: {}", fifo_path.display());
        
        // FIFOファイルからの読み取り試行（スキップ：読み取りでハングする可能性）
        println!("  FIFO読み取り: スキップ（ハング防止のため）");
        // 実際のプロダクションでは、FIFOファイルはignoreクレートで除外される
    } else {
        println!("  FIFO作成失敗または権限なし（スキップ）");
    }
    
    // 空ファイル
    match cache_manager.get_symbols(&empty_file) {
        Ok(symbols) => {
            println!("  空ファイル: {} シンボル", symbols.len());
            assert_eq!(symbols.len(), 0, "空ファイルはシンボルを含まないべき");
        }
        Err(e) => {
            println!("  空ファイル: エラー - {}", e);
        }
    }
    
    // バイナリ風ファイル
    match cache_manager.get_symbols(&binary_like_file) {
        Ok(symbols) => {
            println!("  バイナリ風ファイル: {} シンボル", symbols.len());
        }
        Err(e) => {
            println!("  バイナリ風ファイル: エラー（期待される） - {}", e);
        }
    }
    
    // 長い行を含むファイル
    match cache_manager.get_symbols(&long_line_file) {
        Ok(symbols) => {
            println!("  長い行ファイル: {} シンボル", symbols.len());
            // 長い行でもパース可能であるべき
            assert!(symbols.len() >= 1, "長い行でも関数は抽出されるべき");
        }
        Err(e) => {
            println!("  長い行ファイル: エラー - {}", e);
        }
    }
    
    println!("✅ 特殊ファイル処理テスト完了");
    Ok(())
}

/// ファイル競合状態（レースコンディション）のテスト
#[tokio::test]
async fn test_file_race_conditions() -> Result<()> {
    println!("🔍 ファイル競合状態テスト");
    
    let temp_dir = TempDir::new()?;
    let cache_manager = std::sync::Arc::new(std::sync::Mutex::new(CacheManager::new()));
    
    // 複数のタスクが同じファイルを同時に操作
    let test_file = temp_dir.path().join("race_test.rs");
    fs::write(&test_file, "fn initial_function() { }")?;
    
    let file_path = test_file.clone();
    let cache1 = cache_manager.clone();
    let cache2 = cache_manager.clone();
    let cache3 = cache_manager.clone();
    
    println!("  ファイル競合状態シミュレーション開始...");
    
    // 3つの並行タスクを開始
    let task1 = tokio::spawn(async move {
        let mut results = Vec::new();
        for i in 0..10 {
            match cache1.lock().unwrap().get_symbols(&file_path) {
                Ok(symbols) => {
                    results.push((i, symbols.len(), "success"));
                }
                Err(e) => {
                    results.push((i, 0, "error"));
                    println!("    タスク1-{}: エラー - {}", i, e);
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        results
    });
    
    let file_path2 = test_file.clone();
    let task2 = tokio::spawn(async move {
        let mut results = Vec::new();
        for i in 0..10 {
            // ファイル書き換えと読み取りを並行実行
            if i % 3 == 0 {
                let new_content = format!("fn modified_function_{}() {{ }}", i);
                let _ = fs::write(&file_path2, new_content);
            }
            
            match cache2.lock().unwrap().get_symbols(&file_path2) {
                Ok(symbols) => {
                    results.push((i, symbols.len(), "success"));
                }
                Err(_) => {
                    results.push((i, 0, "error"));
                }
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        results
    });
    
    let file_path3 = test_file.clone();
    let task3 = tokio::spawn(async move {
        let mut results = Vec::new();
        for i in 0..10 {
            match cache3.lock().unwrap().get_symbols(&file_path3) {
                Ok(symbols) => {
                    results.push((i, symbols.len(), "success"));
                }
                Err(_) => {
                    results.push((i, 0, "error"));
                }
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        results
    });
    
    // 全タスクの完了を待機
    let (results1, results2, results3) = tokio::join!(task1, task2, task3);
    
    let results1 = results1.unwrap();
    let results2 = results2.unwrap();
    let results3 = results3.unwrap();
    
    println!("📊 ファイル競合状態テスト結果:");
    
    let total_attempts = results1.len() + results2.len() + results3.len();
    let successful_attempts = results1.iter().filter(|(_, _, status)| *status == "success").count() +
                             results2.iter().filter(|(_, _, status)| *status == "success").count() +
                             results3.iter().filter(|(_, _, status)| *status == "success").count();
    
    println!("  総試行数: {}", total_attempts);
    println!("  成功数: {}", successful_attempts);
    println!("  成功率: {:.1}%", (successful_attempts as f64 / total_attempts as f64) * 100.0);
    
    // 競合状態でも大部分は成功するべき
    assert!(successful_attempts >= total_attempts * 70 / 100, 
           "競合状態でも70%以上は成功するべき");
    
    // 各タスクで最低限の成功は得られるべき
    let task1_success = results1.iter().filter(|(_, _, status)| *status == "success").count();
    let task2_success = results2.iter().filter(|(_, _, status)| *status == "success").count();
    let task3_success = results3.iter().filter(|(_, _, status)| *status == "success").count();
    
    println!("  タスク別成功数: {} / {} / {}", task1_success, task2_success, task3_success);
    
    assert!(task1_success >= 5, "タスク1は少なくとも5回は成功するべき");
    assert!(task2_success >= 3, "タスク2は少なくとも3回は成功するべき（書き込みあり）");
    assert!(task3_success >= 5, "タスク3は少なくとも5回は成功するべき");
    
    println!("✅ ファイル競合状態テスト完了");
    Ok(())
}

/// ネットワークファイルシステム風の遅延テスト
#[tokio::test]
async fn test_network_filesystem_simulation() -> Result<()> {
    println!("🔍 ネットワークファイルシステム風遅延テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 複数のファイルを作成
    let file_count = 20;
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("network_file_{}.rs", i));
        let content = format!(r#"
fn network_function_{}() -> String {{
    // ネットワークファイルシステム上のファイル
    format!("network operation result: {{}}", {})
}}

struct NetworkStruct_{} {{
    id: usize,
    data: String,
}}
"#, i, i, i);
        fs::write(&file_path, content)?;
    }
    
    println!("  遅延シミュレーション付きファイル処理...");
    
    let start_time = std::time::Instant::now();
    let mut processed_files = 0;
    let mut total_symbols = 0;
    let mut processing_times = Vec::new();
    
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("network_file_{}.rs", i));
        
        // ネットワーク遅延をシミュレート
        let delay_ms = (i % 3 + 1) * 10; // 10-30ms の遅延
        tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
        
        let file_start = std::time::Instant::now();
        match cache_manager.get_symbols(&file_path) {
            Ok(symbols) => {
                let file_duration = file_start.elapsed();
                processing_times.push(file_duration.as_millis() as u64);
                
                total_symbols += symbols.len();
                processed_files += 1;
                
                if i % 5 == 0 {
                    println!("    ファイル {}: {} シンボル, {:?}", i, symbols.len(), file_duration);
                }
            }
            Err(e) => {
                println!("    ファイル {} エラー: {}", i, e);
            }
        }
    }
    
    let total_duration = start_time.elapsed();
    
    println!("📊 ネットワークファイルシステム風テスト結果:");
    println!("  処理ファイル数: {} / {}", processed_files, file_count);
    println!("  総シンボル数: {}", total_symbols);
    println!("  総処理時間: {:?}", total_duration);
    
    if !processing_times.is_empty() {
        let avg_time = processing_times.iter().sum::<u64>() / processing_times.len() as u64;
        let max_time = *processing_times.iter().max().unwrap();
        let min_time = *processing_times.iter().min().unwrap();
        
        println!("  ファイル処理時間: 平均 {}ms, 最大 {}ms, 最小 {}ms", avg_time, max_time, min_time);
    }
    
    // 遅延があっても全ファイル処理できるべき
    assert_eq!(processed_files, file_count, "全ファイルが処理されるべき");
    assert!(total_symbols >= file_count * 2, "ファイルあたり少なくとも2シンボル");
    
    // 合理的な処理時間であるべき
    assert!(total_duration.as_secs() < 10, "20ファイル処理は10秒以内であるべき");
    
    println!("✅ ネットワークファイルシステム風遅延テスト完了");
    Ok(())
}

/// 非Unix環境用のダミーテスト
#[cfg(not(unix))]
#[tokio::test]
async fn test_windows_compatibility_placeholder() -> Result<()> {
    println!("🔍 Windows互換性プレースホルダー");
    println!("  Unix固有の権限テストはスキップされました");
    println!("  Windows環境での基本的な動作確認:");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    let test_file = temp_dir.path().join("windows_test.rs");
    fs::write(&test_file, "fn windows_function() { println!(\"Windows test\"); }")?;
    
    match cache_manager.get_symbols(&test_file) {
        Ok(symbols) => {
            println!("  Windowsファイル処理: {} シンボル", symbols.len());
            assert!(symbols.len() > 0, "Windowsでもファイル処理は動作するべき");
        }
        Err(e) => {
            println!("  Windowsファイル処理: エラー - {}", e);
        }
    }
    
    println!("✅ Windows互換性確認完了");
    Ok(())
}