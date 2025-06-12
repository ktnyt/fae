use super::{ModeMetadata, SearchMode};
use crate::{
    searcher::SearchManager,
    types::{SearchOptions, SearchResult},
};

/// Symbol search mode - searches for code symbols using fuzzy matching
pub struct SymbolMode {
    metadata: ModeMetadata,
}

impl SymbolMode {
    pub fn new() -> Self {
        Self {
            metadata: ModeMetadata {
                name: "Symbol".to_string(),
                prefix: "#".to_string(),
                icon: "🏷️".to_string(),
                description: "Search for code symbols (functions, classes, variables) using fuzzy matching".to_string(),
            },
        }
    }
}

impl Default for SymbolMode {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMode for SymbolMode {
    fn execute(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> Vec<SearchResult> {
        searcher.search_symbols(query, options)
    }

    fn metadata(&self) -> &ModeMetadata {
        &self.metadata
    }
}