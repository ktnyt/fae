use crate::cache_manager::MemoryEfficientCacheManager;
use crate::filters::{FileFilter, GitignoreFilter};
use crate::parsers::SymbolExtractor;
use crate::types::*;
use anyhow::{anyhow, Result};
use chrono::Utc;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct TreeSitterIndexer {
    symbols_cache: HashMap<PathBuf, Vec<CodeSymbol>>,
    initialized: bool,
    verbose: bool,
    respect_gitignore: bool,
    symbol_extractor: SymbolExtractor,
    file_filter: FileFilter,
    gitignore_filter: GitignoreFilter,
    // Cache-related fields
    index_cache: IndexCache,
    cache_enabled: bool,
    cache_directory: Option<PathBuf>,
    memory_efficient_cache: Option<MemoryEfficientCacheManager>,
    use_memory_efficient_cache: bool,
}

impl Default for TreeSitterIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeSitterIndexer {
    pub fn new() -> Self {
        let verbose = false;
        let respect_gitignore = true;

        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose,
            respect_gitignore,
            symbol_extractor: SymbolExtractor::new(verbose),
            file_filter: FileFilter::new(verbose),
            gitignore_filter: GitignoreFilter::new(respect_gitignore, verbose),
            index_cache: IndexCache::new(),
            cache_enabled: true,
            cache_directory: None,
            memory_efficient_cache: None,
            use_memory_efficient_cache: false,
        }
    }

    pub fn with_verbose(verbose: bool) -> Self {
        let respect_gitignore = true;

        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose,
            respect_gitignore,
            symbol_extractor: SymbolExtractor::new(verbose),
            file_filter: FileFilter::new(verbose),
            gitignore_filter: GitignoreFilter::new(respect_gitignore, verbose),
            index_cache: IndexCache::new(),
            cache_enabled: true,
            cache_directory: None,
            memory_efficient_cache: None,
            use_memory_efficient_cache: false,
        }
    }

    pub fn with_options(verbose: bool, respect_gitignore: bool) -> Self {
        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose,
            respect_gitignore,
            symbol_extractor: SymbolExtractor::new(verbose),
            file_filter: FileFilter::new(verbose),
            gitignore_filter: GitignoreFilter::new(respect_gitignore, verbose),
            index_cache: IndexCache::new(),
            cache_enabled: true,
            cache_directory: None,
            memory_efficient_cache: None,
            use_memory_efficient_cache: false,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    // Synchronous version for parallel processing
    pub fn initialize_sync(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    pub async fn index_file(&mut self, file_path: &Path) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("Indexer not initialized"));
        }

        let symbols = self.create_file_symbols(file_path)?;
        self.symbols_cache.insert(file_path.to_path_buf(), symbols);
        Ok(())
    }

    // Synchronous version for parallel processing that returns symbols
    pub fn index_file_sync(&mut self, file_path: &Path) -> Result<Vec<CodeSymbol>> {
        if !self.initialized {
            return Err(anyhow!("Indexer not initialized"));
        }

        self.create_file_symbols(file_path)
    }

    // Public synchronous method for extracting symbols without caching
    pub fn extract_symbols_sync(&self, file_path: &Path, verbose: bool) -> Result<Vec<CodeSymbol>> {
        // Use create_file_symbols to get complete symbols including filename/dirname
        let symbols = self.create_file_symbols(file_path)?;

        if verbose && !symbols.is_empty() {
            eprintln!(
                "Extracted {} symbols from {}",
                symbols.len(),
                file_path.display()
            );
        }

        Ok(symbols)
    }

    /// Create file and directory symbols for a given path
    pub fn create_file_symbols(&self, file_path: &Path) -> Result<Vec<CodeSymbol>> {
        // Handle non-existent files gracefully
        if !file_path.exists() {
            return Ok(vec![]);
        }

        let mut symbols = Vec::new();

        // Add filename and dirname symbols
        if let Some(filename) = file_path.file_name() {
            symbols.push(CodeSymbol {
                name: filename.to_string_lossy().to_string(),
                symbol_type: SymbolType::Filename,
                file: file_path.to_path_buf(),
                line: 1,
                column: 1,
                context: None,
            });
        }

        if let Some(parent) = file_path.parent() {
            if let Some(dirname) = parent.file_name() {
                symbols.push(CodeSymbol {
                    name: dirname.to_string_lossy().to_string(),
                    symbol_type: SymbolType::Dirname,
                    file: file_path.to_path_buf(),
                    line: 1,
                    column: 1,
                    context: None,
                });
            }
        }

        // Get file extension for parser selection
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // For supported extensions, parse the file using Tree-sitter
        match extension {
            // Web languages
            "ts" | "tsx" | "js" | "jsx" | "py" | "php" | "rb" | "ruby" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self
                        .symbol_extractor
                        .extract_symbols(&source_code, file_path)
                    {
                        symbols.extend(extracted);
                    }
                }
            }
            // Systems languages
            "go" | "rs" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self
                        .symbol_extractor
                        .extract_symbols(&source_code, file_path)
                    {
                        symbols.extend(extracted);
                    }
                }
            }
            // Additional languages
            "cs" | "scala" | "pl" | "pm" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self
                        .symbol_extractor
                        .extract_symbols(&source_code, file_path)
                    {
                        symbols.extend(extracted);
                    }
                }
            }
            _ => {
                // Unsupported file extension - just store filename/dirname symbols
            }
        }

        Ok(symbols)
    }

    pub fn get_symbols_by_file(&self, file_path: &Path) -> Vec<CodeSymbol> {
        self.symbols_cache
            .get(file_path)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_all_symbols(&self) -> Vec<CodeSymbol> {
        // If symbols are in memory cache, use that
        if !self.symbols_cache.is_empty() {
            return self.symbols_cache.values().flatten().cloned().collect();
        }

        // Otherwise, extract from index cache
        self.index_cache
            .files
            .values()
            .flat_map(|cached_file| cached_file.symbols.iter())
            .cloned()
            .collect()
    }

    pub async fn index_directory(
        &mut self,
        directory: &Path,
        patterns: &[String],
    ) -> anyhow::Result<()> {
        // Collect all valid file paths first
        let mut file_paths = Vec::new();
        let walker = self.gitignore_filter.create_walker(directory);

        for entry in walker.build() {
            if let Some(path) = self.gitignore_filter.should_process_entry(&entry) {
                if self.file_filter.matches_patterns(&path, patterns)
                    && self.file_filter.should_index_file(&path)
                {
                    file_paths.push(path);
                }
            }
        }

        // Process files using cache-aware indexing
        let verbose = self.verbose;
        let mut results = Vec::new();

        for path in file_paths {
            match self.load_or_index_file(&path) {
                Ok(symbols) => {
                    results.push((path, symbols));
                }
                Err(e) => {
                    if verbose {
                        eprintln!("Warning: Failed to index {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Merge results back into main indexer (cache already updated by load_or_index_file)
        for (path, symbols) in results {
            self.symbols_cache.insert(path.clone(), symbols.clone());
        }

        Ok(())
    }

    pub fn clear_cache(&mut self) {
        self.symbols_cache.clear();
    }

    // === Incremental index update methods for file watching ===

    /// Apply an index update from file watcher
    pub fn apply_index_update(&mut self, update: &IndexUpdate) -> Result<()> {
        match update {
            IndexUpdate::Added { file, symbols } => {
                self.add_file_symbols(file, symbols.clone())?;
            }
            IndexUpdate::Modified { file, symbols } => {
                self.update_file_symbols(file, symbols.clone())?;
            }
            IndexUpdate::Removed { file, .. } => {
                self.remove_file_symbols(file)?;
            }
        }
        Ok(())
    }

    /// Add symbols for a new file
    pub fn add_file_symbols(&mut self, file_path: &Path, symbols: Vec<CodeSymbol>) -> Result<()> {
        if self.verbose {
            eprintln!(
                "Adding {} symbols from new file: {}",
                symbols.len(),
                file_path.display()
            );
        }
        self.symbols_cache.insert(file_path.to_path_buf(), symbols);
        Ok(())
    }

    /// Update symbols for an existing file
    pub fn update_file_symbols(
        &mut self,
        file_path: &Path,
        symbols: Vec<CodeSymbol>,
    ) -> Result<()> {
        if self.verbose {
            eprintln!(
                "Updating {} symbols for modified file: {}",
                symbols.len(),
                file_path.display()
            );
        }
        self.symbols_cache.insert(file_path.to_path_buf(), symbols);
        Ok(())
    }

    /// Remove symbols for a deleted file
    pub fn remove_file_symbols(&mut self, file_path: &PathBuf) -> Result<()> {
        if let Some(removed_symbols) = self.symbols_cache.remove(file_path) {
            if self.verbose {
                eprintln!(
                    "Removed {} symbols from deleted file: {}",
                    removed_symbols.len(),
                    file_path.display()
                );
            }
        }
        Ok(())
    }

    /// Re-index a specific file and update cache
    pub fn reindex_file(
        &mut self,
        file_path: &PathBuf,
        patterns: &[String],
    ) -> Result<Vec<CodeSymbol>> {
        // Check if file should be indexed based on patterns and filters
        if !self.file_filter.matches_patterns(file_path, patterns)
            || !self.file_filter.should_index_file(file_path)
        {
            // File shouldn't be indexed - remove from cache if present
            if let Some(old_symbols) = self.symbols_cache.remove(file_path) {
                if self.verbose {
                    eprintln!(
                        "File no longer matches patterns, removed {} symbols: {}",
                        old_symbols.len(),
                        file_path.display()
                    );
                }
            }
            return Ok(vec![]);
        }

        // Re-index the file
        let symbols = self.create_file_symbols(file_path)?;
        self.symbols_cache
            .insert(file_path.clone(), symbols.clone());

        if self.verbose {
            eprintln!(
                "Re-indexed file with {} symbols: {}",
                symbols.len(),
                file_path.display()
            );
        }

        Ok(symbols)
    }

    /// Get current symbol count for monitoring
    pub fn get_symbol_count(&self) -> usize {
        self.symbols_cache
            .values()
            .map(|symbols| symbols.len())
            .sum()
    }

    /// Get current file count for monitoring
    pub fn get_file_count(&self) -> usize {
        self.symbols_cache.len()
    }

    /// Check if a file is currently indexed
    pub fn is_file_indexed(&self, file_path: &PathBuf) -> bool {
        self.symbols_cache.contains_key(file_path)
    }

    // ====== Cache Management Methods ======

    /// Enable or disable cache functionality
    pub fn set_cache_enabled(&mut self, enabled: bool) {
        self.cache_enabled = enabled;
    }

    /// Check if cache is enabled
    pub fn is_cache_enabled(&self) -> bool {
        self.cache_enabled
    }

    /// Set cache directory (defaults to project root)
    pub fn set_cache_directory(&mut self, directory: PathBuf) {
        self.cache_directory = Some(directory);
    }

    /// Enable memory-efficient cache with specified memory limit (MB)
    pub fn enable_memory_efficient_cache(&mut self, directory: PathBuf, max_memory_mb: usize) {
        self.use_memory_efficient_cache = true;
        let cache_manager = MemoryEfficientCacheManager::new(directory, max_memory_mb);
        self.memory_efficient_cache = Some(cache_manager);

        if self.verbose {
            eprintln!(
                "Memory-efficient cache enabled with {}MB limit",
                max_memory_mb
            );
        }
    }

    /// Disable memory-efficient cache
    pub fn disable_memory_efficient_cache(&mut self) {
        self.use_memory_efficient_cache = false;
        self.memory_efficient_cache = None;

        if self.verbose {
            eprintln!("Memory-efficient cache disabled");
        }
    }

    /// Check if memory-efficient cache is enabled
    pub fn is_memory_efficient_cache_enabled(&self) -> bool {
        self.use_memory_efficient_cache
    }

    /// Get memory usage of efficient cache (MB)
    pub fn get_memory_efficient_cache_usage(&self) -> f64 {
        self.memory_efficient_cache
            .as_ref()
            .map(|cache| cache.memory_usage_mb())
            .unwrap_or(0.0)
    }

    /// Calculate SHA-256 hash of file content
    pub fn calculate_file_hash(&self, file_path: &Path) -> Result<String> {
        let mut file = fs::File::open(file_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192]; // 8KB buffer

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash = hasher.finalize();
        Ok(format!("sha256:{:x}", hash))
    }

    /// Check if cached file is still valid (hash matches)
    pub fn is_cache_valid(&self, file_path: &Path, cached_hash: &str) -> Result<bool> {
        if !file_path.exists() {
            return Ok(false);
        }

        let current_hash = self.calculate_file_hash(file_path)?;
        Ok(current_hash == cached_hash)
    }

    /// Load cache from compressed .sfscache.gz file (with fallback to uncompressed .sfscache)
    pub fn load_cache(&mut self, directory: &Path) -> Result<CacheStats> {
        if !self.cache_enabled {
            return Err(anyhow!("Cache is disabled"));
        }

        // Try compressed cache first, then fallback to uncompressed
        let compressed_cache_path = directory.join(".sfscache.gz");
        let uncompressed_cache_path = directory.join(".sfscache");

        let (cache_path, is_compressed) = if compressed_cache_path.exists() {
            (compressed_cache_path, true)
        } else if uncompressed_cache_path.exists() {
            (uncompressed_cache_path, false)
        } else {
            if self.verbose {
                eprintln!(
                    "No cache file found at: {} or {}",
                    compressed_cache_path.display(),
                    uncompressed_cache_path.display()
                );
            }
            return Ok(CacheStats {
                total_files: 0,
                total_symbols: 0,
                cache_created: "N/A".to_string(),
                sfs_version: "N/A".to_string(),
            });
        };

        if self.verbose {
            eprintln!(
                "Loading {} cache from: {}",
                if is_compressed {
                    "compressed"
                } else {
                    "uncompressed"
                },
                cache_path.display()
            );
        }

        let cache_content = if is_compressed {
            // Decompress gzip file
            let file = fs::File::open(&cache_path)?;
            let mut decoder = GzDecoder::new(file);
            let mut decompressed = String::new();
            decoder.read_to_string(&mut decompressed)?;
            decompressed
        } else {
            // Read uncompressed file
            fs::read_to_string(&cache_path)?
        };

        let loaded_cache: IndexCache = serde_json::from_str(&cache_content)?;

        // Check cache compatibility
        if !loaded_cache.is_compatible() {
            return Err(anyhow!(
                "Cache version incompatible. Current format: 1.0, found: {}",
                loaded_cache.version
            ));
        }

        self.index_cache = loaded_cache;
        self.cache_directory = Some(directory.to_path_buf());

        let stats = self.index_cache.stats();

        if self.verbose {
            eprintln!(
                "Cache loaded: {} files, {} symbols",
                stats.total_files, stats.total_symbols
            );
        }

        Ok(stats)
    }

    /// Save current cache to compressed .sfscache.gz file
    pub fn save_cache(&self, directory: &Path) -> Result<()> {
        if !self.cache_enabled {
            return Err(anyhow!("Cache is disabled"));
        }

        let cache_path = directory.join(".sfscache.gz");

        if self.verbose {
            eprintln!("Saving compressed cache to: {}", cache_path.display());
        }

        // Serialize to compact JSON (no pretty printing for compression efficiency)
        let cache_json = serde_json::to_string(&self.index_cache)?;

        // Create gzip encoder with high compression
        let file = fs::File::create(&cache_path)?;
        let mut encoder = GzEncoder::new(file, Compression::best());
        encoder.write_all(cache_json.as_bytes())?;
        encoder.finish()?;

        if self.verbose {
            let stats = self.index_cache.stats();
            let uncompressed_size = cache_json.len();
            let compressed_size = fs::metadata(&cache_path)?.len();
            let compression_ratio =
                (uncompressed_size as f64 / compressed_size as f64).round() as u64;

            eprintln!(
                "Cache saved: {} files, {} symbols",
                stats.total_files, stats.total_symbols
            );
            eprintln!(
                "Compression: {} bytes → {} bytes ({}x reduction)",
                uncompressed_size, compressed_size, compression_ratio
            );
        }

        Ok(())
    }

    /// Load symbols from cache if file hash matches, otherwise re-index
    pub fn load_or_index_file(&mut self, file_path: &Path) -> Result<Vec<CodeSymbol>> {
        if !self.cache_enabled {
            return self.create_file_symbols(file_path);
        }

        let path_str = file_path.to_string_lossy().to_string();

        // Check if file is in cache
        if let Some(cached_file) = self.index_cache.get_file(&path_str) {
            // Validate cache entry
            match self.is_cache_valid(file_path, &cached_file.hash) {
                Ok(true) => {
                    if self.verbose {
                        eprintln!(
                            "Cache hit for file: {} ({} symbols)",
                            file_path.display(),
                            cached_file.symbols.len()
                        );
                    }
                    return Ok(cached_file.symbols.clone());
                }
                Ok(false) => {
                    if self.verbose {
                        eprintln!("Cache miss (file changed): {}", file_path.display());
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("Cache validation error for {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        // Cache miss or invalid - re-index file
        let symbols = self.create_file_symbols(file_path)?;
        self.update_cache_entry(file_path, &symbols)?;

        Ok(symbols)
    }

    /// Update cache entry for a file
    pub fn update_cache_entry(&mut self, file_path: &Path, symbols: &[CodeSymbol]) -> Result<()> {
        if !self.cache_enabled {
            return Ok(());
        }

        let path_str = file_path.to_string_lossy().to_string();
        let hash = self.calculate_file_hash(file_path)?;
        let size = file_path.metadata()?.len();

        let cached_file = CachedFile {
            hash,
            last_modified: Utc::now().to_rfc3339(),
            symbols: symbols.to_vec(),
            size,
        };

        self.index_cache.update_file(path_str, cached_file);

        if self.verbose {
            eprintln!(
                "Updated cache entry for: {} ({} symbols)",
                file_path.display(),
                symbols.len()
            );
        }

        Ok(())
    }

    /// Remove cache entry for a file
    pub fn remove_cache_entry(&mut self, file_path: &Path) -> Result<()> {
        if !self.cache_enabled {
            return Ok(());
        }

        let path_str = file_path.to_string_lossy().to_string();

        if let Some(removed) = self.index_cache.remove_file(&path_str) {
            if self.verbose {
                eprintln!(
                    "Removed cache entry for: {} ({} symbols)",
                    file_path.display(),
                    removed.symbols.len()
                );
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        self.index_cache.stats()
    }

    /// Extract all symbols from loaded cache for immediate use
    pub fn get_all_cached_symbols(&self) -> Vec<CodeSymbol> {
        if !self.cache_enabled {
            return Vec::new();
        }

        let mut all_symbols = Vec::new();
        for cached_file in self.index_cache.files.values() {
            all_symbols.extend(cached_file.symbols.clone());
        }

        all_symbols
    }

    /// Check if a file exists in cache and is still valid
    pub fn is_file_cached_and_valid(&self, file_path: &Path) -> bool {
        if !self.cache_enabled {
            return false;
        }

        let path_str = file_path.to_string_lossy().to_string();
        if let Some(cached_file) = self.index_cache.get_file(&path_str) {
            if let Ok(current_hash) = self.calculate_file_hash(file_path) {
                return current_hash == cached_file.hash;
            }
        }

        false
    }

    /// Clear all index cache data
    pub fn clear_index_cache(&mut self) {
        self.index_cache = IndexCache::new();

        if self.verbose {
            eprintln!("Index cache cleared");
        }
    }

    /// Delete cache files from disk (both compressed and uncompressed)
    pub fn delete_cache_file(&self, directory: &Path) -> Result<()> {
        let compressed_cache_path = directory.join(".sfscache.gz");
        let uncompressed_cache_path = directory.join(".sfscache");

        let mut deleted_files = Vec::new();

        if compressed_cache_path.exists() {
            fs::remove_file(&compressed_cache_path)?;
            deleted_files.push(compressed_cache_path.display().to_string());
        }

        if uncompressed_cache_path.exists() {
            fs::remove_file(&uncompressed_cache_path)?;
            deleted_files.push(uncompressed_cache_path.display().to_string());
        }

        if self.verbose && !deleted_files.is_empty() {
            eprintln!("Cache files deleted: {}", deleted_files.join(", "));
        }

        Ok(())
    }
}
