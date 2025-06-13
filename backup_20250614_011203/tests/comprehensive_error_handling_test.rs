//! 包括的エラーハンドリングテスト
//! 
//! 各モジュールのエラーハンドリング、例外ケース、境界値テストの実装

use fae::{
    RealtimeIndexer, CacheManager, SearchRunner, SymbolIndex, SymbolMetadata,
    types::SymbolType,
    cli::strategies::{SymbolStrategy, RegexStrategy},
};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

/// 不正なファイルパスのハンドリングテスト
#[tokio::test]
async fn test_invalid_file_paths() -> Result<()> {
    println!("🔍 不正ファイルパス処理テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    let mut cache_manager = CacheManager::new();
    
    // 存在しないファイル
    let nonexistent_path = temp_path.join("does_not_exist.rs");
    let result = cache_manager.get_symbols(&nonexistent_path);
    assert!(result.is_err(), "存在しないファイルはエラーを返すべき");
    println!("✅ 存在しないファイルのエラーハンドリング正常");
    
    // 無効なUTF-8ファイル名（バイト列から作成）
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let invalid_utf8_path = PathBuf::from(OsStr::from_bytes(b"\xff\xfe\x00invalid.rs"));
        let result = cache_manager.get_symbols(&invalid_utf8_path);
        assert!(result.is_err(), "無効なUTF-8パスはエラーを返すべき");
        println!("✅ 無効なUTF-8パスのエラーハンドリング正常");
    }
    
    // 権限なしディレクトリ（可能な場合）
    let readonly_dir = temp_path.join("readonly");
    fs::create_dir(&readonly_dir)?;
    
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        
        // 読み取り専用権限に設定
        let readonly_perms = Permissions::from_mode(0o000);
        let _ = fs::set_permissions(&readonly_dir, readonly_perms);
        
        let readonly_file = readonly_dir.join("protected.rs");
        let result = cache_manager.get_symbols(&readonly_file);
        // 権限エラーまたはファイル非存在エラーが発生するはず
        assert!(result.is_err(), "権限なしファイルはエラーを返すべき");
        
        // 後始末：権限を戻す
        let normal_perms = Permissions::from_mode(0o755);
        let _ = fs::set_permissions(&readonly_dir, normal_perms);
        println!("✅ 権限制限ファイルのエラーハンドリング正常");
    }
    
    Ok(())
}

/// 巨大ファイルの処理テスト
#[tokio::test]
async fn test_large_file_handling() -> Result<()> {
    println!("🔍 巨大ファイル処理テスト");
    
    let temp_dir = TempDir::new()?;
    let large_file = temp_dir.path().join("huge.rs");
    
    // 100KBのファイルを作成（中程度のサイズ）
    let large_content = "fn large_function() {\n    println!(\"test\");\n}\n".repeat(2000);
    fs::write(&large_file, &large_content)?;
    
    let mut cache_manager = CacheManager::new();
    let result = cache_manager.get_symbols(&large_file);
    
    match result {
        Ok(symbols) => {
            println!("大きなファイルから {} シンボルを抽出", symbols.len());
            assert!(symbols.len() > 1000, "期待されるシンボル数");
        }
        Err(e) => {
            println!("大きなファイル処理エラー（期待される場合もある）: {}", e);
        }
    }
    
    println!("✅ 巨大ファイル処理テスト完了");
    Ok(())
}

