// プログレッシブインデックシング機能のテストケース
// バックグラウンド処理、チャネル通信、UI応答性をテスト

use sfs::indexer::TreeSitterIndexer;
use sfs::types::*;
use std::fs;
use tempfile::TempDir;
// use std::sync::mpsc;
use std::time::{Duration, Instant};

#[cfg(test)]
mod progressive_indexing_tests {
    use super::*;

    fn create_test_project_with_multiple_files(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // Create a realistic project structure for testing progressive indexing
        fs::create_dir_all(dir_path.join("src"))?;
        fs::create_dir_all(dir_path.join("tests"))?;
        fs::create_dir_all(dir_path.join("docs"))?;

        // Create TypeScript files
        fs::write(
            dir_path.join("src/main.ts"),
            "function main() {\n    console.log('Hello World');\n}\nexport { main };",
        )?;
        fs::write(
            dir_path.join("src/utils.ts"),
            "const helper = (x: string) => x.toUpperCase();\nclass Manager { process() {} }",
        )?;
        fs::write(
            dir_path.join("src/types.ts"),
            "interface User { id: number; name: string; }\ntype Status = 'active' | 'inactive';",
        )?;

        // Create JavaScript files
        fs::write(
            dir_path.join("tests/test.js"),
            "function testHelper() { return true; }\nconst assert = require('assert');",
        )?;
        fs::write(
            dir_path.join("tests/integration.js"),
            "describe('Integration', () => { it('works', () => {}); });",
        )?;

        // Create Python files
        fs::write(
            dir_path.join("docs/generator.py"),
            "def generate_docs():\n    pass\n\nclass DocGenerator:\n    def run(self): pass",
        )?;

        // Create some larger files to test progressive behavior
        let large_content = (0..100)
            .map(|i| format!("function func{}() {{ return {}; }}", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(dir_path.join("src/large.ts"), large_content)?;

        Ok(())
    }

    #[tokio::test]
    async fn should_perform_basic_progressive_indexing() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project_with_multiple_files(&temp_dir).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        // Test progressive indexing with realistic patterns
        let patterns = vec![
            "**/*.ts".to_string(),
            "**/*.js".to_string(),
            "**/*.py".to_string(),
        ];

        let start_time = Instant::now();
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();
        let duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // Verify symbols were extracted from multiple files
        assert!(
            all_symbols.len() > 10,
            "Should extract symbols from multiple files"
        );

        // Verify symbols from different file types
        assert!(
            all_symbols.iter().any(|s| s.name == "main"),
            "Should find main function"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "Manager"),
            "Should find Manager class"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "testHelper"),
            "Should find testHelper function"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "generate_docs"),
            "Should find Python function"
        );

        // Verify performance is reasonable
        assert!(
            duration < Duration::from_secs(5),
            "Indexing should complete within 5 seconds"
        );

        println!(
            "Progressive indexing completed in {:?} with {} symbols",
            duration,
            all_symbols.len()
        );
    }

    #[test]
    fn should_handle_extract_symbols_sync_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.ts");

        fs::write(
            &file_path,
            "function asyncFunc() { return Promise.resolve(); }\n\
             const arrow = () => 'test';\n\
             class TestClass { method() {} }",
        )
        .unwrap();

        let indexer = TreeSitterIndexer::with_verbose(true);

        // Test the synchronous symbol extraction used in progressive indexing
        let symbols = indexer.extract_symbols_sync(&file_path, true).unwrap();

        // Verify different symbol types are extracted
        assert!(symbols
            .iter()
            .any(|s| s.name == "asyncFunc" && s.symbol_type == SymbolType::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "arrow" && s.symbol_type == SymbolType::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "TestClass" && s.symbol_type == SymbolType::Class));
        assert!(symbols
            .iter()
            .any(|s| s.name == "test.ts" && s.symbol_type == SymbolType::Filename));

        assert!(
            symbols.len() >= 4,
            "Should extract at least 4 symbols (function, arrow, class, filename)"
        );
    }

    #[test]
    fn should_handle_non_existent_files_gracefully_in_sync_mode() {
        let indexer = TreeSitterIndexer::with_verbose(false);
        let non_existent_path = std::path::Path::new("/definitely/does/not/exist.ts");

        // Should not panic and return empty result
        let result = indexer.extract_symbols_sync(non_existent_path, false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn should_respect_file_filtering_during_progressive_indexing() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create files with different extensions
        fs::write(dir_path.join("code.ts"), "function test() {}").unwrap();
        fs::write(dir_path.join("data.json"), r#"{"key": "value"}"#).unwrap();
        fs::write(dir_path.join("readme.md"), "# README").unwrap();
        fs::write(dir_path.join("image.png"), b"fake image data").unwrap();

        let _indexer = TreeSitterIndexer::with_verbose(false);

        // Test with TypeScript-only pattern
        let ts_symbols = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let mut indexer = TreeSitterIndexer::with_verbose(false);
            indexer.initialize().await.unwrap();
            let patterns = vec!["**/*.ts".to_string()];
            indexer
                .index_directory(temp_dir.path(), &patterns)
                .await
                .unwrap();
            indexer.get_all_symbols()
        });

        // Should only find TypeScript-related symbols
        assert!(ts_symbols.iter().any(|s| s.name == "code.ts"));
        assert!(!ts_symbols.iter().any(|s| s.name == "data.json"));
        assert!(!ts_symbols.iter().any(|s| s.name == "readme.md"));
        assert!(!ts_symbols.iter().any(|s| s.name == "image.png"));
    }

    #[test]
    fn should_maintain_symbol_consistency_across_multiple_runs() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project_with_multiple_files(&temp_dir).unwrap();

        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string()];

        // Run indexing multiple times
        let mut symbol_counts = Vec::new();

        for _ in 0..3 {
            let symbols = tokio::runtime::Runtime::new().unwrap().block_on(async {
                let mut indexer = TreeSitterIndexer::with_verbose(false);
                indexer.initialize().await.unwrap();
                indexer
                    .index_directory(temp_dir.path(), &patterns)
                    .await
                    .unwrap();
                indexer.get_all_symbols()
            });

            symbol_counts.push(symbols.len());
        }

        // All runs should produce the same number of symbols
        assert!(
            symbol_counts.iter().all(|&count| count == symbol_counts[0]),
            "Symbol counts should be consistent across runs: {:?}",
            symbol_counts
        );

        println!("Consistent symbol count across runs: {}", symbol_counts[0]);
    }
}
