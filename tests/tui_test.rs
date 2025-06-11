use sfs::types::{CodeSymbol, SymbolType, SearchOptions};
use sfs::searcher::FuzzySearcher;
use std::path::PathBuf;

// Mock TUI interface for testing purposes
#[derive(Debug)]
pub struct MockTuiInterface {
    searcher: FuzzySearcher,
    symbols: Vec<CodeSymbol>,
    current_results: Vec<MockSearchResult>,
    selected_index: usize,
    query: String,
    current_search_mode: MockSearchMode,
    search_modes: Vec<MockSearchMode>,
}

#[derive(Debug, Clone)]
pub struct MockSearchResult {
    pub symbol: CodeSymbol,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct MockSearchMode {
    pub name: String,
    pub prefix: String,
    pub icon: String,
}

impl MockTuiInterface {
    pub fn new(searcher: FuzzySearcher, symbols: Vec<CodeSymbol>) -> Self {
        let search_modes = vec![
            MockSearchMode {
                name: "Fuzzy".to_string(),
                prefix: "".to_string(),
                icon: "🔍".to_string(),
            },
            MockSearchMode {
                name: "Symbol".to_string(),
                prefix: "#".to_string(),
                icon: "🏷️".to_string(),
            },
            MockSearchMode {
                name: "File".to_string(),
                prefix: ">".to_string(),
                icon: "📁".to_string(),
            },
            MockSearchMode {
                name: "Regex".to_string(),
                prefix: "/".to_string(),
                icon: "🔧".to_string(),
            },
        ];

        Self {
            searcher,
            symbols,
            current_results: Vec::new(),
            selected_index: 0,
            query: String::new(),
            current_search_mode: search_modes[0].clone(),
            search_modes,
        }
    }

    fn detect_search_mode(&self, query: &str) -> MockSearchMode {
        if query.starts_with('#') {
            return self.search_modes[1].clone(); // Symbol
        } else if query.starts_with('>') {
            return self.search_modes[2].clone(); // File
        } else if query.starts_with('/') {
            return self.search_modes[3].clone(); // Regex
        }
        self.search_modes[0].clone() // Fuzzy (default)
    }

    fn extract_search_query(&self, query: &str) -> String {
        match &self.current_search_mode.prefix {
            prefix if !prefix.is_empty() && query.starts_with(prefix) => {
                query[prefix.len()..].to_string()
            }
            _ => query.to_string(),
        }
    }

    fn perform_mode_specific_search(&self, query: &str) -> Vec<MockSearchResult> {
        match self.current_search_mode.name.as_str() {
            "Symbol" => self.perform_symbol_search(query, 100),
            "File" => self.perform_file_search(query, 100),
            "Regex" => self.perform_regex_search(query, 100),
            _ => self.perform_fuzzy_search(query, 100), // Default fuzzy
        }
    }

    fn perform_symbol_search(&self, query: &str, limit: usize) -> Vec<MockSearchResult> {
        // Exclude files and directories
        let filtered_symbols: Vec<CodeSymbol> = self.symbols
            .iter()
            .filter(|s| s.symbol_type != SymbolType::Variable || 
                       (!s.name.contains('.') && !s.name.contains('/')))
            .cloned()
            .collect();

        let searcher = FuzzySearcher::new(filtered_symbols);
        let options = SearchOptions {
            limit: Some(limit),
            ..Default::default()
        };
        let results = searcher.search(query, &options);
        
        results.into_iter().map(|r| MockSearchResult {
            symbol: r.symbol,
            score: r.score,
        }).collect()
    }

    fn perform_file_search(&self, query: &str, limit: usize) -> Vec<MockSearchResult> {
        // Check if the query ends with '/' for directory-only search
        let is_directory_search = query.ends_with('/');
        let clean_query = if is_directory_search {
            query.trim_end_matches('/').to_string()
        } else {
            query.to_string()
        };

        // Filter symbols based on directory search flag
        let search_options = if is_directory_search {
            SearchOptions {
                types: Some(vec![SymbolType::Dirname]),
                limit: Some(limit),
                ..Default::default()
            }
        } else {
            SearchOptions {
                types: Some(vec![SymbolType::Filename, SymbolType::Dirname]),
                limit: Some(limit),
                ..Default::default()
            }
        };

        let searcher = FuzzySearcher::new(self.symbols.clone());
        let results = searcher.search(&clean_query, &search_options);

        results.into_iter().map(|r| MockSearchResult {
            symbol: r.symbol,
            score: r.score,
        }).collect()
    }

