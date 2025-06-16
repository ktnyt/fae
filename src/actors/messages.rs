use crate::actors::types::{SearchParams, SearchResult, SymbolType};

#[derive(Clone)]
pub enum FaeMessage {
    UpdateSearchParams(SearchParams),
    ClearResults,
    PushSearchResult(SearchResult),
    CompleteSearch, // Indicates search operation completion
    SearchFinished {
        result_count: usize,
    }, // Final search completion with result count
    SetMaxResults {
        max_results: usize,
    }, // Configuration message for result handler

    // Symbol index management messages
    ClearSymbolIndex(String), // filepath
    PushSymbolIndex {
        filepath: String,
        line: u32,
        column: u32,
        content: String,
        symbol_type: SymbolType,
    },
    CompleteSymbolIndex(String), // filepath
    CompleteInitialIndexing,     // Indicates all initial symbol indexing is complete

    // File change detection messages
    DetectFileCreate(String), // filepath
    DetectFileUpdate(String), // filepath
    DetectFileDelete(String), // filepath

    // Symbol query messages (for testing)
    QuerySymbols {
        pattern: String,
        limit: Option<u32>,
    },
}
