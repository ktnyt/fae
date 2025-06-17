use crate::actors::types::{SearchParams, SearchResult, SymbolType};

pub type RequestId = String;

#[derive(Clone)]
pub enum FaeMessage {
    UpdateSearchParams {
        params: SearchParams,
        request_id: RequestId,
    },
    AbortSearch, // Request to abort current search operation
    ClearResults,
    PushSearchResult {
        result: SearchResult,
        request_id: RequestId,
    },
    CompleteSearch, // Indicates search operation completion
    NotifySearchReport {
        result_count: usize,
    }, // Final search completion with result count

    // Symbol index management messages
    ClearSymbolIndex(String), // filepath
    PushSymbolIndex {
        filepath: String,
        line: u32,
        column: u32,
        name: String,
        content: String,
        symbol_type: SymbolType,
    },
    CompleteSymbolIndex(String), // filepath
    CompleteInitialIndexing,     // Indicates all initial symbol indexing is complete
    ReportSymbolIndex {
        remaining_files: usize,
        processed_files: usize,
        symbols_found: usize,
    }, // Progress report for symbol indexing

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