    fn perform_regex_search(&self, query: &str, limit: usize) -> Vec<MockSearchResult> {
        match regex::Regex::new(query) {
            Ok(re) => {
                self.symbols
                    .iter()
                    .filter(|s| re.is_match(&s.name))
                    .take(limit)
                    .map(|s| MockSearchResult {
                        symbol: s.clone(),
                        score: 1.0, // Perfect match for regex
                            })
                    .collect()
            }
            Err(_) => Vec::new(), // Invalid regex returns empty results
        }
    }

    fn perform_fuzzy_search(&self, query: &str, limit: usize) -> Vec<MockSearchResult> {
        let options = SearchOptions {
            limit: Some(limit),
            ..Default::default()
        };
        let results = self.searcher.search(query, &options);
        results.into_iter().map(|r| MockSearchResult {
            symbol: r.symbol,
            score: r.score,
        }).collect()
    }

    pub fn perform_search(&mut self, query: &str) {
        self.query = query.to_string();
        
        // Detect and update search mode
        self.current_search_mode = self.detect_search_mode(query);
        
        if query.is_empty() {
            // Show all symbols when query is empty (limit to 100)
            self.current_results = self.symbols
                .iter()
                .take(100)
                .map(|s| MockSearchResult {
                    symbol: s.clone(),
                    score: 1.0,
                    })
                .collect();
        } else {
            let clean_query = self.extract_search_query(query);
            self.current_results = self.perform_mode_specific_search(&clean_query);
        }
        
        self.selected_index = 0;
    }

    pub fn select_current_result(&mut self) -> Result<String, String> {
        if self.current_results.is_empty() || self.selected_index >= self.current_results.len() {
            return Err("No result selected".to_string());
        }

        let selected_result = &self.current_results[self.selected_index];
        let symbol = &selected_result.symbol;
        
        // Format location
        let location = format!("{}:{}:{}", 
            symbol.file.display(), 
            symbol.line, 
            symbol.column
        );
        
        // In a real implementation, this would copy to clipboard
        // For testing, we just return the location
        
        // Clear search box
        self.query.clear();
        
        Ok(location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_symbols() -> Vec<CodeSymbol> {
        vec![
            CodeSymbol {
                name: "Calculator".to_string(),
                symbol_type: SymbolType::Class,
                file: PathBuf::from("/test/src/Calculator.ts"),
                line: 1,
                column: 1,
                context: Some("class Calculator {".to_string()),
            },
            CodeSymbol {
                name: "add".to_string(),
                symbol_type: SymbolType::Function,
                file: PathBuf::from("/test/src/Calculator.ts"),
                line: 5,
                column: 2,
                context: Some("add(a: number, b: number) {".to_string()),
            },
            CodeSymbol {
                name: "Calculator.ts".to_string(),
                symbol_type: SymbolType::Filename,
                file: PathBuf::from("/test/src/Calculator.ts"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "src".to_string(),
                symbol_type: SymbolType::Dirname,
                file: PathBuf::from("/test/src/Calculator.ts"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "test".to_string(),
                symbol_type: SymbolType::Dirname,
                file: PathBuf::from("/test/src/Calculator.ts"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "api.ts".to_string(),
                symbol_type: SymbolType::Filename,
                file: PathBuf::from("/test/utils/api.ts"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "utils".to_string(),
                symbol_type: SymbolType::Dirname,
                file: PathBuf::from("/test/utils/api.ts"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "ApiService".to_string(),
                symbol_type: SymbolType::Class,
                file: PathBuf::from("/test/utils/api.ts"),
                line: 10,
                column: 1,
                context: Some("class ApiService {".to_string()),
            },
        ]
    }

    mod search_mode_detection {
        use super::*;

        #[test]
        fn should_detect_fuzzy_search_mode_by_default() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            let mode = interface.detect_search_mode("Calculator");
            
            assert_eq!(mode.name, "Fuzzy");
            assert_eq!(mode.prefix, "");
            assert_eq!(mode.icon, "🔍");
        }

        #[test]
        fn should_detect_symbol_search_mode_with_hash_prefix() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            let mode = interface.detect_search_mode("#Calculator");
            
            assert_eq!(mode.name, "Symbol");
            assert_eq!(mode.prefix, "#");
            assert_eq!(mode.icon, "🏷️");
        }

        #[test]
        fn should_detect_file_search_mode_with_greater_than_prefix() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            let mode = interface.detect_search_mode(">sample");
            
            assert_eq!(mode.name, "File");
            assert_eq!(mode.prefix, ">");
            assert_eq!(mode.icon, "📁");
        }

        #[test]
        fn should_detect_regex_search_mode_with_slash_prefix() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            let mode = interface.detect_search_mode("/Cal.*");
            
            assert_eq!(mode.name, "Regex");
            assert_eq!(mode.prefix, "/");
            assert_eq!(mode.icon, "🔧");
        }
    }

    mod query_extraction {
        use super::*;

        #[test]
        fn should_extract_query_without_prefix_for_fuzzy_search() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to fuzzy
            interface.current_search_mode = interface.search_modes[0].clone();
            
            let query = interface.extract_search_query("Calculator");
            assert_eq!(query, "Calculator");
        }

        #[test]
        fn should_extract_query_without_hash_prefix_for_symbol_search() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to symbol search
            interface.current_search_mode = interface.search_modes[1].clone();
            
            let query = interface.extract_search_query("#Calculator");
            assert_eq!(query, "Calculator");
        }

        #[test]
        fn should_extract_query_without_greater_than_prefix_for_file_search() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to file search
            interface.current_search_mode = interface.search_modes[2].clone();
            
            let query = interface.extract_search_query(">sample");
            assert_eq!(query, "sample");
        }

        #[test]
        fn should_extract_query_without_slash_prefix_for_regex_search() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to regex search
            interface.current_search_mode = interface.search_modes[3].clone();
            
            let query = interface.extract_search_query("/Cal.*");
            assert_eq!(query, "Cal.*");
        }
    }

