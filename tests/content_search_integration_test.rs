use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod content_search_integration_tests {
    use super::*;

    fn create_test_project() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create test files with various content
        fs::write(
            temp_dir.path().join("main.rs"),
            r#"
use std::collections::HashMap;

fn main() {
    println!("Hello, world!");
    let mut map = HashMap::new();
    map.insert("key", "value");
}

struct Config {
    debug: bool,
    port: u16,
}

impl Config {
    fn new() -> Self {
        Self { debug: false, port: 8080 }
    }
}
"#,
        )
        .expect("Failed to write main.rs");

        fs::write(
            temp_dir.path().join("utils.js"),
            r#"
// JavaScript utility functions
function validateEmail(email) {
    return email.includes('@');
}

const API_URL = 'https://api.example.com';

class UserManager {
    constructor() {
        this.users = [];
    }
    
    addUser(user) {
        this.users.push(user);
        console.log('User added:', user);
    }
}
"#,
        )
        .expect("Failed to write utils.js");

        fs::write(
            temp_dir.path().join("data.py"),
            r#"
import json
import requests

def fetch_data():
    """Fetch data from API"""
    response = requests.get('https://api.example.com/data')
    return response.json()

class DataProcessor:
    def __init__(self):
        self.data = []
    
    def process(self, raw_data):
        # Process the raw data
        processed = [item for item in raw_data if item.get('valid')]
        return processed
"#,
        )
        .expect("Failed to write data.py");

        temp_dir
    }

    fn create_test_symbols(temp_dir: &TempDir) -> Vec<CodeSymbol> {
        vec![
            CodeSymbol {
                name: "main.rs".to_string(),
                symbol_type: SymbolType::Filename,
                file: temp_dir.path().join("main.rs"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "utils.js".to_string(),
                symbol_type: SymbolType::Filename,
                file: temp_dir.path().join("utils.js"),
                line: 1,
                column: 1,
                context: None,
            },
            CodeSymbol {
                name: "data.py".to_string(),
                symbol_type: SymbolType::Filename,
                file: temp_dir.path().join("data.py"),
                line: 1,
                column: 1,
                context: None,
            },
        ]
    }

    #[test]
    fn should_find_content_across_multiple_languages() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Test searching for "function" - should find content in JS and Python files
        let results = searcher.search_content("function", &SearchOptions::default());
        assert!(!results.is_empty(), "Should find 'function' content");

        // Should find JavaScript function definitions
        let js_results: Vec<_> = results
            .iter()
            .filter(|r| {
                r.symbol
                    .file
                    .extension()
                    .map(|ext| ext == "js")
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !js_results.is_empty(),
            "Should find 'function' in JavaScript files"
        );
    }

    #[test]
    fn should_find_imports_and_includes() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Test searching for "import" - should find in Python
        let results = searcher.search_content("import", &SearchOptions::default());
        assert!(!results.is_empty(), "Should find 'import' statements");

        // Should find import in Python file
        let python_results: Vec<_> = results
            .iter()
            .filter(|r| {
                r.symbol
                    .file
                    .extension()
                    .map(|ext| ext == "py")
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !python_results.is_empty(),
            "Should find 'import' in Python files"
        );
    }

    #[test]
    fn should_find_struct_definitions() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Test searching for "struct" - should find in Rust
        let results = searcher.search_content("struct", &SearchOptions::default());
        assert!(!results.is_empty(), "Should find 'struct' definitions");

        // Should find struct in Rust file
        let rust_results: Vec<_> = results
            .iter()
            .filter(|r| {
                r.symbol
                    .file
                    .extension()
                    .map(|ext| ext == "rs")
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            !rust_results.is_empty(),
            "Should find 'struct' in Rust files"
        );
    }

    #[test]
    fn should_handle_special_characters_safely() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Test searching with regex special characters
        let test_queries = vec![
            "HashMap::new",
            "console.log",
            "response.json()",
            "[item for item",
        ];

        for query in test_queries {
            let results = searcher.search_content(query, &SearchOptions::default());
            // Should not crash and should handle the query safely
            // Results may or may not be found depending on exact content matching
            println!("Query '{}' returned {} results", query, results.len());
        }
    }

    #[test]
    fn should_respect_limit_option() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Search for a common term that should return multiple results
        let options_with_limit = SearchOptions {
            limit: Some(2),
            ..Default::default()
        };

        let limited_results = searcher.search_content("the", &options_with_limit);
        assert!(limited_results.len() <= 2, "Should respect limit option");

        let unlimited_results = searcher.search_content("the", &SearchOptions::default());
        if unlimited_results.len() > 2 {
            assert_eq!(
                limited_results.len(),
                2,
                "Should return exactly 2 results when limited"
            );
        }
    }

    #[test]
    fn should_return_accurate_line_numbers() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        // Search for a specific string that we know the location of
        let results = searcher.search_content("Hello, world!", &SearchOptions::default());
        assert!(!results.is_empty(), "Should find 'Hello, world!' string");

        // Check that line numbers are reasonable (not 0, not way too high)
        for result in &results {
            assert!(result.symbol.line > 0, "Line numbers should be positive");
            assert!(
                result.symbol.line < 1000,
                "Line numbers should be reasonable for test files"
            );
        }
    }

    #[test]
    fn should_handle_empty_query_gracefully() {
        let temp_dir = create_test_project();
        let symbols = create_test_symbols(&temp_dir);
        let searcher = FuzzySearcher::new(symbols);

        let results = searcher.search_content("", &SearchOptions::default());
        assert_eq!(results.len(), 0, "Empty query should return no results");

        let whitespace_results = searcher.search_content("   ", &SearchOptions::default());
        assert_eq!(
            whitespace_results.len(),
            0,
            "Whitespace-only query should return no results"
        );
    }

    #[test]
    fn should_handle_missing_files_gracefully() {
        // Create symbols pointing to non-existent files
        let non_existent_symbols = vec![CodeSymbol {
            name: "missing.rs".to_string(),
            symbol_type: SymbolType::Filename,
            file: "/non/existent/path/missing.rs".into(),
            line: 1,
            column: 1,
            context: None,
        }];

        let searcher = FuzzySearcher::new(non_existent_symbols);
        let results = searcher.search_content("test", &SearchOptions::default());

        // Should not crash and should return empty results
        assert_eq!(results.len(), 0, "Should handle missing files gracefully");
    }
}
