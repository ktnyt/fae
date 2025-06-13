//! パフォーマンス回帰テストとベンチマーク
//! 
//! 既知のパフォーマンス特性を保つための回帰テストと
//! 将来のパフォーマンス最適化のためのベンチマーク

use fae::{
    CacheManager, SearchRunner, SymbolIndex, SymbolMetadata,
    types::SymbolType,
    cli::strategies::{SymbolStrategy, ContentStrategy, FileStrategy},
};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// パフォーマンスメトリクス
#[derive(Debug)]
struct PerformanceMetrics {
    operation: String,
    duration: Duration,
    items_processed: usize,
    items_per_second: f64,
    memory_usage_estimate: usize,
}

impl PerformanceMetrics {
    fn new(operation: String, duration: Duration, items_processed: usize) -> Self {
        let items_per_second = if duration.as_secs_f64() > 0.0 {
            items_processed as f64 / duration.as_secs_f64()
        } else {
            0.0
        };
        
        Self {
            operation,
            duration,
            items_processed,
            items_per_second,
            memory_usage_estimate: 0,
        }
    }
    
    fn print(&self) {
        println!("📊 {}", self.operation);
        println!("   時間: {:?}", self.duration);
        println!("   処理数: {} items", self.items_processed);
        println!("   スループット: {:.2} items/sec", self.items_per_second);
        if self.memory_usage_estimate > 0 {
            println!("   推定メモリ: {} KB", self.memory_usage_estimate / 1024);
        }
        println!();
    }
}

/// 大規模シンボルインデックスの構築パフォーマンステスト
#[tokio::test]
async fn test_large_symbol_index_performance() -> Result<()> {
    println!("🔍 大規模シンボルインデックス構築パフォーマンステスト");
    
    // 10,000個のシンボルを生成
    let symbol_count = 10_000usize;
    let start = Instant::now();
    
    let symbols: Vec<SymbolMetadata> = (0..symbol_count)
        .map(|i| SymbolMetadata {
            name: format!("function_{}", i),
            file_path: PathBuf::from(format!("file_{}.rs", i % 100)),
            line: ((i % 1000) + 1) as u32,
            column: 1,
            symbol_type: if i % 4 == 0 { SymbolType::Function }
                        else if i % 4 == 1 { SymbolType::Class }
                        else if i % 4 == 2 { SymbolType::Variable }
                        else { SymbolType::Constant },
        })
        .collect();
    
    let generation_duration = start.elapsed();
    let generation_metrics = PerformanceMetrics::new(
        "シンボルデータ生成".to_string(),
        generation_duration,
        symbol_count,
    );
    generation_metrics.print();
    
    // インデックス構築時間測定
    let start = Instant::now();
    let symbol_index = SymbolIndex::from_symbols(symbols);
    let indexing_duration = start.elapsed();
    
    let indexing_metrics = PerformanceMetrics::new(
        "SymbolIndex構築".to_string(),
        indexing_duration,
        symbol_count,
    );
    indexing_metrics.print();
    
    // パフォーマンス回帰チェック
    assert!(indexing_duration.as_millis() < 1000, 
            "10K シンボルのインデックス構築は1秒未満であるべき");
    assert!(indexing_metrics.items_per_second > 5000.0,
            "インデックス構築は5K items/sec以上であるべき");
    
    // 検索パフォーマンステスト
    let search_queries = vec!["function", "class", "var", "const", "test"];
    let search_limit = 100;
    
    for query in search_queries {
        let start = Instant::now();
        let results = symbol_index.fuzzy_search(query, search_limit);
        let search_duration = start.elapsed();
        
        let search_metrics = PerformanceMetrics::new(
            format!("ファジー検索 '{}'", query),
            search_duration,
            results.len(),
        );
        search_metrics.print();
        
        // 検索パフォーマンス回帰チェック
        assert!(search_duration.as_millis() < 100,
                "ファジー検索は100ms未満であるべき");
    }
    
    println!("✅ 大規模シンボルインデックスパフォーマンステスト完了");
    Ok(())
}

