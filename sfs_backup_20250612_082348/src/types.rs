use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMode {
    pub name: String,
    pub prefix: String,
    pub icon: String,
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
    Added {
        file: PathBuf,
        symbols: Vec<CodeSymbol>,
    },
    /// Existing file was modified and re-indexed
    Modified {
        file: PathBuf,
        symbols: Vec<CodeSymbol>,
    },
    /// File was deleted from the index
    Removed { file: PathBuf, symbol_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchEvent {
    /// File system event that should trigger index update
    FileChanged {
        path: PathBuf,
        event_kind: WatchEventKind,
    },
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

/// Cache entry for a single file containing hash and symbols
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedFile {
    /// SHA-256 hash of file content
    pub hash: String,
    /// Timestamp when this cache entry was created
    pub last_modified: String, // ISO 8601 format
    /// Symbols extracted from this file
    pub symbols: Vec<CodeSymbol>,
    /// File size in bytes for additional validation
    pub size: u64,
}

/// Index cache data structure for .sfscache file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexCache {
    /// Cache format version for compatibility checking
    pub version: String,
    /// Timestamp when cache was created
    pub cache_created: String, // ISO 8601 format
    /// SFS version that created this cache
    pub sfs_version: String,
    /// Map of file paths to cached file data
    pub files: HashMap<String, CachedFile>, // String keys for JSON compatibility
}

impl IndexCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            cache_created: chrono::Utc::now().to_rfc3339(),
            sfs_version: env!("CARGO_PKG_VERSION").to_string(),
            files: HashMap::new(),
        }
    }

    /// Check if this cache is compatible with current SFS version
    pub fn is_compatible(&self) -> bool {
        // For now, only check version format
        self.version == "1.0"
    }

    /// Get cached file data by path
    pub fn get_file(&self, path: &str) -> Option<&CachedFile> {
        self.files.get(path)
    }

    /// Add or update cached file data
    pub fn update_file(&mut self, path: String, cached_file: CachedFile) {
        self.files.insert(path, cached_file);
    }

    /// Remove cached file data
    pub fn remove_file(&mut self, path: &str) -> Option<CachedFile> {
        self.files.remove(path)
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_files = self.files.len();
        let total_symbols: usize = self.files.values().map(|f| f.symbols.len()).sum();

        CacheStats {
            total_files,
            total_symbols,
            cache_created: self.cache_created.clone(),
            sfs_version: self.sfs_version.clone(),
        }
    }
}

impl Default for IndexCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for reporting
#[derive(Debug, Clone, PartialEq)]
pub struct CacheStats {
    pub total_files: usize,
    pub total_symbols: usize,
    pub cache_created: String,
    pub sfs_version: String,
}
