pub mod content_backend;
pub mod fuzzy_search;
pub mod literal_search;

use crate::types::{CodeSymbol, SearchOptions, SearchResult, SymbolType};
use fuzzy_search::FuzzySearchEngine;
use literal_search::LiteralSearchEngine;

/// Unified search manager that coordinates different search engines
pub struct SearchManager {
    fuzzy_engine: FuzzySearchEngine,
    literal_engine: LiteralSearchEngine,
}

impl std::fmt::Debug for SearchManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchManager")
            .field("fuzzy_engine", &self.fuzzy_engine)
            .field("literal_engine", &"LiteralSearchEngine")
            .finish()
    }
}

impl SearchManager {
    /// Create new search manager with initial symbols
    pub fn new(symbols: Vec<CodeSymbol>) -> Self {
        Self {
            fuzzy_engine: FuzzySearchEngine::new(symbols.clone()),
            literal_engine: LiteralSearchEngine::new(symbols),
        }
    }

    /// Update symbols in both engines
    pub fn update_symbols(&mut self, symbols: Vec<CodeSymbol>) {
        self.fuzzy_engine.update_symbols(symbols.clone());
        self.literal_engine.update_symbols(symbols);
    }

    /// Perform symbol fuzzy search (for # prefix)
    pub fn search_symbols(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        let symbol_options = SearchOptions {
            include_files: Some(false),
            include_dirs: Some(false),
            types: Some(vec![
                SymbolType::Function,
                SymbolType::Variable,
                SymbolType::Class,
                SymbolType::Interface,
                SymbolType::Type,
                SymbolType::Enum,
                SymbolType::Constant,
                SymbolType::Method,
                SymbolType::Property,
            ]),
            ..options.clone()
        };
        self.fuzzy_engine.search(query, &symbol_options)
    }

    /// Perform file fuzzy search (for > prefix)
    pub fn search_files(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        let file_options = SearchOptions {
            types: Some(vec![SymbolType::Filename, SymbolType::Dirname]),
            ..options.clone()
        };
        self.fuzzy_engine.search(query, &file_options)
    }

    /// Perform content literal search (for no prefix or / prefix)
    pub fn search_content(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        self.literal_engine.search(query, options)
    }

    /// Generic search method that combines both engines for full-text search
    pub fn search_all(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // For comprehensive search, combine both fuzzy and literal results
        let mut all_results = Vec::new();

        // Add fuzzy search results for symbols
        let mut fuzzy_results = self.fuzzy_engine.search(query, options);
        all_results.append(&mut fuzzy_results);

        // Add literal search results for content
        let mut literal_results = self.literal_engine.search(query, options);
        all_results.append(&mut literal_results);

        // Sort by score (ascending = better match first)
        all_results.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit after combining results
        if let Some(limit) = options.limit {
            all_results.truncate(limit);
        }

        all_results
    }

    /// Get access to the symbols for inspection
    pub fn symbols(&self) -> &[CodeSymbol] {
        self.fuzzy_engine.symbols()
    }

    /// Generic search method for backward compatibility
    pub fn search(&self, query: &str, options: &SearchOptions) -> Vec<SearchResult> {
        // Default to fuzzy search for symbols for backward compatibility
        self.fuzzy_engine.search(query, options)
    }
}

// Re-export commonly used types and structs for convenience
pub use content_backend::ContentSearchBackend;

// Backward compatibility alias
pub type FuzzySearcher = SearchManager;