pub mod content_mode;
pub mod detector;
pub mod file_mode;
pub mod regex_mode;
pub mod symbol_mode;

use crate::{
    searcher::SearchManager,
    types::{SearchOptions, SearchResult},
};
use detector::{ModeDetector, ModeType};

/// Metadata about a search mode
#[derive(Debug, Clone, PartialEq)]
pub struct ModeMetadata {
    pub name: String,
    pub prefix: String,
    pub icon: String,
    pub description: String,
}

/// Common interface for all search modes
pub trait SearchMode: Send + Sync {
    /// Execute search with the given query
    fn execute(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> Vec<SearchResult>;

    /// Get metadata about this search mode
    fn metadata(&self) -> &ModeMetadata;
}

/// Manager for all search modes
pub struct SearchModeManager {
    content_mode: content_mode::ContentMode,
    symbol_mode: symbol_mode::SymbolMode,
    file_mode: file_mode::FileMode,
    regex_mode: regex_mode::RegexMode,
}

impl Default for SearchModeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchModeManager {
    /// Create a new search mode manager with all available modes
    pub fn new() -> Self {
        Self {
            content_mode: content_mode::ContentMode::new(),
            symbol_mode: symbol_mode::SymbolMode::new(),
            file_mode: file_mode::FileMode::new(),
            regex_mode: regex_mode::RegexMode::new(),
        }
    }

    /// Get the appropriate search mode for the given mode type
    fn get_mode(&self, mode_type: &ModeType) -> &dyn SearchMode {
        match mode_type {
            ModeType::Content => &self.content_mode,
            ModeType::Symbol => &self.symbol_mode,
            ModeType::File => &self.file_mode,
            ModeType::Regex => &self.regex_mode,
        }
    }

    /// Execute search using the appropriate mode
    pub fn search(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> (Vec<SearchResult>, &ModeMetadata) {
        let mode_type = ModeDetector::detect_mode_type(query);
        let clean_query = ModeDetector::clean_query(query, &mode_type);
        let mode = self.get_mode(&mode_type);
        let results = mode.execute(&clean_query, searcher, options);
        (results, mode.metadata())
    }

    /// Get all available modes
    pub fn all_modes(&self) -> Vec<&ModeMetadata> {
        vec![
            self.content_mode.metadata(),
            self.symbol_mode.metadata(),
            self.file_mode.metadata(),
            self.regex_mode.metadata(),
        ]
    }
}

// Re-export specific modes, detector, and trait for convenience
pub use content_mode::ContentMode;
pub use file_mode::FileMode;
pub use regex_mode::RegexMode;
pub use symbol_mode::SymbolMode;