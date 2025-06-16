use crate::actors::types::{SearchParams, SearchResult};

pub enum FaeMessage {
    UpdateSearchParams(SearchParams),
    ClearResults,
    PushSearchResult(SearchResult),
}
