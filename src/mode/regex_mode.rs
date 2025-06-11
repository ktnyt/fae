use super::{ModeMetadata, SearchMode};
use crate::{
    searcher::SearchManager,
    types::{SearchOptions, SearchResult},
};

/// Regex search mode - searches within file contents using regular expressions
pub struct RegexMode {
    metadata: ModeMetadata,
}

impl RegexMode {
    pub fn new() -> Self {
        Self {
            metadata: ModeMetadata {
                name: "Regex".to_string(),
                prefix: "/".to_string(),
                icon: "🔧".to_string(),
                description: "Search within file contents using regular expressions".to_string(),
            },
        }
    }
}

impl Default for RegexMode {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMode for RegexMode {
    fn execute(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> Vec<SearchResult> {
        // Use content search for regex - the literal search engine can handle this
        searcher.search_content(query, options)
    }

    fn metadata(&self) -> &ModeMetadata {
        &self.metadata
    }
}