#[derive(Debug, Clone, Copy)]
pub enum SearchMode {
    Literal,
    Regexp,
}

pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
}

pub struct SearchResult {
    pub filename: String,
    pub line: u32,
    pub offset: u32, // Column position (1-based) within the line
    pub content: String,
}