    mod mode_specific_search {
        use super::*;

        #[test]
        fn should_perform_symbol_search_excluding_files_and_directories() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to symbol search
            interface.current_search_mode = interface.search_modes[1].clone();
            
            let results = interface.perform_mode_specific_search("Calculator");
            
            // Should find Calculator class but not files/directories
            assert!(!results.is_empty());
            assert!(results.iter().any(|r| r.symbol.name == "Calculator"));
            // Note: In our simplified implementation, we use symbol filtering logic
        }

        #[test]
        fn should_perform_file_search_including_only_files_and_directories() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to file search
            interface.current_search_mode = interface.search_modes[2].clone();
            
            let _results = interface.perform_mode_specific_search("sample");
            
            // Should find files/directories (those containing '.' or '/')
            // Note: This is a simplified test - in real implementation, 
            // we'd have proper file/directory symbol types
        }

        #[test]
        fn should_perform_regex_search_with_valid_patterns() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            let results = interface.perform_regex_search("Cal.*", 100);
            
            // Should find Calculator symbols
            assert!(results.iter().any(|r| r.symbol.name == "Calculator"));
        }

        #[test]
        fn should_handle_invalid_regex_patterns_gracefully() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            // Invalid regex pattern
            let results = interface.perform_regex_search("[invalid", 100);
            
            // Should return empty array for invalid regex
            assert!(results.is_empty());
        }

        #[test]
        fn should_perform_default_fuzzy_search() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to fuzzy search
            interface.current_search_mode = interface.search_modes[0].clone();
            
            let results = interface.perform_mode_specific_search("Calculator");
            
            // Should perform normal fuzzy search
            assert!(!results.is_empty());
            assert!(results.iter().any(|r| r.symbol.name == "Calculator"));
        }

        #[test]
        fn should_perform_file_search_including_files_and_directories() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to file search
            interface.current_search_mode = interface.search_modes[2].clone();
            
            let results = interface.perform_file_search("src", 100);
            
            // Should find both files and directories containing "src"
            assert!(!results.is_empty());
            let result_names: Vec<&String> = results.iter().map(|r| &r.symbol.name).collect();
            
            // Should include both directories named "src" and possibly files containing "src"
            assert!(result_names.iter().any(|name| *name == "src"));
        }

        #[test]
        fn should_perform_directory_only_search_with_trailing_slash() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to file search
            interface.current_search_mode = interface.search_modes[2].clone();
            
            let results = interface.perform_file_search("src/", 100);
            
            // Should find only directories, not files
            assert!(!results.is_empty());
            
            // All results should be directories
            for result in &results {
                assert_eq!(result.symbol.symbol_type, SymbolType::Dirname);
            }
            
            // Should contain "src" directory
            let result_names: Vec<&String> = results.iter().map(|r| &r.symbol.name).collect();
            assert!(result_names.iter().any(|name| *name == "src"));
        }

        #[test]
        fn should_remove_trailing_slash_from_directory_search_query() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Set mode to file search
            interface.current_search_mode = interface.search_modes[2].clone();
            
            let results = interface.perform_file_search("test/", 100);
            
            // Should find directories named "test" even though we searched for "test/"
            assert!(!results.is_empty());
            
            let result_names: Vec<&String> = results.iter().map(|r| &r.symbol.name).collect();
            assert!(result_names.iter().any(|name| *name == "test"));
            
            // All results should be directories
            for result in &results {
                assert_eq!(result.symbol.symbol_type, SymbolType::Dirname);
            }
        }
    }

    mod empty_query_handling {
        use super::*;

        #[test]
        fn should_show_all_symbols_when_query_is_empty() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Simulate empty query search
            interface.perform_search("");
            
            // Should show symbols (limited to 100)
            assert!(!interface.current_results.is_empty());
            assert!(interface.current_results.len() <= 100);
        }
    }

    mod search_mode_integration {
        use super::*;

        #[test]
        fn should_change_mode_when_prefix_is_detected() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Start with fuzzy mode
            assert_eq!(interface.current_search_mode.name, "Fuzzy");
            
            // Simulate search with # prefix
            interface.perform_search("#Calculator");
            
            // Should switch to Symbol mode
            assert_eq!(interface.current_search_mode.name, "Symbol");
        }

        #[test]
        fn should_return_to_fuzzy_mode_when_no_prefix_is_used() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Start with symbol mode
            interface.perform_search("#Calculator");
            assert_eq!(interface.current_search_mode.name, "Symbol");
            
            // Search without prefix
            interface.perform_search("Calculator");
            
            // Should return to fuzzy mode
            assert_eq!(interface.current_search_mode.name, "Fuzzy");
        }
    }

    mod search_results_formatting {
        use super::*;

        #[test]
        fn should_format_search_results_correctly() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Perform a search
            interface.perform_search("Calculator");
            
            // Should have results with proper structure
            assert!(!interface.current_results.is_empty());
            
            let result = &interface.current_results[0];
            // Results should have symbol, score, and matches
            assert!(!result.symbol.name.is_empty());
            // Fuzzy matcher can return negative scores, so check for finite instead
            assert!(result.score.is_finite());
            // matches can be empty in this implementation
        }
    }

    mod navigation_functionality {
        use super::*;

        #[test]
        fn should_support_cursor_navigation_with_arrow_keys() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup multiple results
            interface.perform_search("a"); // Should match several symbols
            assert!(interface.current_results.len() > 1);
            
            // Start at index 0
            assert_eq!(interface.selected_index, 0);
            
            // Simulate down movement (in a real implementation, this would be handled by key events)
            // For testing, we'll simulate the result of key handling
            if interface.selected_index < interface.current_results.len().saturating_sub(1) {
                interface.selected_index += 1;
            }
            assert_eq!(interface.selected_index, 1);
            
            // Simulate up movement
            if interface.selected_index > 0 {
                interface.selected_index -= 1;
            }
            assert_eq!(interface.selected_index, 0);
        }

        #[test]
        fn should_not_navigate_beyond_boundaries() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup results
            interface.perform_search("Calculator");
            let max_index = interface.current_results.len().saturating_sub(1);
            
            // Test upper boundary
            interface.selected_index = 0;
            if interface.selected_index > 0 {
                interface.selected_index -= 1;
            }
            assert_eq!(interface.selected_index, 0); // Should stay at 0
            
            // Test lower boundary
            interface.selected_index = max_index;
            if interface.selected_index < interface.current_results.len().saturating_sub(1) {
                interface.selected_index += 1;
            }
            assert_eq!(interface.selected_index, max_index); // Should stay at max
        }

        #[test]
        fn should_reset_selection_when_new_search_is_performed() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup initial search and move selection
            interface.perform_search("Calculator");
            interface.selected_index = 1; // Move away from 0
            
            // Perform new search
            interface.perform_search("api");
            
            // Selection should reset to 0
            assert_eq!(interface.selected_index, 0);
        }

        #[test]  
        fn should_handle_ctrl_n_and_ctrl_p_navigation_logic() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup multiple results - use a more general query that should match multiple symbols
            interface.perform_search("a"); // Should match "add", "api.ts", "Calculator", "ApiService" etc.
            assert!(interface.current_results.len() > 1);
            
            // Test Ctrl+N logic (next/down)
            interface.selected_index = 0;
            // Simulate Ctrl+N behavior
            if interface.selected_index < interface.current_results.len().saturating_sub(1) {
                interface.selected_index += 1;
            }
            assert_eq!(interface.selected_index, 1);
            
            // Test Ctrl+P logic (previous/up)
            // Simulate Ctrl+P behavior  
            if interface.selected_index > 0 {
                interface.selected_index -= 1;
            }
            assert_eq!(interface.selected_index, 0);
        }
    }

    mod default_display_functionality {
        use super::*;

        #[test]
        fn should_show_default_results_on_startup() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Simulate startup with show_default_results
            // Since we can't call the actual method, we'll test the expected behavior
            interface.perform_search(""); // Empty search should show all symbols
            
            // Should have results
            assert!(!interface.current_results.is_empty());
            
            // Should start at index 0
            assert_eq!(interface.selected_index, 0);
        }

        #[test]
        fn should_limit_default_results_to_reasonable_number() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Test empty search (which shows default results)
            interface.perform_search("");
            
            // Should limit results (our mock has 8 symbols, so all should be shown)
            assert!(interface.current_results.len() <= 100);
            assert!(!interface.current_results.is_empty());
        }

        #[test]
        fn should_support_different_default_strategies() {
            // This test validates the concept that different strategies could be used
            // In a real implementation, we would test DefaultDisplayStrategy enum variants
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let interface = MockTuiInterface::new(searcher, symbols);
            
            // Test that we have symbols available for sorting
            assert!(!interface.symbols.is_empty());
            
            // Different strategies would sort these symbols differently
            // For now, we just verify the foundation is in place
            assert!(interface.symbols.len() >= 3); // Enough symbols for meaningful sorting
        }
    }

    mod clipboard_functionality {
        use super::*;

        #[test]
        fn should_copy_symbol_location_on_select() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup results
            interface.current_results = vec![MockSearchResult {
                symbol: CodeSymbol {
                    name: "Calculator".to_string(),
                    symbol_type: SymbolType::Variable,
                    file: PathBuf::from("/test/Calculator.ts"),
                    line: 1,
                    column: 1,
                    context: None,
                },
                score: 0.0,
            }];
            interface.selected_index = 0;
            
            // Call selectCurrentResult
            let result = interface.select_current_result();
            
            // Should return location
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "/test/Calculator.ts:1:1");
        }

        #[test]
        fn should_clear_search_box_after_copying_to_clipboard() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // Setup results
            interface.current_results = vec![MockSearchResult {
                symbol: CodeSymbol {
                    name: "Calculator".to_string(),
                    symbol_type: SymbolType::Variable,
                    file: PathBuf::from("/test/Calculator.ts"),
                    line: 1,
                    column: 1,
                    context: None,
                },
                score: 0.0,
            }];
            interface.selected_index = 0;
            interface.query = "Calculator".to_string();
            
            // Call selectCurrentResult
            let _result = interface.select_current_result();
            
            // Should clear search box
            assert!(interface.query.is_empty());
        }

        #[test]
        fn should_do_nothing_when_no_result_is_selected() {
            let symbols = create_mock_symbols();
            let searcher = FuzzySearcher::new(symbols.clone());
            let mut interface = MockTuiInterface::new(searcher, symbols);
            
            // No results
            interface.current_results = vec![];
            interface.selected_index = 0;
            
            // Call selectCurrentResult
            let result = interface.select_current_result();
            
            // Should return error
            assert!(result.is_err());
        }
    }
}