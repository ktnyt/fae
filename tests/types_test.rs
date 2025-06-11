// TypeScript types.test.ts をRustに移植
// 目標: 8つのテストすべてをパスする

use sfs::types::*;
use std::path::PathBuf;

#[cfg(test)]
mod type_definitions {
    use super::*;

    mod code_symbol {
        use super::*;

        #[test]
        fn should_accept_valid_code_symbol_objects() {
            let symbol = CodeSymbol {
                name: "testFunction".to_string(),
                symbol_type: SymbolType::Function,
                file: "/src/test.ts".into(),
                line: 10,
                column: 5,
                context: Some("function testFunction() {".to_string()),
            };

            assert_eq!(symbol.name, "testFunction");
            assert_eq!(symbol.symbol_type, SymbolType::Function);
            assert_eq!(symbol.file, PathBuf::from("/src/test.ts"));
            assert_eq!(symbol.line, 10);
            assert_eq!(symbol.column, 5);
            assert_eq!(symbol.context, Some("function testFunction() {".to_string()));
        }

        #[test]
        fn should_accept_code_symbol_without_optional_context() {
            let symbol = CodeSymbol {
                name: "testVariable".to_string(),
                symbol_type: SymbolType::Variable,
                file: "/src/test.ts".into(),
                line: 5,
                column: 1,
                context: None,
            };

            assert_eq!(symbol.context, None);
        }
    }

    mod symbol_type {
        use super::*;

        #[test]
        fn should_include_all_expected_symbol_types() {
            let valid_types = vec![
                SymbolType::Function,
                SymbolType::Variable,
                SymbolType::Class,
                SymbolType::Interface,
                SymbolType::Type,
                SymbolType::Enum,
                SymbolType::Constant,
                SymbolType::Method,
                SymbolType::Property,
                SymbolType::Filename,
                SymbolType::Dirname,
            ];

            // TypeScriptテストと同様に、各型でシンボルを作成してテスト
            for symbol_type in valid_types {
                let symbol = CodeSymbol {
                    name: "test".to_string(),
                    symbol_type: symbol_type.clone(),
                    file: "/test.ts".into(),
                    line: 1,
                    column: 1,
                    context: None,
                };
                assert_eq!(symbol.symbol_type, symbol_type);
            }
        }
    }

    mod search_options {
        use super::*;

        #[test]
        fn should_accept_empty_options_object() {
            let options = SearchOptions::default();
            // デフォルト値の検証
            assert_eq!(options, SearchOptions::default());
        }

        #[test]
        fn should_accept_all_optional_properties() {
            let options = SearchOptions {
                include_files: Some(false),
                include_dirs: Some(true),
                types: Some(vec![SymbolType::Function, SymbolType::Class]),
                threshold: Some(0.5),
                limit: Some(10),
            };

            assert_eq!(options.include_files, Some(false));
            assert_eq!(options.include_dirs, Some(true));
            assert_eq!(options.types, Some(vec![SymbolType::Function, SymbolType::Class]));
            assert_eq!(options.threshold, Some(0.5));
            assert_eq!(options.limit, Some(10));
        }

        #[test]
        fn should_accept_partial_options() {
            let options1 = SearchOptions {
                limit: Some(5),
                ..Default::default()
            };
            let options2 = SearchOptions {
                types: Some(vec![SymbolType::Variable]),
                ..Default::default()
            };
            let options3 = SearchOptions {
                threshold: Some(0.2),
                include_files: Some(false),
                ..Default::default()
            };

            assert_eq!(options1.limit, Some(5));
            assert_eq!(options2.types, Some(vec![SymbolType::Variable]));
            assert_eq!(options3.threshold, Some(0.2));
            assert_eq!(options3.include_files, Some(false));
        }
    }

    mod search_result {
        use super::*;

        #[test]
        fn should_structure_search_results_correctly() {
            let symbol = CodeSymbol {
                name: "testSymbol".to_string(),
                symbol_type: SymbolType::Function,
                file: "/test.ts".into(),
                line: 1,
                column: 1,
                context: None,
            };

            let result = SearchResult {
                symbol: symbol.clone(),
                score: 0.25,
            };

            assert_eq!(result.symbol, symbol);
            assert_eq!(result.score, 0.25);
        }
    }

    mod indexed_file {
        use super::*;

        #[test]
        fn should_structure_indexed_file_data_correctly() {
            let symbols = vec![CodeSymbol {
                name: "testFunction".to_string(),
                symbol_type: SymbolType::Function,
                file: "/test.ts".into(),
                line: 1,
                column: 1,
                context: None,
            }];

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            let indexed_file = IndexedFile {
                path: "/test.ts".into(),
                symbols: symbols.clone(),
                last_modified: now,
            };

            assert_eq!(indexed_file.path, PathBuf::from("/test.ts"));
            assert_eq!(indexed_file.symbols, symbols);
            assert!(indexed_file.last_modified > 0);
        }
    }
}