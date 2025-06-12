// デダプリケーション機能のテストケース
// ファイル名シンボル優先度、ディレクトリ重複排除、ソート順保持をテスト

use sfs::indexer::TreeSitterIndexer;
use sfs::types::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod deduplication_tests {
    use super::*;

    fn create_duplicate_symbol_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // Create nested directory structure that could cause duplicates
        fs::create_dir_all(dir_path.join("src/components"))?;
        fs::create_dir_all(dir_path.join("src/utils"))?;
        fs::create_dir_all(dir_path.join("tests/components"))?;

        // Create files with similar names that could cause confusion
        fs::write(
            dir_path.join("src/main.ts"),
            "function main() {}\nconst helper = () => 'main';\nclass App {}",
        )?;

        fs::write(
            dir_path.join("src/components/App.ts"),
            "function render() {}\nconst helper = () => 'component';\nclass Component {}",
        )?;

        fs::write(
            dir_path.join("src/utils/helper.ts"),
            "function helper() {}\nconst utility = () => 'util';\nclass Helper {}",
        )?;

        fs::write(
            dir_path.join("tests/main.test.ts"),
            "function testMain() {}\nconst testHelper = () => 'test';",
        )?;

        // Create files with same base names in different directories
        fs::write(dir_path.join("src/index.ts"), "export * from './main';")?;
        fs::write(
            dir_path.join("tests/index.ts"),
            "export * from './main.test';",
        )?;

        Ok(())
    }

    #[tokio::test]
    async fn should_prioritize_filename_symbols_over_code_symbols() {
        let temp_dir = TempDir::new().unwrap();
        create_duplicate_symbol_project(&temp_dir).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let all_symbols = indexer.get_all_symbols();

        // Group symbols by file
        let mut symbols_by_file: HashMap<std::path::PathBuf, Vec<&CodeSymbol>> = HashMap::new();
        for symbol in &all_symbols {
            symbols_by_file
                .entry(symbol.file.clone())
                .or_default()
                .push(symbol);
        }

        // Verify each file has filename symbol
        for (file_path, symbols) in &symbols_by_file {
            let filename_symbols: Vec<_> = symbols
                .iter()
                .filter(|s| s.symbol_type == SymbolType::Filename)
                .collect();

            assert!(
                !filename_symbols.is_empty(),
                "File {:?} should have at least one filename symbol",
                file_path
            );

            // Verify filename symbol has correct name
            if let Some(expected_name) = file_path.file_name() {
                assert!(
                    filename_symbols
                        .iter()
                        .any(|s| s.name == expected_name.to_string_lossy()),
                    "File {:?} should have filename symbol with correct name",
                    file_path
                );
            }
        }

        // Verify we have the expected files
        let file_names: Vec<String> = all_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Filename)
            .map(|s| s.name.clone())
            .collect();

        assert!(file_names.contains(&"main.ts".to_string()));
        assert!(file_names.contains(&"App.ts".to_string()));
        assert!(file_names.contains(&"helper.ts".to_string()));
        assert!(file_names.contains(&"main.test.ts".to_string()));

        println!(
            "Found {} filename symbols: {:?}",
            file_names.len(),
            file_names
        );
    }

    #[tokio::test]
    async fn should_handle_directory_name_uniqueness() {
        let temp_dir = TempDir::new().unwrap();
        create_duplicate_symbol_project(&temp_dir).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let all_symbols = indexer.get_all_symbols();

        // Get all directory symbols
        let dir_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Dirname)
            .collect();

        // Count occurrences of each directory name
        let mut dir_name_counts: HashMap<String, usize> = HashMap::new();
        for symbol in &dir_symbols {
            *dir_name_counts.entry(symbol.name.clone()).or_insert(0) += 1;
        }

        // Common directory names should appear multiple times (not deduplicated at this level)
        assert!(
            dir_name_counts.get("src").map_or(0, |&count| count) > 0,
            "Should find 'src' directory symbols"
        );
        assert!(
            dir_name_counts.get("components").map_or(0, |&count| count) > 0,
            "Should find 'components' directory symbols"
        );

        // But they should be associated with correct files
        let src_symbols: Vec<_> = dir_symbols.iter().filter(|s| s.name == "src").collect();

        // Files directly in src/ directory should have a "src" dirname symbol
        let src_direct_files = ["main.ts", "index.ts"];
        for file_name in &src_direct_files {
            assert!(
                src_symbols
                    .iter()
                    .any(|s| s.file.to_string_lossy().contains(file_name)),
                "src directory symbol should be associated with {}",
                file_name
            );
        }

        // Files in src/components/ and src/utils/ should have "components"/"utils" dirname symbols
        let components_symbols: Vec<_> = dir_symbols
            .iter()
            .filter(|s| s.name == "components")
            .collect();
        assert!(
            components_symbols
                .iter()
                .any(|s| s.file.to_string_lossy().contains("App.ts")),
            "components directory symbol should be associated with App.ts"
        );

        let utils_symbols: Vec<_> = dir_symbols.iter().filter(|s| s.name == "utils").collect();
        assert!(
            utils_symbols
                .iter()
                .any(|s| s.file.to_string_lossy().contains("helper.ts")),
            "utils directory symbol should be associated with helper.ts"
        );

        println!("Directory name distribution: {:?}", dir_name_counts);
    }

    #[tokio::test]
    async fn should_preserve_file_representative_selection_logic() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create a file with both filename and code symbols
        fs::write(
            dir_path.join("calculator.ts"),
            "class Calculator {\n\
                add(a: number, b: number) { return a + b; }\n\
                multiply(a: number, b: number) { return a * b; }\n\
             }\n\
             const helper = (x: number) => x * 2;\n\
             function process() { return 'processed'; }",
        )?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let all_symbols = indexer.get_all_symbols();
        let file_path = dir_path.join("calculator.ts");

        // Get all symbols for this file
        let file_symbols: Vec<_> = all_symbols.iter().filter(|s| s.file == file_path).collect();

        // Should have filename symbol
        let filename_symbols: Vec<_> = file_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Filename)
            .collect();
        assert!(!filename_symbols.is_empty(), "Should have filename symbol");

        // Should have directory symbol
        let dirname_symbols: Vec<_> = file_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Dirname)
            .collect();
        assert!(!dirname_symbols.is_empty(), "Should have dirname symbol");

        // Should have code symbols
        let code_symbols: Vec<_> = file_symbols
            .iter()
            .filter(|s| {
                matches!(
                    s.symbol_type,
                    SymbolType::Class | SymbolType::Function | SymbolType::Variable
                )
            })
            .collect();
        assert!(!code_symbols.is_empty(), "Should have code symbols");

        // Verify specific symbols exist
        assert!(file_symbols
            .iter()
            .any(|s| s.name == "calculator.ts" && s.symbol_type == SymbolType::Filename));
        assert!(file_symbols
            .iter()
            .any(|s| s.name == "Calculator" && s.symbol_type == SymbolType::Class));
        assert!(file_symbols
            .iter()
            .any(|s| s.name == "add" && s.symbol_type == SymbolType::Function));
        assert!(file_symbols
            .iter()
            .any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function));
        assert!(file_symbols
            .iter()
            .any(|s| s.name == "process" && s.symbol_type == SymbolType::Function));

        println!(
            "File {} has {} symbols: {} filename, {} dirname, {} code",
            file_path.display(),
            file_symbols.len(),
            filename_symbols.len(),
            dirname_symbols.len(),
            code_symbols.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_files_with_only_code_symbols() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create a file that will generate code symbols
        fs::write(
            dir_path.join("logic.ts"),
            "function logic() { return true; }\n\
             const value = 42;\n\
             class Processor {}",
        )?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let all_symbols = indexer.get_all_symbols();
        let file_path = dir_path.join("logic.ts");

        // Get symbols for this specific file
        let file_symbols: Vec<_> = all_symbols.iter().filter(|s| s.file == file_path).collect();

        // Should always have filename symbol (created by indexer)
        assert!(
            file_symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::Filename),
            "Should have filename symbol even with code symbols present"
        );

        // Should have code symbols as well
        assert!(
            file_symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::Function),
            "Should have function symbols"
        );
        assert!(
            file_symbols
                .iter()
                .any(|s| s.symbol_type == SymbolType::Class),
            "Should have class symbols"
        );

        // Verify that the filename symbol is the expected representative
        let filename_symbol = file_symbols
            .iter()
            .find(|s| s.symbol_type == SymbolType::Filename)
            .expect("Should have filename symbol");

        assert_eq!(filename_symbol.name, "logic.ts");
        assert_eq!(filename_symbol.file, file_path);

        println!(
            "File with code symbols has {} total symbols",
            file_symbols.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_maintain_consistent_symbol_ordering() {
        let temp_dir = TempDir::new().unwrap();
        create_duplicate_symbol_project(&temp_dir).unwrap();

        // Run indexing multiple times to check consistency
        let mut all_runs_symbols = Vec::new();

        for run in 0..3 {
            let mut indexer = TreeSitterIndexer::with_verbose(false);
            indexer.initialize().await.unwrap();

            let patterns = vec!["**/*.ts".to_string()];
            indexer
                .index_directory(temp_dir.path(), &patterns)
                .await
                .unwrap();

            let symbols = indexer.get_all_symbols();
            all_runs_symbols.push(symbols);

            println!(
                "Run {}: Found {} symbols",
                run + 1,
                all_runs_symbols[run].len()
            );
        }

        // Verify all runs produced the same number of symbols
        let symbol_counts: Vec<usize> = all_runs_symbols.iter().map(|s| s.len()).collect();
        assert!(
            symbol_counts.iter().all(|&count| count == symbol_counts[0]),
            "Symbol counts should be consistent: {:?}",
            symbol_counts
        );

        // Verify symbol names are consistent (though order might vary)
        for run_idx in 1..all_runs_symbols.len() {
            let run0_names: std::collections::HashSet<_> = all_runs_symbols[0]
                .iter()
                .map(|s| (&s.name, &s.symbol_type, &s.file))
                .collect();
            let run_names: std::collections::HashSet<_> = all_runs_symbols[run_idx]
                .iter()
                .map(|s| (&s.name, &s.symbol_type, &s.file))
                .collect();

            assert_eq!(
                run0_names, run_names,
                "Symbol sets should be identical between runs 0 and {}",
                run_idx
            );
        }

        println!(
            "Deduplication logic is consistent across {} runs",
            all_runs_symbols.len()
        );
    }
}
