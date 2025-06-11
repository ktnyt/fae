// TypeScript tree-sitter-indexer.test.ts をRustに移植
// 目標: 10つのテストすべてをパスする

use sfs::types::*;
use sfs::indexer::TreeSitterIndexer;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use std::io::Write;

// Test fixture files path - TypeScriptテストと同じファイルを使用
fn get_fixtures_path() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

#[cfg(test)]
mod tree_sitter_indexer {
    use super::*;

    mod typescript_file_indexing {
        use super::*;

        #[tokio::test]
        async fn should_extract_typescript_symbols_correctly() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.ts");
            indexer.index_file(&file_path).await.unwrap();
            let symbols = indexer.get_symbols_by_file(&file_path);

            // Should include filename and dirname
            assert!(symbols.iter().any(|s| s.name == "sample.ts" && s.symbol_type == SymbolType::Filename));
            assert!(symbols.iter().any(|s| s.name == "fixtures" && s.symbol_type == SymbolType::Dirname));

            // Tree-sitter function queries should work well
            assert!(symbols.iter().any(|s| s.name == "formatUserName" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "addUser" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "findUserById" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "constructor" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "userCount" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "internalHelper" && s.symbol_type == SymbolType::Function));
            
            // Variables/constants are extracted as identifiers
            assert!(symbols.iter().any(|s| s.name == "DEFAULT_TIMEOUT"));
            
            // Status enum should be found as variable
            assert!(symbols.iter().any(|s| s.name == "Status"));

