use crate::types::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::{Result, anyhow};

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
        self.initialized = true;
        Ok(())
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
            "ts" | "tsx" | "js" | "jsx" | "py" => {
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

    fn extract_symbols_from_source(&self, source: &str, file_path: &Path, symbols: &mut Vec<CodeSymbol>) {
        // Enhanced regex-based extraction for testing purposes
        use regex::Regex;
        
        for (line_num, line) in source.lines().enumerate() {
            let trimmed_line = line.trim();
            
            // Skip empty lines and comments
            if trimmed_line.is_empty() || trimmed_line.starts_with("//") || trimmed_line.starts_with("#") {
                continue;
            }
            
            // Extract different types of functions
            let function_patterns = [
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
            
            for (pattern, _func_type) in &function_patterns {
                if let Ok(re) = Regex::new(pattern) {
                    if let Some(cap) = re.captures(trimmed_line) {
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
                            }
                        }
                    }
                }
            }
            
            // Extract identifiers (constants, variables, classes)
            let identifier_patterns = [
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
            
            for (pattern, _id_type) in &identifier_patterns {
                if let Ok(re) = Regex::new(pattern) {
                    if let Some(cap) = re.captures(trimmed_line) {
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
        
        let mut builder = WalkBuilder::new(directory);
        builder.git_ignore(true)
               .git_global(true)
               .git_exclude(true)
               .hidden(false); // Show hidden files but respect .gitignore
        
        for entry in builder.build() {
            match entry {
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    if path.is_file() && self.matches_patterns(path, patterns) {
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

    pub fn clear_cache(&mut self) {
        self.symbols_cache.clear();
    }
}