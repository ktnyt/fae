use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolType {
    Function,
    Variable,
    Class,
    Interface,
    Type,
    Enum,
    Constant,
    Method,
    Property,
    Filename,
    Dirname,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub include_files: Option<bool>,
    pub include_dirs: Option<bool>,
    pub types: Option<Vec<SymbolType>>,
    pub threshold: Option<f64>,
    pub limit: Option<usize>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            include_files: None,
            include_dirs: None,
            types: None,
            threshold: None,
            limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub symbol: CodeSymbol,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedFile {
    pub path: PathBuf,
    pub symbols: Vec<CodeSymbol>,
    pub last_modified: u64,
}