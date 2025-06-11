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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefaultDisplayStrategy {
    /// Show recently modified files first
    RecentlyModified,
    /// Show project important files (README, config files, main files)
    ProjectImportant,
    /// Show balanced mix of different symbol types
    SymbolBalance,
    /// Show files with most symbols first
    MostSymbols,
    /// Show random selection
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SearchOptions {
    pub include_files: Option<bool>,
    pub include_dirs: Option<bool>,
    pub types: Option<Vec<SymbolType>>,
    pub threshold: Option<f64>,
    pub limit: Option<usize>,
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

#[derive(Debug, Clone, PartialEq)]
pub enum IndexUpdate {
    /// New file was added to the index
    Added { file: PathBuf, symbols: Vec<CodeSymbol> },
    /// Existing file was modified and re-indexed
    Modified { file: PathBuf, symbols: Vec<CodeSymbol> },
    /// File was deleted from the index
    Removed { file: PathBuf, symbol_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchEvent {
    /// File system event that should trigger index update
    FileChanged { path: PathBuf, event_kind: WatchEventKind },
    /// Batch of events (for optimization)
    BatchUpdate { events: Vec<WatchEvent> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchEventKind {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf, to: PathBuf },
}