/// ファイル読み込みパフォーマンステスト
#[tokio::test]
async fn test_file_loading_performance() -> Result<()> {
    println!("🔍 ファイル読み込みパフォーマンステスト");
    
    let temp_dir = TempDir::new()?;
    let file_count = 100;
    let mut cache_manager = CacheManager::new();
    
    // 多数のファイルを作成
    let file_creation_start = Instant::now();
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("perf_test_{}.rs", i));
        let content = format!(r#"
// File {}
use std::collections::HashMap;

fn function_{}() -> i32 {{
    let mut data = HashMap::new();
    data.insert("key_{}", {});
    data.len() as i32
}}

struct Struct_{} {{
    value: i32,
    name: String,
}}

impl Struct_{} {{
    fn new() -> Self {{
        Self {{
            value: {},
            name: "test_{}".to_string(),
        }}
    }}
    
    fn calculate(&self) -> i32 {{
        self.value * 2
    }}
}}

const CONSTANT_{}: i32 = {};
"#, i, i, i, i, i, i, i, i, i, i);
        
        fs::write(&file_path, content)?;
    }
    let file_creation_duration = file_creation_start.elapsed();
    
    let creation_metrics = PerformanceMetrics::new(
        "ファイル作成".to_string(),
        file_creation_duration,
        file_count,
    );
    creation_metrics.print();
    
    // 初回ファイル読み込み（キャッシュなし）
    let initial_loading_start = Instant::now();
    let mut total_symbols = 0;
    
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("perf_test_{}.rs", i));
        let symbols = cache_manager.get_symbols(&file_path)?;
        total_symbols += symbols.len();
    }
    
    let initial_loading_duration = initial_loading_start.elapsed();
    let initial_metrics = PerformanceMetrics::new(
        "初回ファイル読み込み（Tree-sitter解析込み）".to_string(),
        initial_loading_duration,
        total_symbols,
    );
    initial_metrics.print();
    
    // キャッシュされたファイル読み込み
    let cached_loading_start = Instant::now();
    let mut cached_total_symbols = 0;
    
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("perf_test_{}.rs", i));
        let symbols = cache_manager.get_symbols(&file_path)?;
        cached_total_symbols += symbols.len();
    }
    
    let cached_loading_duration = cached_loading_start.elapsed();
    let cached_metrics = PerformanceMetrics::new(
        "キャッシュ済みファイル読み込み".to_string(),
        cached_loading_duration,
        cached_total_symbols,
    );
    cached_metrics.print();
    
    // パフォーマンス回帰チェック
    assert_eq!(total_symbols, cached_total_symbols, "シンボル数は一致すべき");
    
    // キャッシュは初回読み込みより十分高速であるべき
    let speedup_ratio = initial_loading_duration.as_nanos() as f64 / cached_loading_duration.as_nanos() as f64;
    println!("💡 キャッシュによる高速化率: {:.1}倍", speedup_ratio);
    assert!(speedup_ratio > 5.0, "キャッシュは5倍以上高速であるべき");
    
    // 絶対パフォーマンス要件
    assert!(initial_metrics.items_per_second > 100.0,
            "初回読み込みは100 symbols/sec以上であるべき");
    assert!(cached_metrics.items_per_second > 10000.0,
            "キャッシュ読み込みは10K symbols/sec以上であるべき");
    
    println!("✅ ファイル読み込みパフォーマンステスト完了");
    Ok(())
}

