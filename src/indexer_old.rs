use crate::types::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::{Result, anyhow};
use tree_sitter::{Parser, Query, QueryCursor};

// Tree-sitter language modules
// Note: imports handled directly in function calls

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
    
    // Synchronous version for parallel processing
    pub fn initialize_sync(&mut self) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    // Get Tree-sitter language based on file extension
    fn get_language_and_query(extension: &str) -> Option<(tree_sitter::Language, &'static str)> {
        let query_source = r#"
            ; Structs
            (struct_item name: (type_identifier) @struct)
            
            ; Enums  
            (enum_item name: (type_identifier) @enum)
            
            ; Functions
            (function_item name: (identifier) @function)
            
            ; Impl blocks
            (impl_item type: (type_identifier) @impl)
            
            ; Traits
            (trait_item name: (type_identifier) @trait)
            
            ; Constants
            (const_item name: (identifier) @const)
            
            ; Statics
            (static_item name: (identifier) @static)
            
            ; Modules
            (mod_item name: (identifier) @module)
            
            ; Type aliases
            (type_item name: (type_identifier) @type)
            
            ; Methods in impl blocks
            (impl_item 
              body: (declaration_list 
                (function_item name: (identifier) @method)))
            
            ; Let bindings
            (let_declaration pattern: (identifier) @variable)
            
            ; Use statements
            (use_declaration argument: (scoped_identifier path: (_) name: (identifier) @use))
            
            ; Field names in structs
            (field_declaration name: (field_identifier) @field)
        "#;
        
        match extension {
            "rs" => Some((tree_sitter_rust::language(), query_source)),
            "ts" | "tsx" => {
                let typescript_query = r#"
                    ; Classes
                    (class_declaration name: (type_identifier) @class)
                    
                    ; Interfaces
                    (interface_declaration name: (type_identifier) @interface)
                    
                    ; Functions
                    (function_declaration name: (identifier) @function)
                    
                    ; Methods
                    (method_definition name: (property_identifier) @method)
                    
                    ; Type aliases
                    (type_alias_declaration name: (type_identifier) @type)
                    
                    ; Enums
                    (enum_declaration name: (identifier) @enum)
                    
                "#;
                Some((tree_sitter_typescript::language_typescript(), typescript_query))
            },
            "js" | "jsx" => {
                let javascript_query = r#"
                    ; Classes
                    (class_declaration name: (identifier) @class)
                    
                    ; Functions
                    (function_declaration name: (identifier) @function)
                    
                    ; Methods
                    (method_definition name: (property_identifier) @method)
                    
                "#;
                Some((tree_sitter_javascript::language(), javascript_query))
            },
            "py" => {
                let python_query = r#"
                    ; Classes
                    (class_definition name: (identifier) @class)
                    
                    ; Functions
                    (function_definition name: (identifier) @function)
                    
                    ; Assignments (variables)
                    (assignment left: (identifier) @variable)
                "#;
                Some((tree_sitter_python::language(), python_query))
            },
            "php" => {
                let php_query = r#"
                    ; Classes
                    (class_declaration name: (name) @class)
                    
                    ; Functions
                    (function_definition name: (name) @function)
                    
                    ; Methods
                    (method_declaration name: (name) @method)
                "#;
                Some((tree_sitter_php::language(), php_query))
            },
            "rb" | "ruby" => {
                let ruby_query = r#"
                    ; Classes
                    (class name: (constant) @class)
                    
                    ; Methods/Functions
                    (method name: (identifier) @function)
                    
                    ; Modules
                    (module name: (constant) @module)
                "#;
                Some((tree_sitter_ruby::language(), ruby_query))
            },
            "go" => {
                let go_query = r#"
                    ; Functions
                    (function_declaration name: (identifier) @function)
                    
                    ; Methods
                    (method_declaration name: (field_identifier) @method)
                    
                    ; Types (structs)
                    (type_declaration (type_spec name: (type_identifier) @type))
                "#;
                Some((tree_sitter_go::language(), go_query))
            },
            "java" => {
                let java_query = r#"
                    ; Classes
                    (class_declaration name: (identifier) @class)
                    
                    ; Methods
                    (method_declaration name: (identifier) @method)
                    
                    ; Constructors
                    (constructor_declaration name: (identifier) @constructor)
                    
                    ; Interfaces
                    (interface_declaration name: (identifier) @interface)
                "#;
                Some((tree_sitter_java::language(), java_query))
            },
            "c" => {
                let c_query = r#"
                    ; Functions
                    (function_definition declarator: (function_declarator declarator: (identifier) @function))
                    
                    ; Function declarations
                    (declaration declarator: (function_declarator declarator: (identifier) @function))
                "#;
                Some((tree_sitter_c::language(), c_query))
            },
            "cpp" | "cc" | "cxx" | "h" | "hpp" => {
                let cpp_query = r#"
                    ; Functions
                    (function_definition declarator: (function_declarator declarator: (identifier) @function))
                    
                    ; Function declarations
                    (declaration declarator: (function_declarator declarator: (identifier) @function))
                    
                    ; Classes
                    (class_specifier name: (type_identifier) @class)
                "#;
                Some((tree_sitter_cpp::language(), cpp_query))
            },
            "cs" => {
                let csharp_query = r#"
                    ; Classes
                    (class_declaration name: (identifier) @class)
                    
                    ; Methods
                    (method_declaration name: (identifier) @method)
                    
                    ; Interfaces
                    (interface_declaration name: (identifier) @interface)
                "#;
                Some((tree_sitter_c_sharp::language(), csharp_query))
            },
            "scala" => {
                let scala_query = r#"
                    ; Classes
                    (class_definition name: (identifier) @class)
                    
                    ; Objects
                    (object_definition name: (identifier) @object)
                    
                    ; Functions
                    (function_definition name: (identifier) @function)
                "#;
                Some((tree_sitter_scala::language(), scala_query))
            },
            _ => None,
        }
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
        // Get file extension for Tree-sitter language selection
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let (language, query_source) = match Self::get_language_and_query(extension) {
            Some((lang, query)) => (lang, query),
            None => return, // Unsupported language, skip Tree-sitter parsing
        };
        
        // Set up Tree-sitter parser
        let mut parser = Parser::new();
        if parser.set_language(language).is_err() {
            if self.verbose {
                eprintln!("Failed to set Tree-sitter language for {}", file_path.display());
            }
            return;
        }
        
        // Parse the source code
        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => {
                if self.verbose {
                    eprintln!("Failed to parse {} with Tree-sitter", file_path.display());
                }
                return;
            }
        };
        
        // Create and execute query
        let query = match Query::new(language, query_source) {
            Ok(query) => query,
            Err(e) => {
                if self.verbose {
                    eprintln!("Failed to create Tree-sitter query for {}: {}", file_path.display(), e);
                }
                return;
            }
        };
        
        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
        
        let capture_names = query.capture_names();
        
        for match_ in matches {
            for capture in match_.captures {
                let node = capture.node;
                let capture_name = &capture_names[capture.index as usize];
                
                if let Ok(text) = node.utf8_text(source.as_bytes()) {
                    // Skip very short or obviously invalid symbols
                    if text.len() < 2 || !text.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    
                    let start = node.start_position();
                    let symbol_type = match capture_name.as_str() {
                        "function" | "method" | "arrow" => SymbolType::Function,
                        "class" | "struct" | "interface" | "trait" => SymbolType::Class,
                        "enum" | "type" => SymbolType::Type,
                        "const" | "static" => SymbolType::Constant,
                        "variable" | "field" | "use" | "module" | "impl" => SymbolType::Variable,
                        _ => SymbolType::Variable, // Default fallback
                    };
                    
                    symbols.push(CodeSymbol {
                        name: text.to_string(),
                        symbol_type,
                        file: file_path.to_path_buf(),
                        line: start.row + 1,
                        column: start.column + 1,
                        context: None,
                    });
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