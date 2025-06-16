#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Literal,
    Regexp,
    Filepath, // File path/name search mode
    Symbol,   // Symbol/function name search mode
}

pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
}

pub struct SearchResult {
    pub filename: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
}