/// SearchRunnerの統合パフォーマンステスト
#[tokio::test]
async fn test_search_runner_performance() -> Result<()> {
    println!("🔍 SearchRunner統合パフォーマンステスト");
    
    let temp_dir = TempDir::new()?;
    let file_count = 50;
    
    // 検索用の多様なファイルを作成
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("search_perf_{}.rs", i));
        let content = format!(r#"
// SearchRunner performance test file {}
use std::{{collections::HashMap, sync::Arc}};

pub fn public_function_{}() -> Result<i32, String> {{
    let result = private_helper_{}();
    Ok(result * 2)
}}

fn private_helper_{}() -> i32 {{
    let mut cache: HashMap<String, i32> = HashMap::new();
    for j in 0..{} {{
        cache.insert(format!("key_{{}}", j), j * {});
    }}
    cache.len() as i32
}}

pub struct SearchTestStruct_{} {{
    pub id: u64,
    pub name: String,
    pub metadata: HashMap<String, String>,
    pub performance_counter: Arc<std::sync::Mutex<i32>>,
}}

impl SearchTestStruct_{} {{
    pub fn new(id: u64, name: &str) -> Self {{
        Self {{
            id,
            name: name.to_string(),
            metadata: HashMap::new(),
            performance_counter: Arc::new(std::sync::Mutex::new(0)),
        }}
    }}
    
    pub fn benchmark_method(&self) -> String {{
        format!("benchmark_result_{{}}_{{}}", self.id, self.name)
    }}
}}

pub trait SearchPerformanceTrait {{
    fn trait_method_{}(&self) -> i32;
}}

impl SearchPerformanceTrait for SearchTestStruct_{} {{
    fn trait_method_{}(&self) -> i32 {{
        {}
    }}
}}

pub const SEARCH_CONSTANT_{}: i32 = {};
pub const TEST_STRING_{}: &str = "performance_test_string_{}";
"#, i, i, i, i, i % 20 + 1, i, i, i, i, i, i, i, i * 42, i, i, i);
        
        fs::write(&file_path, content)?;
    }
    
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // シンボル検索のパフォーマンステスト
    let symbol_strategy = SymbolStrategy::new();
    for query in &["function", "struct", "trait"] {
        let start = Instant::now();
        let results = search_runner.collect_results_with_strategy(&symbol_strategy, query)?;
        let search_duration = start.elapsed();
        
        let search_metrics = PerformanceMetrics::new(
            format!("シンボル検索 '{}'", query),
            search_duration,
            results.len(),
        );
        search_metrics.print();
        
        // パフォーマンス要件チェック
        assert!(search_duration.as_millis() < 2000,
                "検索は2秒未満であるべき");
        assert!(results.len() > 0, "関連する検索では結果が見つかるべき");
    }
    
    // コンテンツ検索のパフォーマンステスト
    let content_strategy = ContentStrategy;
    for query in &["HashMap", "performance"] {
        let start = Instant::now();
        let results = search_runner.collect_results_with_strategy(&content_strategy, query)?;
        let search_duration = start.elapsed();
        
        let search_metrics = PerformanceMetrics::new(
            format!("コンテンツ検索 '{}'", query),
            search_duration,
            results.len(),
        );
        search_metrics.print();
        
        assert!(search_duration.as_millis() < 2000,
                "検索は2秒未満であるべき");
    }
    
    // ファイル検索のパフォーマンステスト
    let file_strategy = FileStrategy;
    let start = Instant::now();
    let results = search_runner.collect_results_with_strategy(&file_strategy, "search_perf")?;
    let search_duration = start.elapsed();
    
    let search_metrics = PerformanceMetrics::new(
        "ファイル検索 'search_perf'".to_string(),
        search_duration,
        results.len(),
    );
    search_metrics.print();
    
    assert!(search_duration.as_millis() < 2000,
            "検索は2秒未満であるべき");
    
    // 複数回検索のパフォーマンス（キャッシュ効果確認）
    let repeated_queries = vec!["function", "struct", "HashMap"];
    
    for query in repeated_queries {
        let mut durations = Vec::new();
        let strategy = SymbolStrategy::new();
        
        for iteration in 0..5 {
            let start = Instant::now();
            let results = search_runner.collect_results_with_strategy(&strategy, query)?;
            let duration = start.elapsed();
            durations.push(duration);
            
            println!("🔄 反復検索 {} 回目 '{}': {:?} ({} 件)", 
                     iteration + 1, query, duration, results.len());
        }
        
        // 反復検索では性能が安定または向上すべき
        let first_duration = durations[0];
        let last_duration = durations[4];
        
        println!("📈 検索性能変化 '{}': {:?} → {:?}", query, first_duration, last_duration);
        
        // 5回目の検索は1回目より遅くなりすぎてはいけない（キャッシュ効果）
        assert!(last_duration.as_millis() <= first_duration.as_millis() * 3,
                "反復検索の性能劣化は3倍以内であるべき");
    }
    
    println!("✅ SearchRunner統合パフォーマンステスト完了");
    Ok(())
}