/// 破損したファイル内容のテスト
#[tokio::test]
async fn test_corrupted_file_content() -> Result<()> {
    println!("🔍 破損ファイル内容テスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    // 無効なRustシンタックス
    let invalid_rust = temp_dir.path().join("invalid.rs");
    fs::write(&invalid_rust, "fn incomplete_function( { // 不完全な構文")?;
    
    let result = cache_manager.get_symbols(&invalid_rust);
    // Tree-sitterは耐性があるので、エラーかもしれないし、部分的に解析するかもしれない
    println!("無効Rust構文処理結果: {:?}", result.is_ok());
    
    // バイナリファイル（偽装）
    let binary_like = temp_dir.path().join("fake_binary.rs");
    let binary_content: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    fs::write(&binary_like, binary_content)?;
    
    let result = cache_manager.get_symbols(&binary_like);
    println!("バイナリ風ファイル処理結果: {:?}", result.is_ok());
    
    // 空ファイル
    let empty_file = temp_dir.path().join("empty.rs");
    fs::write(&empty_file, "")?;
    
    let result = cache_manager.get_symbols(&empty_file);
    assert!(result.is_ok(), "空ファイルは成功すべき");
    let symbols = result.unwrap();
    assert_eq!(symbols.len(), 0, "空ファイルはシンボルなし");
    
    println!("✅ 破損ファイル内容テスト完了");
    Ok(())
}

/// 同時実行時の競合テスト
#[tokio::test]
async fn test_concurrent_access() -> Result<()> {
    println!("🔍 同時実行競合テスト");
    
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("concurrent.rs");
    
    fs::write(&test_file, r#"
fn function_1() { println!("1"); }
fn function_2() { println!("2"); }
fn function_3() { println!("3"); }
"#)?;
    
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // 複数タスクで同時にアクセス
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let cm = cache_manager.clone();
        let file_path = test_file.clone();
        
        let handle = tokio::spawn(async move {
            let result = {
                let mut cache = cm.lock().unwrap();
                cache.get_symbols(&file_path)
            };
            (i, result.is_ok())
        });
        
        handles.push(handle);
    }
    
    // 全タスクの完了を待機
    let mut success_count = 0;
    for handle in handles {
        let (task_id, success) = handle.await?;
        if success {
            success_count += 1;
        }
        println!("タスク {} 結果: {}", task_id, if success { "成功" } else { "失敗" });
    }
    
    assert!(success_count >= 8, "大部分のタスクが成功すべき");
    println!("✅ 同時実行テスト完了: {}/10 タスク成功", success_count);
    
    Ok(())
}

/// SearchRunnerのエラーハンドリングテスト
#[tokio::test]
async fn test_search_runner_error_cases() -> Result<()> {
    println!("🔍 SearchRunnerエラーケーステスト");
    
    let temp_dir = TempDir::new()?;
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // 空クエリ
    let symbol_strategy = SymbolStrategy::new();
    let empty_results = search_runner.collect_results_with_strategy(&symbol_strategy, "")?;
    println!("空クエリ結果: {} 件", empty_results.len());
    
    // 非常に長いクエリ
    let very_long_query = "a".repeat(10000);
    let long_results = search_runner.collect_results_with_strategy(&symbol_strategy, &very_long_query)?;
    println!("超長クエリ結果: {} 件", long_results.len());
    
    // 特殊文字クエリ
    let special_chars = "!@#$%^&*()_+-=[]{}|;':\",./<>?`~";
    let special_results = search_runner.collect_results_with_strategy(&symbol_strategy, special_chars)?;
    println!("特殊文字クエリ結果: {} 件", special_results.len());
    
    // 無効な正規表現（RegexStrategy）
    let regex_strategy = RegexStrategy;
    let invalid_regex = "[invalid regex";
    match search_runner.collect_results_with_strategy(&regex_strategy, invalid_regex) {
        Ok(results) => println!("無効正規表現が意外に成功: {} 件", results.len()),
        Err(e) => println!("無効正規表現エラー（期待通り）: {}", e),
    }
    
    println!("✅ SearchRunnerエラーケーステスト完了");
    Ok(())
}

/// RealtimeIndexerのエラーハンドリングテスト
#[tokio::test]
async fn test_realtime_indexer_error_cases() -> Result<()> {
    println!("🔍 RealtimeIndexerエラーケーステスト");
    
    let temp_dir = TempDir::new()?;
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    
    // 存在しないディレクトリでの初期化
    let nonexistent_dir = temp_dir.path().join("does_not_exist");
    match RealtimeIndexer::new(nonexistent_dir, cache_manager.clone()) {
        Ok(_) => println!("⚠️ 存在しないディレクトリでも初期化が成功"),
        Err(e) => println!("✅ 存在しないディレクトリで期待通りエラー: {}", e),
    }
    
    // 権限なしディレクトリ（Unix系のみ）
    #[cfg(unix)]
    {
        let readonly_dir = temp_dir.path().join("no_permission");
        fs::create_dir(&readonly_dir)?;
        
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        let readonly_perms = Permissions::from_mode(0o000);
        fs::set_permissions(&readonly_dir, readonly_perms)?;
        
        match RealtimeIndexer::new(readonly_dir.clone(), cache_manager.clone()) {
            Ok(_) => println!("⚠️ 権限なしディレクトリでも初期化が成功"),
            Err(e) => println!("✅ 権限なしディレクトリで期待通りエラー: {}", e),
        }
        
        // 後始末
        let normal_perms = Permissions::from_mode(0o755);
        let _ = fs::set_permissions(&readonly_dir, normal_perms);
    }
    
    println!("✅ RealtimeIndexerエラーケーステスト完了");
    Ok(())
}

/// SymbolIndexの境界値テスト
#[tokio::test]
async fn test_symbol_index_boundary_cases() -> Result<()> {
    println!("🔍 SymbolIndex境界値テスト");
    
    // 空のSymbolIndex
    let empty_index = SymbolIndex::from_symbols(vec![]);
    assert_eq!(empty_index.len(), 0);
    
    let empty_search = empty_index.fuzzy_search("anything", 10);
    assert_eq!(empty_search.len(), 0);
    println!("✅ 空インデックスの処理正常");
    
    // 大量のシンボル（メモリ使用量テスト）
    let large_symbols: Vec<SymbolMetadata> = (0..10000)
        .map(|i| SymbolMetadata {
            name: format!("function_{}", i),
            file_path: PathBuf::from(format!("file_{}.rs", i % 100)),
            line: (i % 1000) + 1,
            column: 1,
            symbol_type: SymbolType::Function,
        })
        .collect();
    
    println!("大量シンボル作成: {} 個", large_symbols.len());
    let large_index = SymbolIndex::from_symbols(large_symbols);
    
    // ファジー検索のパフォーマンステスト
    let start = std::time::Instant::now();
    let search_results = large_index.fuzzy_search("function_", 100);
    let duration = start.elapsed();
    
    println!("大量データ検索: {} 件を {:?} で取得", search_results.len(), duration);
    assert!(search_results.len() > 0, "検索結果が見つかるべき");
    assert!(duration.as_millis() < 1000, "検索は1秒未満であるべき");
    
    println!("✅ SymbolIndex境界値テスト完了");
    Ok(())
}

/// メモリ使用量テスト
#[tokio::test]
async fn test_memory_usage_patterns() -> Result<()> {
    println!("🔍 メモリ使用量パターンテスト");
    
    let temp_dir = TempDir::new()?;
    
    // 多数の小さなファイルを作成
    for i in 0..100 {
        let file_path = temp_dir.path().join(format!("small_{}.rs", i));
        fs::write(&file_path, format!("fn small_function_{}() {{}}", i))?;
    }
    
    let mut cache_manager = CacheManager::new();
    let initial_memory = get_memory_usage();
    
    // 全ファイルをキャッシュに読み込み
    for i in 0..100 {
        let file_path = temp_dir.path().join(format!("small_{}.rs", i));
        let _ = cache_manager.get_symbols(&file_path);
    }
    
    let after_load_memory = get_memory_usage();
    println!("メモリ使用量: 初期 {}KB → 読み込み後 {}KB", 
             initial_memory / 1024, after_load_memory / 1024);
    
    // ファジー検索を多数回実行
    for _ in 0..1000 {
        let _ = cache_manager.fuzzy_search_symbols("function", 10);
    }
    
    let after_search_memory = get_memory_usage();
    println!("多数検索後: {}KB", after_search_memory / 1024);
    
    // メモリ使用量が異常に増加していないかチェック
    let memory_increase = after_search_memory.saturating_sub(initial_memory);
    assert!(memory_increase < 100 * 1024 * 1024, "メモリ使用量が100MB未満であるべき");
    
    println!("✅ メモリ使用量テスト完了");
    Ok(())
}

/// 粗い方法でメモリ使用量を取得（クロスプラットフォーム対応）
fn get_memory_usage() -> usize {
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
    
    #[cfg(not(target_os = "linux"))]
    {
        // 他のプラットフォームでは概算値を返す
        0
    }
}

/// 統合エラーハンドリングテスト
#[tokio::test]
async fn test_integrated_error_scenarios() -> Result<()> {
    println!("🔍 統合エラーハンドリングテスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // 複雑なディレクトリ構造
    fs::create_dir_all(temp_path.join("deep/nested/dirs"))?;
    
    // 様々な問題のあるファイル
    let huge_content = "x".repeat(100000);
    let problems = vec![
        ("empty.rs", ""),
        ("malformed.rs", "fn incomplete("),
        ("huge_line.rs", &huge_content),
        ("mixed_content.rs", "fn valid() {}\n/* partial comment\n fn also_valid() {}"),
    ];
    
    let mut cache_manager = CacheManager::new();
    let mut success_count = 0;
    let mut error_count = 0;
    
    for (filename, content) in problems {
        let file_path = temp_path.join(filename);
        
        if let Err(_) = fs::write(&file_path, content) {
            println!("⚠️ ファイル作成失敗: {}", filename);
            continue;
        }
        
        match cache_manager.get_symbols(&file_path) {
            Ok(symbols) => {
                success_count += 1;
                println!("✅ {} 処理成功: {} シンボル", filename, symbols.len());
            }
            Err(e) => {
                error_count += 1;
                println!("❌ {} 処理エラー: {}", filename, e);
            }
        }
    }
    
    println!("統合テスト結果: 成功 {}, エラー {}", success_count, error_count);
    
    // 少なくとも一部は成功すべき
    assert!(success_count > 0, "一部のファイルは正常に処理されるべき");
    
    println!("✅ 統合エラーハンドリングテスト完了");
    Ok(())
}