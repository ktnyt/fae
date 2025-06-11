use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::path::PathBuf;

fn create_test_symbols() -> Vec<CodeSymbol> {
    vec![
        CodeSymbol {
            name: "test.txt".to_string(),
            symbol_type: SymbolType::Filename,
            file: PathBuf::from("tests/fixtures/test.txt"),
            line: 1,
            column: 1,
            context: None,
        },
        CodeSymbol {
            name: "sample.ts".to_string(),
            symbol_type: SymbolType::Filename,
            file: PathBuf::from("tests/fixtures/sample.ts"),
            line: 1,
            column: 1,
            context: None,
        },
    ]
}

#[cfg(test)]
mod content_search_tests {
    use super::*;

    #[test]
    fn should_search_file_contents() {
        let symbols = create_test_symbols();
        let searcher = FuzzySearcher::new(symbols);
        let options = SearchOptions::default();

        // Search for content that exists in sample.ts
        let results = searcher.search_content("export", &options);
        
        // Should find lines containing "export"
        assert!(!results.is_empty(), "Should find content matches");
        
        // Check that results contain file paths and line numbers
        for result in &results {
            assert!(result.symbol.line > 0, "Should have valid line number");
            
            // File path should be one of our test fixtures (could be absolute or relative)
            let file_str = result.symbol.file.to_string_lossy();
            let is_valid_file = file_str.contains("sample.ts") || file_str.ends_with("sample.ts") ||
                               file_str.contains("sample.js") || file_str.ends_with("sample.js") ||
                               file_str.contains("test.txt") || file_str.ends_with("test.txt");
            assert!(
                is_valid_file,
                "Should reference a test fixture file, got: {}", file_str
            );
        }
    }

    #[test]
    fn should_not_search_non_existent_content() {
        let symbols = create_test_symbols();
        let searcher = FuzzySearcher::new(symbols);
        let options = SearchOptions::default();

        // Search for content that doesn't exist
        let results = searcher.search_content("nonexistentcontent12345", &options);
        
        // Should find no matches
        assert!(results.is_empty(), "Should find no matches for non-existent content");
    }

    #[test]
    fn should_handle_empty_query() {
        let symbols = create_test_symbols();
        let searcher = FuzzySearcher::new(symbols);
        let options = SearchOptions::default();

        let results = searcher.search_content("", &options);
        
        // Should return empty results for empty query
        assert!(results.is_empty(), "Should return empty results for empty query");
    }

    #[test]
    fn should_respect_search_options_limit() {
        let symbols = create_test_symbols();
        let searcher = FuzzySearcher::new(symbols);
        let options = SearchOptions {
            limit: Some(2),
            ..Default::default()
        };

        // Search for a common word that might have many matches
        let results = searcher.search_content("function", &options);
        
        // Should respect the limit
        assert!(results.len() <= 2, "Should respect the limit option");
    }
}