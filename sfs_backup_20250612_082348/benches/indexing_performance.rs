use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sfs::TreeSitterIndexer;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// Helper function to create test files
fn create_test_files(dir: &Path, file_count: usize, file_size: usize) -> anyhow::Result<()> {
    for i in 0..file_count {
        let content = match i % 4 {
            0 => format!(
                "{}
export function testFunction{}() {{
    const variable{} = 'test';
    return variable{};
}}

export class TestClass{} {{
    private field{}: string;
    
    constructor() {{
        this.field{} = 'initialized';
    }}
    
    public method{}(): void {{
        console.log('method called');
    }}
}}",
                "// ".repeat(file_size / 10),
                i,
                i,
                i,
                i,
                i,
                i,
                i
            ),
            1 => format!(
                "{}
def test_function_{}():
    variable_{} = 'test'
    return variable_{}

class TestClass{}:
    def __init__(self):
        self.field_{} = 'initialized'
    
    def method_{}(self):
        print('method called')

TEST_CONSTANT_{} = 'constant_value'",
                "# ".repeat(file_size / 10),
                i,
                i,
                i,
                i,
                i,
                i,
                i
            ),
            2 => format!(
                "{}
func TestFunction{}() string {{
    variable{} := \"test\"
    return variable{}
}}

type TestStruct{} struct {{
    Field{} string
}}

func (t *TestStruct{}) Method{}() {{
    fmt.Println(\"method called\")
}}

const TestConstant{} = \"constant\"",
                "// ".repeat(file_size / 10),
                i,
                i,
                i,
                i,
                i,
                i,
                i,
                i
            ),
            _ => format!(
                "{}
fn test_function_{}() -> String {{
    let variable_{} = \"test\".to_string();
    variable_{}
}}

pub struct TestStruct{} {{
    field_{}: String,
}}

impl TestStruct{} {{
    pub fn new() -> Self {{
        Self {{
            field_{}: \"initialized\".to_string(),
        }}
    }}
    
    pub fn method_{}(&self) {{
        println!(\"method called\");
    }}
}}

const TEST_CONSTANT_{}: &str = \"constant\";",
                "// ".repeat(file_size / 10),
                i,
                i,
                i,
                i,
                i,
                i,
                i,
                i,
                i
            ),
        };

        let extension = match i % 4 {
            0 => "ts",
            1 => "py",
            2 => "go",
            _ => "rs",
        };

        let file_path = dir.join(format!("test_file_{}.{}", i, extension));
        fs::write(file_path, content)?;
    }
    Ok(())
}

// Benchmark indexing small projects (10 files)
fn bench_small_project_indexing(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path(), 10, 500).unwrap();

    c.bench_function("index_small_project_10_files", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut indexer = TreeSitterIndexer::with_options(false, true);
                indexer.initialize().await.unwrap();
                let patterns = vec!["**/*".to_string()];
                indexer
                    .index_directory(black_box(temp_dir.path()), black_box(&patterns))
                    .await
                    .unwrap();
                black_box(indexer.get_all_symbols().len())
            })
        })
    });
}

// Benchmark indexing medium projects (50 files)
fn bench_medium_project_indexing(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path(), 50, 1000).unwrap();

    c.bench_function("index_medium_project_50_files", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut indexer = TreeSitterIndexer::with_options(false, true);
                indexer.initialize().await.unwrap();
                let patterns = vec!["**/*".to_string()];
                indexer
                    .index_directory(black_box(temp_dir.path()), black_box(&patterns))
                    .await
                    .unwrap();
                black_box(indexer.get_all_symbols().len())
            })
        })
    });
}

// Benchmark indexing large projects (200 files)
fn bench_large_project_indexing(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    create_test_files(temp_dir.path(), 200, 2000).unwrap();

    c.bench_function("index_large_project_200_files", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut indexer = TreeSitterIndexer::with_options(false, true);
                indexer.initialize().await.unwrap();
                let patterns = vec!["**/*".to_string()];
                indexer
                    .index_directory(black_box(temp_dir.path()), black_box(&patterns))
                    .await
                    .unwrap();
                black_box(indexer.get_all_symbols().len())
            })
        })
    });
}

// Benchmark single file indexing with different sizes
fn bench_single_file_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_file_indexing");

    for size in [100, 500, 1000, 5000].iter() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path(), 1, *size).unwrap();

        group.bench_with_input(format!("file_size_{}_chars", size), size, |b, _| {
            b.iter(|| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut indexer = TreeSitterIndexer::with_options(false, true);
                    indexer.initialize().await.unwrap();
                    let patterns = vec!["**/*".to_string()];
                    indexer
                        .index_directory(black_box(temp_dir.path()), black_box(&patterns))
                        .await
                        .unwrap();
                    black_box(indexer.get_all_symbols().len())
                })
            })
        });
    }
    group.finish();
}

// Benchmark current project indexing (real-world test)
fn bench_current_project(c: &mut Criterion) {
    let current_dir = std::env::current_dir().unwrap();

    c.bench_function("index_current_project", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut indexer = TreeSitterIndexer::with_options(false, true);
                indexer.initialize().await.unwrap();
                let patterns = vec!["**/*".to_string()];
                indexer
                    .index_directory(black_box(&current_dir), black_box(&patterns))
                    .await
                    .unwrap();
                black_box(indexer.get_all_symbols().len())
            })
        })
    });
}

criterion_group!(
    indexing_benches,
    bench_small_project_indexing,
    bench_medium_project_indexing,
    bench_large_project_indexing,
    bench_single_file_sizes,
    bench_current_project
);

criterion_main!(indexing_benches);
