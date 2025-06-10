// TypeScript fuzzy-searcher.test.ts をRustに移植
// 目標: 15つのテストすべてをパスする

use sfs_rs::types::*;
use sfs_rs::searcher::FuzzySearcher;
use std::path::PathBuf;

// Mock symbols for testing (TypeScriptテストと同じデータ)
fn create_mock_symbols() -> Vec<CodeSymbol> {
    vec![
        CodeSymbol {
            name: "getUserById".to_string(),
            symbol_type: SymbolType::Function,
            file: "/src/user.ts".into(),
            line: 10,
            column: 1,
            context: Some("function getUserById(id: number) {".to_string()),
        },
        CodeSymbol {
            name: "UserManager".to_string(),
            symbol_type: SymbolType::Class,
            file: "/src/user.ts".into(),
            line: 5,
            column: 1,
            context: Some("class UserManager {".to_string()),
        },
        CodeSymbol {
            name: "User".to_string(),
            symbol_type: SymbolType::Interface,
            file: "/src/types.ts".into(),
            line: 1,
            column: 1,
            context: Some("interface User {".to_string()),
        },
        CodeSymbol {
            name: "createUser".to_string(),
            symbol_type: SymbolType::Function,
            file: "/src/user.ts".into(),
            line: 20,
            column: 1,
            context: Some("function createUser(data: UserData) {".to_string()),
        },
        CodeSymbol {
            name: "deleteUser".to_string(),
            symbol_type: SymbolType::Function,
            file: "/src/user.ts".into(),
            line: 30,
            column: 1,
            context: Some("function deleteUser(id: number) {".to_string()),
        },
        CodeSymbol {
            name: "API_BASE_URL".to_string(),
            symbol_type: SymbolType::Constant,
            file: "/src/config.ts".into(),
            line: 1,
            column: 1,
            context: Some("const API_BASE_URL = 'https://api.example.com';".to_string()),
        },
        CodeSymbol {
            name: "user.ts".to_string(),
            symbol_type: SymbolType::Filename,
            file: "/src/user.ts".into(),
            line: 1,
            column: 1,
            context: None,
        },
        CodeSymbol {
            name: "src".to_string(),
            symbol_type: SymbolType::Dirname,
            file: "/src/user.ts".into(),
            line: 1,
            column: 1,
            context: None,
        },
    ]
}

#[cfg(test)]
mod fuzzy_searcher {
    use super::*;

    mod basic_search_functionality {
        use super::*;

        #[test]
        fn should_find_exact_matches() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("User", &SearchOptions::default());
            
            assert!(results.len() > 0);
            
            // Should find User interface with high score
            let user_interface = results.iter().find(|r| r.symbol.name == "User");
            assert!(user_interface.is_some());
            assert!(user_interface.unwrap().score < 0.1); // Very low score = very good match
        }

        #[test]
        fn should_find_fuzzy_matches() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("User", &SearchOptions::default());
            
