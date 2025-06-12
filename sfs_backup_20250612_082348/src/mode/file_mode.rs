use super::{ModeMetadata, SearchMode};
use crate::{
    searcher::SearchManager,
    types::{SearchOptions, SearchResult},
};

/// File search mode - searches for files and directories using fuzzy matching
pub struct FileMode {
    metadata: ModeMetadata,
}

impl FileMode {
    pub fn new() -> Self {
        Self {
            metadata: ModeMetadata {
                name: "File".to_string(),
                prefix: ">".to_string(),
                icon: "📁".to_string(),
                description: "Search for files and directories using fuzzy matching".to_string(),
            },
        }
    }
}

impl Default for FileMode {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMode for FileMode {
    fn execute(
        &self,
        query: &str,
        searcher: &SearchManager,
        options: &SearchOptions,
    ) -> Vec<SearchResult> {
        searcher.search_files(query, options)
    }

    fn metadata(&self) -> &ModeMetadata {
        &self.metadata
    }
}