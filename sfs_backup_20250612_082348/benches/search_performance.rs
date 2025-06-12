use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sfs::{FuzzySearcher, SearchOptions, SymbolType, TreeSitterIndexer};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// Helper function to create test files with many symbols
fn create_symbol_heavy_files(
    dir: &Path,
    file_count: usize,
    symbols_per_file: usize,
) -> anyhow::Result<()> {
    for file_idx in 0..file_count {
        let mut content = String::new();

        // Generate many functions and classes
        for i in 0..symbols_per_file {
            content.push_str(&format!(
                "export function func_{}_{}_{}() {{\n",
                file_idx,
                i,
                ["search", "find", "filter", "map", "reduce", "process", "handle", "execute"]
                    [i % 8]
            ));
            content.push_str(&format!("  const var_{}_{} = 'value';\n", file_idx, i));
            content.push_str(&format!("  return var_{}_{};\n", file_idx, i));
            content.push_str("}\n\n");

            if i % 10 == 0 {
                content.push_str(&format!(
                    "export class Class_{}_{} {{\n  field_{}_{}: string;\n  method_{}_{}_{}() {{}}\n}}\n\n",
                    file_idx, i, file_idx, i, file_idx, i,
                    ["handler", "processor", "manager", "service", "controller"][i % 5]
                ));
            }
        }

        let file_path = dir.join(format!("symbols_{}.ts", file_idx));
        fs::write(file_path, content)?;
    }
    Ok(())
}

// Setup function to create indexer with symbols
async fn setup_indexer_with_symbols(symbol_count: usize) -> (TempDir, FuzzySearcher) {
    let temp_dir = TempDir::new().unwrap();
    let files_needed = (symbol_count / 50).max(1); // ~50 symbols per file
    create_symbol_heavy_files(temp_dir.path(), files_needed, 50).unwrap();

    let mut indexer = TreeSitterIndexer::with_options(false, true);
    indexer.initialize().await.unwrap();
    let patterns = vec!["**/*".to_string()];
    indexer
        .index_directory(temp_dir.path(), &patterns)
        .await
        .unwrap();

    let symbols = indexer.get_all_symbols();
    let searcher = FuzzySearcher::new(symbols);

    (temp_dir, searcher)
}

// Benchmark fuzzy search with different symbol counts
fn bench_fuzzy_search_by_symbol_count(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("fuzzy_search_by_symbol_count");

    for symbol_count in [100, 500, 1000, 5000].iter() {
        let (_temp_dir, searcher) = rt.block_on(setup_indexer_with_symbols(*symbol_count));

        group.bench_with_input(format!("{}_symbols", symbol_count), symbol_count, |b, _| {
            b.iter(|| {
                let options = SearchOptions {
                    limit: Some(10),
                    threshold: Some(0.5),
                    ..Default::default()
                };
                black_box(searcher.search(black_box("search"), black_box(&options)))
            })
        });
    }
    group.finish();
}

// Benchmark different search query types
fn bench_search_query_types(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_temp_dir, searcher) = rt.block_on(setup_indexer_with_symbols(1000));

    let mut group = c.benchmark_group("search_query_types");

    let test_queries = [
        ("exact_match", "func_0_0_search"),
        ("partial_fuzzy", "srch"),
        ("common_word", "handle"),
        ("class_search", "Class"),
        ("mixed_case", "FuNc"),
        ("long_query", "function_that_does_something_specific"),
    ];

    for (query_type, query) in test_queries.iter() {
        group.bench_function(*query_type, |b| {
            b.iter(|| {
                let options = SearchOptions {
                    limit: Some(10),
                    threshold: Some(0.5),
                    ..Default::default()
                };
                black_box(searcher.search(black_box(query), black_box(&options)))
            })
        });
    }
    group.finish();
}

// Benchmark different search thresholds
fn bench_search_thresholds(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_temp_dir, searcher) = rt.block_on(setup_indexer_with_symbols(1000));

    let mut group = c.benchmark_group("search_thresholds");

    for threshold in [0.0, 0.3, 0.5, 0.7, 0.9].iter() {
        group.bench_with_input(
            format!("threshold_{}", threshold),
            threshold,
            |b, &threshold| {
                b.iter(|| {
                    let options = SearchOptions {
                        limit: Some(10),
                        threshold: Some(threshold),
                        ..Default::default()
                    };
                    black_box(searcher.search(black_box("search"), black_box(&options)))
                })
            },
        );
    }
    group.finish();
}

// Benchmark different result limits
fn bench_search_limits(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_temp_dir, searcher) = rt.block_on(setup_indexer_with_symbols(2000));

    let mut group = c.benchmark_group("search_limits");

    for limit in [5, 10, 25, 50, 100].iter() {
        group.bench_with_input(format!("limit_{}", limit), limit, |b, &limit| {
            b.iter(|| {
                let options = SearchOptions {
                    limit: Some(limit),
                    threshold: Some(0.5),
                    ..Default::default()
                };
                black_box(searcher.search(black_box("search"), black_box(&options)))
            })
        });
    }
    group.finish();
}

// Benchmark symbol type filtering
fn bench_symbol_type_filtering(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (_temp_dir, searcher) = rt.block_on(setup_indexer_with_symbols(1000));

    let mut group = c.benchmark_group("symbol_type_filtering");

    let filter_options = [
        ("no_filter", vec![]),
        ("functions_only", vec![SymbolType::Function]),
        ("classes_only", vec![SymbolType::Class]),
        (
            "multiple_types",
            vec![SymbolType::Function, SymbolType::Class],
        ),
    ];

    for (filter_name, symbol_types) in filter_options.iter() {
        group.bench_function(*filter_name, |b| {
            b.iter(|| {
                let options = SearchOptions {
                    limit: Some(10),
                    threshold: Some(0.5),
                    types: Some(symbol_types.clone()),
                    ..Default::default()
                };
                black_box(searcher.search(black_box("search"), black_box(&options)))
            })
        });
    }
    group.finish();
}

criterion_group!(
    search_benches,
    bench_fuzzy_search_by_symbol_count,
    bench_search_query_types,
    bench_search_thresholds,
    bench_search_limits,
    bench_symbol_type_filtering
);

criterion_main!(search_benches);
