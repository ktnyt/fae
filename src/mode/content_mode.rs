use super::{ModeMetadata, SearchMode};
use crate::{
    searcher::SearchManager,
    types::{SearchOptions, SearchResult},
};

/// Content search mode - searches within file contents using literal search
pub struct ContentMode {
    metadata: ModeMetadata,
}

impl ContentMode {
    pub fn new() -> Self {
        Self {
            metadata: ModeMetadata {
                name: "Content".to_string(),
                prefix: "".to_string(),
                icon: "🔍".to_string(),
                description: "Search within file contents using literal search".to_string(),
            },
        }
    }
}

impl Default for ContentMode {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMode for ContentMode {
    fn execute(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> Vec<SearchResult> {
        searcher.search_content(query, options)
    }

    fn metadata(&self) -> &ModeMetadata {
        &self.metadata
    }
}