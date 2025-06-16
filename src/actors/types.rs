#[derive(Debug, Clone, Copy)]
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
    pub offset: u32, // Column position (1-based) within the line
    pub content: String,
}