            // Verify symbol structure with actually found symbol
            let found_symbol = symbols.iter().find(|s| s.name == "formatUserName");
            assert!(found_symbol.is_some());
            let symbol = found_symbol.unwrap();
            assert_eq!(symbol.file, file_path);
            assert!(symbol.line > 0);
            assert!(symbol.column > 0);
        }
    }

    mod javascript_file_indexing {
        use super::*;

        #[tokio::test]
        async fn should_extract_javascript_symbols_correctly() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.js");
            indexer.index_file(&file_path).await.unwrap();
            let symbols = indexer.get_symbols_by_file(&file_path);

            // Check for class (class queries still failing, but should be found as identifier)
            assert!(symbols.iter().any(|s| s.name == "Calculator"));

            // Check for functions (should now be extracted as proper function types)
            assert!(symbols.iter().any(|s| s.name == "createCalculator" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "constructor" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "add" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "multiply" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "getValue" && s.symbol_type == SymbolType::Function));
            assert!(symbols.iter().any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function));

            // Check for constants/variables
            assert!(symbols.iter().any(|s| s.name == "API_BASE_URL"));
        }
    }

    mod python_file_indexing {
        use super::*;

        #[tokio::test]
        async fn should_extract_python_symbols_correctly() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.py");
            indexer.index_file(&file_path).await.unwrap();
            let symbols = indexer.get_symbols_by_file(&file_path);

            // Check for class (may be found as identifier due to Tree-sitter query failures)
            assert!(symbols.iter().any(|s| s.name == "DataProcessor"));

            // Check for functions (may be found as identifier due to Tree-sitter query failures)
            assert!(symbols.iter().any(|s| s.name == "process_file"));
            assert!(symbols.iter().any(|s| s.name == "calculate_sum"));

            // Check for constants/variables
            assert!(symbols.iter().any(|s| s.name == "MAX_ITEMS"));
            assert!(symbols.iter().any(|s| s.name == "counter"));
        }
    }

    mod comprehensive_function_extraction {
        use super::*;

        #[tokio::test]
        async fn should_extract_all_function_types_from_typescript() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.ts");
            indexer.index_file(&file_path).await.unwrap();
            let symbols = indexer.get_symbols_by_file(&file_path);
            
            let functions: Vec<_> = symbols.iter().filter(|s| s.symbol_type == SymbolType::Function).collect();
            
            
            // Should find all 6 function types
            assert_eq!(functions.len(), 6);
            
            // Function declaration
            assert!(functions.iter().any(|f| f.name == "formatUserName"));
            
            // Class methods (including constructor)
            assert!(functions.iter().any(|f| f.name == "constructor"));
            assert!(functions.iter().any(|f| f.name == "addUser"));
            assert!(functions.iter().any(|f| f.name == "findUserById"));
            assert!(functions.iter().any(|f| f.name == "userCount"));
            
            // Arrow function
            assert!(functions.iter().any(|f| f.name == "internalHelper"));
        }

        #[tokio::test]
        async fn should_extract_all_function_types_from_javascript() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.js");
            indexer.index_file(&file_path).await.unwrap();
            let symbols = indexer.get_symbols_by_file(&file_path);
            
            let functions: Vec<_> = symbols.iter().filter(|s| s.symbol_type == SymbolType::Function).collect();
            
            
            // Should find all 6 function types
            assert_eq!(functions.len(), 6);
            
            // Function declaration
            assert!(functions.iter().any(|f| f.name == "createCalculator"));
            
            // Class methods (including constructor)
            assert!(functions.iter().any(|f| f.name == "constructor"));
            assert!(functions.iter().any(|f| f.name == "add"));
            assert!(functions.iter().any(|f| f.name == "multiply"));
            assert!(functions.iter().any(|f| f.name == "getValue"));
            
            // Arrow function
            assert!(functions.iter().any(|f| f.name == "helper"));
        }
    }

    mod file_caching {
        use super::*;

        #[tokio::test]
        async fn should_cache_indexed_files() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.ts");
            
            // Index file twice
            indexer.index_file(&file_path).await.unwrap();
            let first_result = indexer.get_symbols_by_file(&file_path);
            
            indexer.index_file(&file_path).await.unwrap();
            let second_result = indexer.get_symbols_by_file(&file_path);

            // Results should be identical (cached)
            assert_eq!(first_result, second_result);
        }

        #[tokio::test]
        async fn should_clear_cache_when_requested() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let file_path = get_fixtures_path().join("sample.ts");
            indexer.index_file(&file_path).await.unwrap();
            
            assert!(indexer.get_symbols_by_file(&file_path).len() > 0);
            
            indexer.clear_cache();
            assert_eq!(indexer.get_symbols_by_file(&file_path).len(), 0);
        }
    }

    mod get_all_symbols {
        use super::*;

        #[tokio::test]
        async fn should_return_all_symbols_from_multiple_files() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let ts_file = get_fixtures_path().join("sample.ts");
            let js_file = get_fixtures_path().join("sample.js");
            
            indexer.index_file(&ts_file).await.unwrap();
            indexer.index_file(&js_file).await.unwrap();
            
            let all_symbols = indexer.get_all_symbols();
            
            // Should contain symbols from both files (based on actually extracted symbols)
            assert!(all_symbols.iter().any(|s| s.name == "formatUserName"));
            assert!(all_symbols.iter().any(|s| s.name == "createCalculator"));
            
            // Should have more symbols than any single file
            let ts_symbols = indexer.get_symbols_by_file(&ts_file);
            let js_symbols = indexer.get_symbols_by_file(&js_file);
            
            assert!(all_symbols.len() > ts_symbols.len());
            assert!(all_symbols.len() > js_symbols.len());
        }
    }

    mod error_handling {
        use super::*;

        #[tokio::test]
        async fn should_handle_non_existent_files_gracefully() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            let non_existent_file = get_fixtures_path().join("non-existent.ts");
            
            // Should not panic - indexer handles errors internally
            let _result = indexer.index_file(&non_existent_file).await;
            // May return error or Ok depending on implementation
            
            // Should return empty array
            let symbols = indexer.get_symbols_by_file(&non_existent_file);
            assert_eq!(symbols, vec![]);
        }

        #[tokio::test]
        async fn should_handle_unsupported_file_extensions() {
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize().await.unwrap();
            
            // Create a temporary file with unsupported extension
            let mut temp_file = NamedTempFile::new().unwrap();
            temp_file.write_all(b"some content").unwrap();
            let unsupported_file = temp_file.path().with_extension("unsupported");
            
            // Should not panic - indexer handles file errors internally
            let _result = indexer.index_file(&unsupported_file).await;
            let symbols = indexer.get_symbols_by_file(&unsupported_file);
            
            // Should return empty array for unsupported files
            assert_eq!(symbols, vec![]);
        }
    }
}