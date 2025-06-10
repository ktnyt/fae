use crate::types::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::{Result, anyhow};
use regex::Regex;
use std::sync::OnceLock;

// Pre-compiled regex patterns for performance
static FUNCTION_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
static IDENTIFIER_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

pub struct TreeSitterIndexer {
    symbols_cache: HashMap<PathBuf, Vec<CodeSymbol>>,
    initialized: bool,
    verbose: bool,
    respect_gitignore: bool,
}

impl TreeSitterIndexer {
    pub fn new() -> Self {
        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose: false,
            respect_gitignore: true, // Default to respecting .gitignore
        }
    }
    
    pub fn with_verbose(verbose: bool) -> Self {
        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose,
            respect_gitignore: true, // Default to respecting .gitignore
        }
    }

    pub fn with_options(verbose: bool, respect_gitignore: bool) -> Self {
        Self {
            symbols_cache: HashMap::new(),
            initialized: false,
            verbose,
            respect_gitignore,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Initialize regex patterns on first use
        Self::init_regex_patterns();
        self.initialized = true;
        Ok(())
    }
    
    // Synchronous version for parallel processing
    pub fn initialize_sync(&mut self) -> Result<()> {
        // Initialize regex patterns on first use
        Self::init_regex_patterns();
        self.initialized = true;
        Ok(())
    }

    // Initialize pre-compiled regex patterns
    fn init_regex_patterns() {
        FUNCTION_PATTERNS.get_or_init(|| {
            let patterns = [
                // Regular function declarations
                (r"^\s*function\s+(\w+)", "function"),
                (r"^\s*export\s+function\s+(\w+)", "function"),
                (r"^\s*async\s+function\s+(\w+)", "function"),
                (r"^\s*export\s+async\s+function\s+(\w+)", "function"),
                
                // Getters and setters (check before general methods)
                (r"^\s*get\s+(\w+)\s*\(\s*\)", "getter"),
                (r"^\s*set\s+(\w+)\s*\([^)]*\)", "setter"),
                
                // Class methods (including constructor and async methods)
                (r"^\s*async\s+(\w+)\s*\([^)]*\)", "async_method"),
                (r"^\s*(\w+)\s*\([^)]*\)", "method"),
                
                // Arrow functions
                (r"^\s*const\s+(\w+)\s*=.*?=>", "arrow"),
                (r"^\s*let\s+(\w+)\s*=.*?=>", "arrow"),
                (r"^\s*var\s+(\w+)\s*=.*?=>", "arrow"),
                
                // Python functions
                (r"^\s*def\s+(\w+)", "python_function"),
            ];
            
            patterns.iter().filter_map(|(pattern, type_name)| {
                Regex::new(pattern).ok().map(|regex| (regex, *type_name))
            }).collect()
        });
        
        IDENTIFIER_PATTERNS.get_or_init(|| {
            let patterns = [
                // TypeScript/JavaScript constants
                (r"^\s*const\s+([A-Z_][A-Z0-9_]*)", "constant"),
                (r"^\s*export\s+const\s+([A-Z_][A-Z0-9_]*)", "constant"),
                
                // Python constants (all caps assignment)
                (r"^\s*([A-Z_][A-Z0-9_]*)\s*=", "python_constant"),
                
                // Variables
                (r"^\s*let\s+(\w+)", "variable"),
                (r"^\s*var\s+(\w+)", "variable"),
                (r"^\s*(\w+)\s*=", "assignment"), // General assignment (lower priority)
                
                // Classes
                (r"^\s*class\s+(\w+)", "class"),
                (r"^\s*export\s+class\s+(\w+)", "class"),
                
                // TypeScript specific
                (r"^\s*interface\s+(\w+)", "interface"),
                (r"^\s*export\s+interface\s+(\w+)", "interface"),
                (r"^\s*enum\s+(\w+)", "enum"),
                (r"^\s*export\s+enum\s+(\w+)", "enum"),
                (r"^\s*type\s+(\w+)", "type"),
                (r"^\s*export\s+type\s+(\w+)", "type"),
            ];
            
            patterns.iter().filter_map(|(pattern, type_name)| {
                Regex::new(pattern).ok().map(|regex| (regex, *type_name))
            }).collect()
        });
    }

    pub async fn index_file(&mut self, file_path: &Path) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("Indexer not initialized"));
        }

        // Handle non-existent files gracefully
        if !file_path.exists() {
            self.symbols_cache.insert(file_path.to_path_buf(), vec![]);
            return Ok(());
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

        // For supported extensions, parse the file using regex-based extraction
        match extension {
            // Web languages
            "ts" | "tsx" | "js" | "jsx" | "py" | "php" | "rb" | "ruby" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Systems languages
            "go" | "rs" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Additional languages (Perl supported via regex patterns)
            "cs" | "scala" | "pl" | "pm" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            _ => {
                // Unsupported file extension - just store filename/dirname symbols
            }
        }

        self.symbols_cache.insert(file_path.to_path_buf(), symbols);
        Ok(())
    }
    
    // Synchronous version for parallel processing that returns symbols
    pub fn index_file_sync(&mut self, file_path: &Path) -> Result<Vec<CodeSymbol>> {
        if !self.initialized {
            return Err(anyhow!("Indexer not initialized"));
        }

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

        // For supported extensions, parse the file using regex-based extraction
        match extension {
            // Web languages
            "ts" | "tsx" | "js" | "jsx" | "py" | "php" | "rb" | "ruby" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Systems languages
            "go" | "rs" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Additional languages (Perl supported via regex patterns)
            "cs" | "scala" | "pl" | "pm" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            _ => {
                // Unsupported file extension - just store filename/dirname symbols
            }
        }

        Ok(symbols)
    }
    
    // Public synchronous method for extracting symbols without caching
    pub fn extract_symbols_sync(&self, file_path: &Path, verbose: bool) -> Result<Vec<CodeSymbol>> {
        // Handle non-existent files gracefully
        if !file_path.exists() {
            return Ok(vec![]);
        }

        let mut symbols = Vec::new();

        // Get file extension for parser selection
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // For supported extensions, parse the file using regex-based extraction
        match extension {
            // Web languages
            "ts" | "tsx" | "js" | "jsx" | "py" | "php" | "rb" | "ruby" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Systems languages
            "go" | "rs" | "java" | "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            // Additional languages (Perl supported via regex patterns)
            "cs" | "scala" | "pl" | "pm" => {
                if let Ok(source_code) = fs::read_to_string(file_path) {
                    self.extract_symbols_from_source(&source_code, file_path, &mut symbols);
                }
            }
            _ => {
                // Unsupported file extension - no code symbols, but we'll add file/dir symbols in caller
            }
        }

        if verbose && !symbols.is_empty() {
            eprintln!("Extracted {} symbols from {}", symbols.len(), file_path.display());
        }

        Ok(symbols)
    }

    fn extract_symbols_from_source(&self, source: &str, file_path: &Path, symbols: &mut Vec<CodeSymbol>) {
        // Use pre-compiled regex patterns for much better performance
        
        for (line_num, line) in source.lines().enumerate() {
            let trimmed_line = line.trim();
            
            // Skip empty lines and comments
            if trimmed_line.is_empty() || trimmed_line.starts_with("//") || trimmed_line.starts_with("#") {
                continue;
            }
            
            // Extract functions using pre-compiled patterns
            if let Some(function_patterns) = FUNCTION_PATTERNS.get() {
                for (regex, _func_type) in function_patterns {
                    if let Some(cap) = regex.captures(trimmed_line) {
                        if let Some(name) = cap.get(1) {
                            let function_name = name.as_str();
                            // Accept all function names, including short ones like 'add'
                            if !function_name.is_empty() && function_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                                symbols.push(CodeSymbol {
                                    name: function_name.to_string(),
                                    symbol_type: SymbolType::Function,
                                    file: file_path.to_path_buf(),
                                    line: line_num + 1,
                                    column: name.start() + 1,
                                    context: None,
                                });
                                // Continue checking for more patterns on the same line
                            }
                        }
                    }
                }
            }
            
            // Extract identifiers using pre-compiled patterns
            if let Some(identifier_patterns) = IDENTIFIER_PATTERNS.get() {
                for (regex, _id_type) in identifier_patterns {
                    if let Some(cap) = regex.captures(trimmed_line) {
                        if let Some(name) = cap.get(1) {
                            let identifier_name = name.as_str();
                            if identifier_name.len() > 2 {
                                symbols.push(CodeSymbol {
                                    name: identifier_name.to_string(),
                                    symbol_type: SymbolType::Variable,
                                    file: file_path.to_path_buf(),
                                    line: line_num + 1,
                                    column: name.start() + 1,
                                    context: None,
                                });
                                // Continue checking for more patterns on the same line
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn get_symbols_by_file(&self, file_path: &Path) -> Vec<CodeSymbol> {
        self.symbols_cache.get(file_path).cloned().unwrap_or_default()
    }

    pub fn get_all_symbols(&self) -> Vec<CodeSymbol> {
        self.symbols_cache.values().flatten().cloned().collect()
    }

    pub async fn index_directory(&mut self, directory: &Path, patterns: &[String]) -> anyhow::Result<()> {
        if self.respect_gitignore {
            self.index_directory_with_gitignore(directory, patterns).await
        } else {
            self.index_directory_ignore_gitignore(directory, patterns).await
        }
    }

    async fn index_directory_with_gitignore(&mut self, directory: &Path, patterns: &[String]) -> anyhow::Result<()> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;
        
        let mut builder = WalkBuilder::new(directory);
        builder.git_ignore(true)       // .gitignore files
               .git_global(true)       // global .gitignore
               .git_exclude(true)      // .git/info/exclude
               .require_git(false)     // don't require git repo
               .hidden(false)          // show hidden files but respect .gitignore
               .parents(true)          // respect parent .gitignore files
               .ignore(true)           // respect .ignore files
               .add_custom_ignore_filename(".ignore"); // custom ignore files
        
        // Collect all valid file paths first
        let mut file_paths = Vec::new();
        for entry in builder.build() {
            match entry {
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    
                    // Skip .git directory and other common build/cache directories
                    if let Some(path_str) = path.to_str() {
                        if path_str.contains("/.git/") || path_str.ends_with("/.git") {
                            continue;
                        }
                    }
                    
                    if path.is_file() && self.matches_patterns(path, patterns) && self.should_index_file(path) {
                        file_paths.push(path.to_path_buf());
                    }
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("Warning: Failed to read directory entry: {}", e);
                    }
                }
            }
        }
        
        // Process files in parallel using rayon
        let verbose = self.verbose;
        let results: Vec<_> = file_paths
            .par_iter()
            .filter_map(|path| {
                // Create a temporary indexer for parallel processing
                let mut temp_indexer = TreeSitterIndexer::with_options(verbose, true);
                if let Err(_) = temp_indexer.initialize_sync() {
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

    async fn index_directory_ignore_gitignore(&mut self, directory: &Path, patterns: &[String]) -> anyhow::Result<()> {
        use globwalk::GlobWalkerBuilder;
        
        for pattern in patterns {
            let walker = GlobWalkerBuilder::from_patterns(directory, &[pattern])
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to create glob walker: {}", e))?;
                
            for entry in walker {
                match entry {
                    Ok(dir_entry) => {
                        let path = dir_entry.path();
                        if path.is_file() {
                            if let Err(e) = self.index_file(path).await {
                                if self.verbose {
                                    eprintln!("Warning: Failed to index {}: {}", path.display(), e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if self.verbose {
                            eprintln!("Warning: Failed to read directory entry: {}", e);
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    fn matches_patterns(&self, path: &Path, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return true;
        }
        
        for pattern in patterns {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches_path(path) {
                    return true;
                }
            }
        }
        false
    }
    
    fn should_index_file(&self, path: &Path) -> bool {
        // Skip files that are clearly not useful for symbol search
        
        // Check file size - skip files larger than 1MB by default
        const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                if self.verbose {
                    println!("Skipping large file: {} ({} bytes)", path.display(), metadata.len());
                }
                return false;
            }
        }
        
        // Skip binary files and common non-source files
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            let binary_extensions = [
                // Images
                "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp",
                // Archives
                "zip", "tar", "gz", "bz2", "7z", "rar",
                // Executables/binaries
                "exe", "bin", "so", "dylib", "dll", "app",
                // Media
                "mp3", "mp4", "avi", "mov", "wmv", "flv",
                // Documents
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
                // Databases
                "db", "sqlite", "sqlite3",
                // Fonts
                "ttf", "otf", "woff", "woff2",
                // Build artifacts (common ones not in .gitignore)
                "o", "obj", "pyc", "class", "jar",
                // Lock files (often very large)
                "lock"
            ];
            
            if binary_extensions.contains(&extension.to_lowercase().as_str()) {
                if self.verbose {
                    println!("Skipping binary/non-source file: {}", path.display());
                }
                return false;
            }
        }
        
        // Skip files with suspicious names (likely generated/cache)
        if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
            let suspicious_patterns = [
                // Temporary files
                "~", ".tmp", ".temp", ".bak", ".backup",
                // IDE files
                ".idea", ".vscode",
                // OS files
                ".DS_Store", "Thumbs.db", "desktop.ini",
                // Log files
                ".log",
            ];
            
            for pattern in &suspicious_patterns {
                if filename.contains(pattern) {
                    if self.verbose {
                        println!("Skipping suspicious file: {}", path.display());
                    }
                    return false;
                }
            }
        }
        
        true
    }

    pub fn clear_cache(&mut self) {
        self.symbols_cache.clear();
    }
}