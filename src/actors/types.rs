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

/// Type of symbol extracted by tree-sitter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Variable,
    Constant,
    Module,
    Type,
    Field,
}

impl SymbolType {
    /// Get a human-readable display name for the symbol type
    pub fn display_name(&self) -> &'static str {
        match self {
            SymbolType::Function => "fn",
            SymbolType::Method => "method",
            SymbolType::Class => "class",
            SymbolType::Struct => "struct",
            SymbolType::Enum => "enum",
            SymbolType::Interface => "interface",
            SymbolType::Variable => "var",
            SymbolType::Constant => "const",
            SymbolType::Module => "mod",
            SymbolType::Type => "type",
            SymbolType::Field => "field",
        }
    }
}

/// Symbol extracted from source code using tree-sitter
#[derive(Debug, Clone)]
pub struct Symbol {
    pub filepath: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
    pub symbol_type: SymbolType,
}

impl Symbol {
    /// Create a new Symbol
    pub fn new(
        filepath: String,
        line: u32,
        column: u32,
        content: String,
        symbol_type: SymbolType,
    ) -> Self {
        Self {
            filepath,
            line,
            column,
            content,
            symbol_type,
        }
    }

    /// Convert this Symbol into a SearchResult for compatibility
    pub fn into_search_result(self) -> SearchResult {
        SearchResult {
            filename: self.filepath,
            line: self.line,
            column: self.column,
            content: format!("[{}] {}", self.symbol_type.display_name(), self.content),
        }
    }
}
