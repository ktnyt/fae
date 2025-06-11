use sfs::{
    mode::{
        detector::{ModeDetector, ModeType}, 
        ContentMode, FileMode, RegexMode, SearchMode, SearchModeManager, SymbolMode
    },
    searcher::SearchManager,
    types::{CodeSymbol, SearchOptions, SymbolType},
};
use std::path::PathBuf;

fn create_test_symbols() -> Vec<CodeSymbol> {
    vec![
        CodeSymbol {
            name: "test_function".to_string(),
            symbol_type: SymbolType::Function,
            file: PathBuf::from("src/main.rs"),
            line: 10,
            column: 1,
            context: Some("fn test_function() {}".to_string()),
        },
        CodeSymbol {
            name: "main.rs".to_string(),
            symbol_type: SymbolType::Filename,
            file: PathBuf::from("src/main.rs"),
            line: 1,
            column: 1,
            context: None,
        },
        CodeSymbol {
            name: "TestClass".to_string(),
            symbol_type: SymbolType::Class,
            file: PathBuf::from("src/lib.rs"),
            line: 5,
            column: 1,
            context: Some("class TestClass {}".to_string()),
        },
    ]
}

#[cfg(test)]
mod mode_detector_tests {
    use super::*;

    #[test]
    fn should_detect_symbol_mode_for_hash_prefix() {
        assert_eq!(ModeDetector::detect_mode_type("#test"), ModeType::Symbol);
    }

    #[test]
    fn should_detect_file_mode_for_greater_prefix() {
        assert_eq!(ModeDetector::detect_mode_type(">test"), ModeType::File);
    }

    #[test]
    fn should_detect_regex_mode_for_slash_prefix() {
        assert_eq!(ModeDetector::detect_mode_type("/test"), ModeType::Regex);
    }

    #[test]
    fn should_detect_content_mode_for_no_prefix() {
        assert_eq!(ModeDetector::detect_mode_type("test"), ModeType::Content);
    }

    #[test]
    fn should_clean_symbol_query_prefix() {
        assert_eq!(ModeDetector::clean_query("#test", &ModeType::Symbol), "test");
        assert_eq!(ModeDetector::clean_query("test", &ModeType::Symbol), "test");
    }

    #[test]
    fn should_clean_file_query_prefix() {
        assert_eq!(ModeDetector::clean_query(">test", &ModeType::File), "test");
        assert_eq!(ModeDetector::clean_query("test", &ModeType::File), "test");
    }

    #[test]
    fn should_clean_regex_query_prefix() {
        assert_eq!(ModeDetector::clean_query("/test", &ModeType::Regex), "test");
        assert_eq!(ModeDetector::clean_query("test", &ModeType::Regex), "test");
    }

    #[test]
    fn should_not_change_content_query() {
        assert_eq!(ModeDetector::clean_query("test", &ModeType::Content), "test");
    }
}

#[cfg(test)]
mod search_mode_tests {
    use super::*;

    #[test]
    fn symbol_mode_should_return_correct_metadata() {
        let mode = SymbolMode::new();
        let metadata = mode.metadata();
        assert_eq!(metadata.name, "Symbol");
        assert_eq!(metadata.prefix, "#");
        assert_eq!(metadata.icon, "🏷️");
    }

    #[test]
    fn file_mode_should_return_correct_metadata() {
        let mode = FileMode::new();
        let metadata = mode.metadata();
        assert_eq!(metadata.name, "File");
        assert_eq!(metadata.prefix, ">");
        assert_eq!(metadata.icon, "📁");
    }

    #[test]
    fn regex_mode_should_return_correct_metadata() {
        let mode = RegexMode::new();
        let metadata = mode.metadata();
        assert_eq!(metadata.name, "Regex");
        assert_eq!(metadata.prefix, "/");
        assert_eq!(metadata.icon, "🔧");
    }

    #[test]
    fn content_mode_should_return_correct_metadata() {
        let mode = ContentMode::new();
        let metadata = mode.metadata();
        assert_eq!(metadata.name, "Content");
        assert_eq!(metadata.prefix, "");
        assert_eq!(metadata.icon, "🔍");
    }
}

#[cfg(test)]
mod search_mode_manager_tests {
    use super::*;

    #[test]
    fn should_use_symbol_mode_for_hash_query() {
        let symbols = create_test_symbols();
        let searcher = SearchManager::new(symbols);
        let manager = SearchModeManager::new();
        let options = SearchOptions::default();

        let (_results, metadata) = manager.search("#test", &searcher, &options);
        assert_eq!(metadata.name, "Symbol");
    }

    #[test]
    fn should_use_file_mode_for_greater_query() {
        let symbols = create_test_symbols();
        let searcher = SearchManager::new(symbols);
        let manager = SearchModeManager::new();
        let options = SearchOptions::default();

        let (_results, metadata) = manager.search(">test", &searcher, &options);
        assert_eq!(metadata.name, "File");
    }

    #[test]
    fn should_use_regex_mode_for_slash_query() {
        let symbols = create_test_symbols();
        let searcher = SearchManager::new(symbols);
        let manager = SearchModeManager::new();
        let options = SearchOptions::default();

        let (_results, metadata) = manager.search("/test", &searcher, &options);
        assert_eq!(metadata.name, "Regex");
    }

    #[test]
    fn should_use_content_mode_for_plain_query() {
        let symbols = create_test_symbols();
        let searcher = SearchManager::new(symbols);
        let manager = SearchModeManager::new();
        let options = SearchOptions::default();

        let (_results, metadata) = manager.search("test", &searcher, &options);
        assert_eq!(metadata.name, "Content");
    }

    #[test]
    fn should_perform_search_with_appropriate_mode() {
        let symbols = create_test_symbols();
        let searcher = SearchManager::new(symbols);
        let manager = SearchModeManager::new();
        let options = SearchOptions::default();

        // Test symbol search
        let (results, metadata) = manager.search("#test", &searcher, &options);
        assert_eq!(metadata.name, "Symbol");
        assert!(!results.is_empty());

        // Test file search
        let (results, metadata) = manager.search(">main", &searcher, &options);
        assert_eq!(metadata.name, "File");
        assert!(!results.is_empty());

        // Test content search
        let (_results, metadata) = manager.search("function", &searcher, &options);
        assert_eq!(metadata.name, "Content");
    }

    #[test]
    fn should_return_all_available_modes() {
        let manager = SearchModeManager::new();
        let modes = manager.all_modes();
        
        assert_eq!(modes.len(), 4);
        
        let mode_names: Vec<&str> = modes.iter().map(|m| m.name.as_str()).collect();
        assert!(mode_names.contains(&"Symbol"));
        assert!(mode_names.contains(&"File"));
        assert!(mode_names.contains(&"Regex"));
        assert!(mode_names.contains(&"Content"));
    }
}