            // Should find UserManager with partial match
            let user_manager = results.iter().find(|r| r.symbol.name == "UserManager");
            assert!(user_manager.is_some());
        }

        #[test]
        fn should_return_empty_array_for_no_matches() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("NonExistentSymbol12345", &SearchOptions::default());
            assert_eq!(results, vec![]);
        }

        #[test]
        fn should_handle_empty_search_query() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("", &SearchOptions::default());
            assert_eq!(results, vec![]);
        }
    }

    mod search_options {
        use super::*;

        #[test]
        fn should_limit_results_when_limit_option_is_provided() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let options = SearchOptions {
                limit: Some(2),
                ..Default::default()
            };
            let results = searcher.search("user", &options);
            
            assert!(results.len() <= 2);
        }

        #[test]
        fn should_filter_by_symbol_types() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let options = SearchOptions {
                types: Some(vec![SymbolType::Function]),
                ..Default::default()
            };
            let results = searcher.search("user", &options);
            
            // All results should be functions
            for result in &results {
                assert_eq!(result.symbol.symbol_type, SymbolType::Function);
            }
        }

        #[test]
        fn should_filter_by_multiple_symbol_types() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let options = SearchOptions {
                types: Some(vec![SymbolType::Function, SymbolType::Class]),
                ..Default::default()
            };
            let results = searcher.search("user", &options);
            
            // All results should be functions or classes
            for result in &results {
                assert!(matches!(result.symbol.symbol_type, SymbolType::Function | SymbolType::Class));
            }
        }

        #[test]
        fn should_exclude_files_when_include_files_is_false() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let options = SearchOptions {
                include_files: Some(false),
                ..Default::default()
            };
            let results = searcher.search("user", &options);
            
            // Should not include filename results
            let has_filename = results.iter().any(|r| r.symbol.symbol_type == SymbolType::Filename);
            assert_eq!(has_filename, false);
        }

        #[test]
        fn should_exclude_directories_when_include_dirs_is_false() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let options = SearchOptions {
                include_dirs: Some(false),
                ..Default::default()
            };
            let results = searcher.search("src", &options);
            
            // Should not include dirname results
            let has_dirname = results.iter().any(|r| r.symbol.symbol_type == SymbolType::Dirname);
            assert_eq!(has_dirname, false);
        }

        #[test]
        fn should_respect_threshold_option() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let strict_options = SearchOptions {
                threshold: Some(0.1), // Very strict
                ..Default::default()
            };
            let loose_options = SearchOptions {
                threshold: Some(0.8), // Very loose
                ..Default::default()
            };
            
            let strict_results = searcher.search("usrmng", &strict_options);
            let loose_results = searcher.search("usrmng", &loose_options);
            
            // Loose search should return more results
            assert!(loose_results.len() >= strict_results.len());
        }
    }

    mod result_scoring {
        use super::*;

        #[test]
        fn should_return_results_sorted_by_relevance() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("user", &SearchOptions::default());
            
            // Results should be sorted by score (ascending = better match first)
            for i in 1..results.len() {
                assert!(results[i].score >= results[i - 1].score);
            }
        }

        #[test]
        fn should_give_exact_matches_better_scores() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("User", &SearchOptions::default());
            
            // Find exact match
            let exact_match = results.iter().find(|r| r.symbol.name == "User");
            
            // Find partial matches
            let partial_matches: Vec<_> = results.iter()
                .filter(|r| r.symbol.name != "User" && r.symbol.name.contains("User"))
                .collect();
            
            if let (Some(exact), Some(partial)) = (exact_match, partial_matches.get(0)) {
                // Exact match should have better (lower) score
                assert!(exact.score < partial.score);
            }
        }
    }

    mod symbol_updates {
        use super::*;

        #[test]
        fn should_update_symbols_and_search_in_new_set() {
            let symbols = create_mock_symbols();
            let mut searcher = FuzzySearcher::new(symbols);
            
            let new_symbols = vec![
                CodeSymbol {
                    name: "Product".to_string(),
                    symbol_type: SymbolType::Interface,
                    file: "/src/product.ts".into(),
                    line: 1,
                    column: 1,
                    context: Some("interface Product {".to_string()),
                }
            ];
            
            searcher.update_symbols(new_symbols);
            
            // Should find new symbol
            let results = searcher.search("Product", &SearchOptions::default());
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].symbol.name, "Product");
            
            // Should not find old symbols
            let old_results = searcher.search("User", &SearchOptions::default());
            assert_eq!(old_results, vec![]);
        }
    }

    mod context_handling {
        use super::*;

        #[test]
        fn should_include_context_in_search_results() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols);
            
            let results = searcher.search("getUserById", &SearchOptions::default());
            
            let match_result = results.iter().find(|r| r.symbol.name == "getUserById");
            assert_eq!(match_result.unwrap().symbol.context, 
                      Some("function getUserById(id: number) {".to_string()));
        }

        #[test]
        fn should_handle_symbols_without_context() {
            let symbols_without_context = vec![
                CodeSymbol {
                    name: "TestSymbol".to_string(),
                    symbol_type: SymbolType::Variable,
                    file: "/test.ts".into(),
                    line: 1,
                    column: 1,
                    context: None,
                }
            ];
            
            let searcher = FuzzySearcher::new(symbols_without_context);
            let results = searcher.search("TestSymbol", &SearchOptions::default());
            
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].symbol.context, None);
        }
    }
}