use crate::actors::types::{SearchParams, SearchResult};

pub enum FaeMessage {
    UpdateSearchQuery(SearchParams),
    ClearResults,
    PushSearchResult(SearchResult),
}

impl FaeMessage {
    pub fn update_search_query(query: SearchParams) -> Self {
        Self::UpdateSearchQuery(query)
    }

    pub fn clear_results() -> Self {
        Self::ClearResults
    }

    pub fn push_search_result(result: SearchResult) -> Self {
        Self::PushSearchResult(result)
    }
}
