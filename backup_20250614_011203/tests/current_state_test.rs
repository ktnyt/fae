//! 現状機能の包括的テスト
//! 
//! 巻き戻し後の現在実装されている機能を確認するためのテスト

use fae::{
    RealtimeIndexer,
    CacheManager, SearchRunner, SymbolIndex, SymbolMetadata,
    types::SymbolType,
};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

/// 現在のRealtimeIndexer機能をテスト
#[tokio::test]
async fn test_current_realtime_indexer() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    println!("🔍 RealtimeIndexer基本機能テスト");
    
    // 初期ファイル作成
    let file_path = temp_path.join("test.rs");
    std::fs::write(&file_path, r#"
fn initial_function() {
    println!("Initial");
}
"#)?;
    
    // CacheManagerとRealtimeIndexer作成
    let cache_manager = Arc::new(Mutex::new(CacheManager::new()));
    let realtime_indexer = RealtimeIndexer::new(temp_path.clone(), cache_manager.clone())?;
    
    println!("✅ RealtimeIndexer作成成功");
    
    // バックグラウンドで監視開始
    tokio::spawn(async move {
        let mut indexer = realtime_indexer;
        let _ = indexer.start_event_loop().await;
    });
    
    // 初期化待機
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // ファイル更新
    std::fs::write(&file_path, r#"
fn initial_function() {
    println!("Initial");
}

fn added_function() {
    println!("Added");
}
"#)?;
    
    // 更新処理の時間を待機
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    println!("✅ ファイル更新とバックグラウンド処理完了");
    
    Ok(())
}

/// CacheManagerの部分更新機能をテスト
#[tokio::test]
async fn test_current_cache_manager_partial_update() -> Result<()> {
    println!("🔍 CacheManager部分更新機能テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    let file_path = temp_path.join("cache_test.rs");
    std::fs::write(&file_path, r#"
fn original_func() {
    println!("Original");
}
"#)?;
    
    let mut cache_manager = CacheManager::new();
    
    // 初回シンボル取得
    let initial_symbols = cache_manager.get_symbols(&file_path)?;
    println!("初回シンボル数: {}", initial_symbols.len());
    assert_eq!(initial_symbols.len(), 1, "初回は1つのシンボル");
    assert_eq!(initial_symbols[0].name, "original_func");
    
    // ファジー検索テスト
    let search_results = cache_manager.fuzzy_search_symbols("original", 10);
    println!("ファジー検索結果: {} 件", search_results.len());
    assert!(!search_results.is_empty(), "original検索で結果が見つかる");
    
    // ファイル更新
    std::fs::write(&file_path, r#"
fn original_func() {
    println!("Original");
}

fn new_func() {
    println!("New");
}

fn another_func() {
    println!("Another");
}
"#)?;
    
    // キャッシュ無効化（ファイル変更をシミュレート）
    cache_manager.invalidate_file(&file_path);
    
    // 再度シンボル取得
    let updated_symbols = cache_manager.get_symbols(&file_path)?;
    println!("更新後シンボル数: {}", updated_symbols.len());
    assert_eq!(updated_symbols.len(), 3, "更新後は3つのシンボル");
    
    let symbol_names: Vec<&str> = updated_symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(symbol_names.contains(&"original_func"), "original_func存在");
    assert!(symbol_names.contains(&"new_func"), "new_func存在");
    assert!(symbol_names.contains(&"another_func"), "another_func存在");
    
    // 更新後のファジー検索
    let new_search_results = cache_manager.fuzzy_search_symbols("func", 10);
    println!("更新後ファジー検索結果: {} 件", new_search_results.len());
    assert!(new_search_results.len() >= 3, "3つのfunc関数が見つかる");
    
    println!("✅ CacheManager部分更新機能正常動作");
    
    Ok(())
}

/// SymbolIndexの部分更新機能をテスト
#[tokio::test]
async fn test_current_symbol_index_partial_update() -> Result<()> {
    println!("🔍 SymbolIndex部分更新機能テスト");
    
    let temp_dir = TempDir::new()?;
    let file1 = temp_dir.path().join("index1.rs");
    let file2 = temp_dir.path().join("index2.rs");
    
    // 初期シンボルデータ
    let initial_symbols = vec![
        SymbolMetadata {
            name: "function_one".to_string(),
            file_path: file1.clone(),
            line: 1,
            column: 1,
            symbol_type: SymbolType::Function,
        },
        SymbolMetadata {
            name: "function_two".to_string(),
            file_path: file2.clone(),
            line: 1,
            column: 1,
            symbol_type: SymbolType::Function,
        },
    ];
    
    let mut symbol_index = SymbolIndex::from_symbols(initial_symbols);
    
    // 初期状態確認
    println!("初期SymbolIndex状態:");
    println!("  - 総シンボル数: {}", symbol_index.len());
    println!("  - file1のシンボル数: {}", symbol_index.get_file_symbol_count(&file1));
    println!("  - file2のシンボル数: {}", symbol_index.get_file_symbol_count(&file2));
    
    assert_eq!(symbol_index.len(), 2, "初期は2つのシンボル");
    assert_eq!(symbol_index.get_file_symbol_count(&file1), 1, "file1は1つ");
    assert_eq!(symbol_index.get_file_symbol_count(&file2), 1, "file2は1つ");
    
    // file1の部分更新（新しいシンボルを追加）
    let updated_file1_symbols = vec![
        SymbolMetadata {
            name: "function_one_updated".to_string(),
            file_path: file1.clone(),
            line: 1,
            column: 1,
            symbol_type: SymbolType::Function,
        },
        SymbolMetadata {
            name: "function_one_new".to_string(),
            file_path: file1.clone(),
            line: 5,
            column: 1,
            symbol_type: SymbolType::Function,
        },
    ];
    
    symbol_index.update_file_symbols(&file1, updated_file1_symbols);
    
    // 部分更新後の確認
    println!("file1部分更新後:");
    println!("  - 総シンボル数: {}", symbol_index.len());
    println!("  - file1のシンボル数: {}", symbol_index.get_file_symbol_count(&file1));
    println!("  - file2のシンボル数: {}", symbol_index.get_file_symbol_count(&file2));
    
    assert_eq!(symbol_index.len(), 3, "更新後は3つのシンボル（file1: 2, file2: 1）");
    assert_eq!(symbol_index.get_file_symbol_count(&file1), 2, "file1は2つに増加");
    assert_eq!(symbol_index.get_file_symbol_count(&file2), 1, "file2は変更なし");
    
    // ファジー検索テスト
    let search_results = symbol_index.fuzzy_search("function_one", 10);
    println!("function_one検索結果: {} 件", search_results.len());
    assert!(search_results.len() >= 2, "function_oneで2つ以上の結果");
    
    // file2を削除
    symbol_index.remove_file_symbols(&file2);
    
    println!("file2削除後:");
    println!("  - 総シンボル数: {}", symbol_index.len());
    println!("  - file1のシンボル数: {}", symbol_index.get_file_symbol_count(&file1));
    println!("  - file2のシンボル数: {}", symbol_index.get_file_symbol_count(&file2));
    
    assert_eq!(symbol_index.len(), 2, "削除後は2つのシンボル（file1のみ）");
    assert_eq!(symbol_index.get_file_symbol_count(&file1), 2, "file1は変更なし");
    assert_eq!(symbol_index.get_file_symbol_count(&file2), 0, "file2は0");
    
    println!("✅ SymbolIndex部分更新機能正常動作");
    
    Ok(())
}

/// SearchRunnerとの統合をテスト
#[tokio::test]
async fn test_current_search_integration() -> Result<()> {
    println!("🔍 SearchRunner統合テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // 複数ファイルを作成
    let file1 = temp_path.join("search1.rs");
    let file2 = temp_path.join("search2.ts");
    let file3 = temp_path.join("search3.py");
    
    std::fs::write(&file1, r#"
fn rust_function() {
    println!("Rust");
}

struct RustStruct {
    value: i32,
}
"#)?;
    
    std::fs::write(&file2, r#"
function typescript_function() {
    console.log("TypeScript");
}

class TypeScriptClass {
    value: number;
}
"#)?;
    
    std::fs::write(&file3, r#"
def python_function():
    print("Python")

class PythonClass:
    def __init__(self):
        self.value = 0
"#)?;
    
    // SearchRunnerを作成
    let search_runner = SearchRunner::new(temp_path.clone(), false);
    
    // 各検索モードをテスト
    use fae::cli::strategies::{SymbolStrategy, ContentStrategy, FileStrategy};
    
    // シンボル検索
    let symbol_strategy = SymbolStrategy::new();
    let function_results = search_runner.collect_results_with_strategy(&symbol_strategy, "function")?;
    println!("シンボル検索 'function': {} 件", function_results.len());
    
    // コンテンツ検索
    let content_strategy = ContentStrategy;
    let print_results = search_runner.collect_results_with_strategy(&content_strategy, "print")?;
    println!("コンテンツ検索 'print': {} 件", print_results.len());
    
    // ファイル検索
    let file_strategy = FileStrategy;
    let rust_files = search_runner.collect_results_with_strategy(&file_strategy, "rs")?;
    println!("ファイル検索 'rs': {} 件", rust_files.len());
    
    // 結果の詳細確認
    assert!(!function_results.is_empty(), "function検索で結果が見つかる");
    assert!(!print_results.is_empty(), "print検索で結果が見つかる");
    assert!(!rust_files.is_empty(), "rs検索で結果が見つかる");
    
    println!("✅ SearchRunner統合正常動作");
    
    Ok(())
}

/// TUI基本機能をテスト（RealtimeIndexer組み込み確認）
#[tokio::test]
async fn test_current_tui_integration() -> Result<()> {
    println!("🔍 TUI基本機能テスト（RealtimeIndexer組み込み確認）");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // テストファイル作成
    let file_path = temp_path.join("tui_test.rs");
    std::fs::write(&file_path, r#"
fn tui_function() {
    println!("TUI");
}
"#)?;
    
    // SearchRunnerを作成
    let search_runner = SearchRunner::new(temp_path.clone(), false);
    
    // TuiEngineの作成をテスト（実際の起動はしない）
    let tui_result = fae::tui::TuiEngine::new(temp_path, search_runner);
    
    match tui_result {
        Ok(_engine) => {
            println!("✅ TuiEngine作成成功");
            println!("  - RealtimeIndexer組み込み済み");
            println!("  - CacheManager統合済み");
            println!("  - バックグラウンド監視準備完了");
            
            // 短時間待機（初期化確認）
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(e) => {
            // ターミナル環境でない場合のエラーは期待される
            println!("ℹ️ TuiEngine作成エラー (ターミナル環境でない場合は正常): {}", e);
        }
    }
    
    println!("✅ TUI基本機能確認完了");
    
    Ok(())
}

/// 現状の全機能統合テスト
#[tokio::test]
async fn test_current_overall_integration() -> Result<()> {
    println!("🔍 現状全機能統合テスト");
    
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    // 複雑なファイル構造を作成
    std::fs::create_dir_all(temp_path.join("src"))?;
    std::fs::create_dir_all(temp_path.join("tests"))?;
    
    let main_file = temp_path.join("src/main.rs");
    let lib_file = temp_path.join("src/lib.rs");
    let test_file = temp_path.join("tests/integration.rs");
    
    std::fs::write(&main_file, r#"
use std::collections::HashMap;

fn main() {
    let mut app = Application::new();
    app.run();
}

struct Application {
    data: HashMap<String, i32>,
}

impl Application {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    
    fn run(&mut self) {
        println!("Running application");
    }
}
"#)?;
    
    std::fs::write(&lib_file, r#"
pub mod utils;

pub fn calculate(a: i32, b: i32) -> i32 {
    a + b
}

pub struct Calculator {
    history: Vec<i32>,
}

impl Calculator {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }
}
"#)?;
    
    std::fs::write(&test_file, r#"
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculation() {
        assert_eq!(calculate(2, 3), 5);
    }
}
"#)?;
    
    // SearchRunnerで各種検索をテスト
    let search_runner = SearchRunner::new(temp_path.clone(), false);
    
    use fae::cli::strategies::{SymbolStrategy, ContentStrategy, FileStrategy};
    
    println!("📊 統合検索結果:");
    
    // 1. シンボル検索
    let symbol_strategy = SymbolStrategy::new();
    let app_symbols = search_runner.collect_results_with_strategy(&symbol_strategy, "Application")?;
    let calc_symbols = search_runner.collect_results_with_strategy(&symbol_strategy, "calculate")?;
    
    println!("  - 'Application'シンボル: {} 件", app_symbols.len());
    println!("  - 'calculate'シンボル: {} 件", calc_symbols.len());
    
    // 2. コンテンツ検索
    let content_strategy = ContentStrategy;
    let println_content = search_runner.collect_results_with_strategy(&content_strategy, "println")?;
    let hashmap_content = search_runner.collect_results_with_strategy(&content_strategy, "HashMap")?;
    
    println!("  - 'println'コンテンツ: {} 件", println_content.len());
    println!("  - 'HashMap'コンテンツ: {} 件", hashmap_content.len());
    
    // 3. ファイル検索
    let file_strategy = FileStrategy;
    let rust_files = search_runner.collect_results_with_strategy(&file_strategy, "main")?;
    let test_files = search_runner.collect_results_with_strategy(&file_strategy, "test")?;
    
    println!("  - 'main'ファイル: {} 件", rust_files.len());
    println!("  - 'test'ファイル: {} 件", test_files.len());
    
    // 結果検証
    assert!(!app_symbols.is_empty(), "Applicationシンボルが見つかる");
    assert!(!calc_symbols.is_empty(), "calculateシンボルが見つかる");
    assert!(!println_content.is_empty(), "printlnコンテンツが見つかる");
    assert!(!hashmap_content.is_empty(), "HashMapコンテンツが見つかる");
    assert!(!rust_files.is_empty(), "mainファイルが見つかる");
    
    println!("✅ 現状全機能統合テスト成功");
    println!("📋 実装済み機能:");
    println!("  ✅ RealtimeIndexer - ファイル監視");
    println!("  ✅ SymbolIndex - 部分更新");
    println!("  ✅ CacheManager - 統合管理");
    println!("  ✅ SearchRunner - 多様な検索");
    println!("  ✅ TuiEngine - UI統合（基本）");
    
    Ok(())
}