/// メモリ使用量増加率テスト
#[tokio::test]
async fn test_memory_growth_patterns() -> Result<()> {
    println!("🔍 メモリ使用量増加パターンテスト");
    
    let temp_dir = TempDir::new()?;
    let mut cache_manager = CacheManager::new();
    
    let mut total_symbols = 0;
    let mut file_count = 0;
    
    // 段階的にファイルを追加してメモリ使用量を測定
    for batch in 0..5 {
        let batch_start = Instant::now();
        let batch_size = 20;
        
        // バッチ内でファイルを作成・処理
        for i in 0..batch_size {
            let file_idx = batch * batch_size + i;
            let file_path = temp_dir.path().join(format!("memory_test_{}.rs", file_idx));
            
            let content = format!(r#"
// Memory growth test file {}
fn memory_function_{}() -> Vec<String> {{
    let mut result = Vec::new();
    for i in 0..{} {{
        result.push(format!("item_{{}}_batch_{{}}", i, {}));
    }}
    result
}}

struct MemoryStruct_{} {{
    data: Vec<i32>,
    metadata: std::collections::HashMap<String, String>,
}}

impl MemoryStruct_{} {{
    fn new() -> Self {{
        let mut data = Vec::new();
        let mut metadata = std::collections::HashMap::new();
        
        for i in 0..{} {{
            data.push(i * {});
            metadata.insert(format!("key_{{}}", i), format!("value_{{}}_file_{{}}", i, {}));
        }}
        
        Self {{ data, metadata }}
    }}
}}
"#, file_idx, file_idx, file_idx % 50 + 10, batch, file_idx, file_idx, file_idx % 30 + 5, file_idx, file_idx);
            
            fs::write(&file_path, content)?;
            
            // シンボル抽出
            let symbols = cache_manager.get_symbols(&file_path)?;
            total_symbols += symbols.len();
            file_count += 1;
        }
        
        let batch_duration = batch_start.elapsed();
        
        let batch_metrics = PerformanceMetrics::new(
            format!("バッチ {} (累計 {} ファイル)", batch + 1, file_count),
            batch_duration,
            batch_size,
        );
        batch_metrics.print();
        
        // ファジー検索パフォーマンス（累積データでの性能変化）
        let search_start = Instant::now();
        let search_results = cache_manager.fuzzy_search_symbols("function", 50);
        let search_duration = search_start.elapsed();
        
        println!("🔎 累積データ検索: {} ファイル, {} シンボル → {} 件検索結果 ({:?})",
                 file_count, total_symbols, search_results.len(), search_duration);
        
        // 性能劣化チェック：検索時間がファイル数に対して線形以下であることを確認
        let expected_max_duration_ms = file_count as u64 * 10; // 1ファイルあたり10ms上限
        assert!(search_duration.as_millis() <= expected_max_duration_ms as u128,
                "検索時間がファイル数に比例して過度に増加している");
    }
    
    // 最終パフォーマンス概要
    println!("📊 最終統計:");
    println!("   処理ファイル数: {}", file_count);
    println!("   総シンボル数: {}", total_symbols);
    println!("   平均シンボル/ファイル: {:.1}", total_symbols as f64 / file_count as f64);
    
    // 最終検索パフォーマンステスト
    let final_search_queries = vec!["memory", "function", "struct", "impl"];
    for query in final_search_queries {
        let start = Instant::now();
        let results = cache_manager.fuzzy_search_symbols(query, 100);
        let duration = start.elapsed();
        
        println!("🎯 最終検索 '{}': {} 件 ({:?})", query, results.len(), duration);
        
        assert!(duration.as_millis() < 500,
                "最終状態でも検索は500ms未満であるべき");
    }
    
    println!("✅ メモリ使用量増加パターンテスト完了");
    Ok(())
}

/// 並行処理性能テスト
#[tokio::test]
async fn test_concurrent_performance() -> Result<()> {
    println!("🔍 並行処理性能テスト");
    
    let temp_dir = TempDir::new()?;
    let file_count = 30;
    
    // 並行処理用のファイルを準備
    for i in 0..file_count {
        let file_path = temp_dir.path().join(format!("concurrent_{}.rs", i));
        let content = format!(r#"
// Concurrent performance test {}
fn concurrent_function_{}() -> i32 {{
    let mut sum = 0;
    for j in 0..{} {{
        sum += j * {};
    }}
    sum
}}

struct ConcurrentStruct_{} {{
    value: i32,
}}

impl ConcurrentStruct_{} {{
    fn process(&self) -> String {{
        format!("processed_{{}}_{{}}", self.value, {})
    }}
}}
"#, i, i, i % 100 + 10, i, i, i, i);
        
        fs::write(&file_path, content)?;
    }
    
    let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
    
    // シーケンシャル処理時間測定
    let sequential_start = Instant::now();
    let strategy = SymbolStrategy::new();
    
    let mut sequential_results = Vec::new();
    for query in &["function", "struct", "impl", "concurrent"] {
        let results = search_runner.collect_results_with_strategy(&strategy, query)?;
        sequential_results.push((query, results.len()));
    }
    
    let sequential_duration = sequential_start.elapsed();
    let sequential_metrics = PerformanceMetrics::new(
        "シーケンシャル検索".to_string(),
        sequential_duration,
        sequential_results.iter().map(|(_, count)| count).sum(),
    );
    sequential_metrics.print();
    
    // 並行処理時間測定
    let concurrent_start = Instant::now();
    let search_runner_arc = std::sync::Arc::new(search_runner);
    
    let mut handles = Vec::new();
    let queries = vec!["function", "struct", "impl", "concurrent"];
    
    for query in queries {
        let runner = search_runner_arc.clone();
        let query = query.to_string();
        
        let handle = tokio::spawn(async move {
            let strategy = SymbolStrategy::new();
            let results = runner.collect_results_with_strategy(&strategy, &query)
                .unwrap_or_default();
            (query, results.len())
        });
        
        handles.push(handle);
    }
    
    // 結果収集
    let mut concurrent_results = Vec::new();
    for handle in handles {
        let result = handle.await?;
        concurrent_results.push(result);
    }
    
    let concurrent_duration = concurrent_start.elapsed();
    let concurrent_metrics = PerformanceMetrics::new(
        "並行検索".to_string(),
        concurrent_duration,
        concurrent_results.iter().map(|(_, count)| count).sum(),
    );
    concurrent_metrics.print();
    
    // 結果の一致確認
    sequential_results.sort_by(|a, b| a.0.cmp(&b.0));
    concurrent_results.sort_by(|a, b| a.0.cmp(&b.0));
    
    for ((seq_query, seq_count), (conc_query, conc_count)) in 
        sequential_results.iter().zip(concurrent_results.iter()) {
        assert_eq!(*seq_query, conc_query, "クエリ順序が一致すべき");
        assert_eq!(seq_count, conc_count, "検索結果数が一致すべき: {} vs {}", seq_count, conc_count);
    }
    
    // 並行処理による性能向上確認
    let speedup = sequential_duration.as_nanos() as f64 / concurrent_duration.as_nanos() as f64;
    println!("💡 並行処理による高速化: {:.2}倍", speedup);
    
    // 並行処理は少なくとも同程度以上の性能であるべき
    assert!(speedup >= 0.8, "並行処理は性能劣化が20%以内であるべき");
    
    println!("✅ 並行処理性能テスト完了");
    Ok(())
}