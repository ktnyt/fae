use crate::types::*;
use crate::parsers::SymbolExtractor;
use crate::filters::{FileFilter, GitignoreFilter};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::{Result, anyhow};
use rayon::prelude::*;

pub struct TreeSitterIndexer {
    symbols_cache: HashMap<PathBuf, Vec<CodeSymbol>>,
    initialized: bool,
    verbose: bool,
    respect_gitignore: bool,
    symbol_extractor: SymbolExtractor,
    file_filter: FileFilter,
    gitignore_filter: GitignoreFilter,
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
            eprintln!("Extracted {} symbols from {}", symbols.len(), file_path.display());
        }

        Ok(symbols)
    }

    /// Create file and directory symbols for a given path
    fn create_file_symbols(&self, file_path: &Path) -> Result<Vec<CodeSymbol>> {
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
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // For supported extensions, parse the file using Tree-sitter
        match extension {
            // Web languages
            "ts" | "tsx" | "js" | "jsx" | "py" | "php" | "rb" | "ruby" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self.symbol_extractor.extract_symbols(&source_code, file_path) {
                        symbols.extend(extracted);
                    }
                }
            }
            // Systems languages
            "go" | "rs" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self.symbol_extractor.extract_symbols(&source_code, file_path) {
                        symbols.extend(extracted);
                    }
                }
            }
            // Additional languages
            "cs" | "scala" | "pl" | "pm" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    if let Ok(extracted) = self.symbol_extractor.extract_symbols(&source_code, file_path) {
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
        self.symbols_cache.get(file_path).cloned().unwrap_or_default()
    }

    pub fn get_all_symbols(&self) -> Vec<CodeSymbol> {
        self.symbols_cache.values().flatten().cloned().collect()
    }

    pub async fn index_directory(&mut self, directory: &Path, patterns: &[String]) -> anyhow::Result<()> {
        // Collect all valid file paths first
        let mut file_paths = Vec::new();
        let walker = self.gitignore_filter.create_walker(directory);
        
        for entry in walker.build() {
            if let Some(path) = self.gitignore_filter.should_process_entry(&entry) {
                if self.file_filter.matches_patterns(&path, patterns) && self.file_filter.should_index_file(&path) {
                    file_paths.push(path);
                }
            }
        }
        
        // Process files in parallel using rayon
        let verbose = self.verbose;
        let results: Vec<_> = file_paths
            .par_iter()
            .filter_map(|path| {
                // Create a temporary indexer for parallel processing
                let mut temp_indexer = TreeSitterIndexer::with_options(verbose, self.respect_gitignore);
                if temp_indexer.initialize_sync().is_err() {
                    if verbose {
                        eprintln!("Warning: Failed to initialize indexer for {}", path.display());
                    }
                    return None;
                }
                
                match temp_indexer.index_file_sync(path) {
                    Ok(symbols) => Some((path.clone(), symbols)),
                    Err(e) => {
                        if verbose {
                            eprintln!("Warning: Failed to index {}: {}", path.display(), e);
                        }
                        None
                    }
                }
            })
            .collect();
        
        // Merge results back into main indexer
        for (path, symbols) in results {
            self.symbols_cache.insert(path, symbols);
        }
        
        Ok(())
    }

    pub fn clear_cache(&mut self) {
        self.symbols_cache.clear();
    }
}