//! Symbol extraction using tree-sitter with modular language support
//!
//! This module provides functionality to extract symbols (functions, structs, etc.)
//! from source code files using tree-sitter AST parsing. Language-specific logic
//! is implemented in separate modules for better maintainability and extensibility.

use crate::languages::LanguageRegistry;
use crate::actors::types::Symbol;
use std::path::Path;
use tree_sitter::Parser;

/// Symbol extractor using tree-sitter with modular language support
pub struct SymbolExtractor {
    parser: Parser,
}

impl SymbolExtractor {
    /// Create a new SymbolExtractor
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let parser = Parser::new();
        Ok(Self { parser })
    }

    /// Extract symbols from a file
    pub fn extract_symbols_from_file(
        &mut self,
        file_path: &Path,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>> {
        // Read file content
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

        self.extract_symbols_from_content(&content, file_path.to_string_lossy().to_string())
    }

    /// Extract symbols from source code content
    pub fn extract_symbols_from_content(
        &mut self,
        content: &str,
        filepath: String,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>> {
        // Get the appropriate language extractor for this file
        let path = Path::new(&filepath);
        let extractor = LanguageRegistry::get_extractor_for_path(path);

        match extractor {
            Some(extractor) => {
                log::debug!(
                    "Using {} extractor for file: {}",
                    extractor.language_name(),
                    filepath
                );
                extractor.extract_symbols(&mut self.parser, content, &filepath)
            }
            None => {
                log::debug!("No language extractor found for file: {}", filepath);
                Ok(Vec::new()) // Unsupported language
            }
        }
    }

    /// Check if a file type is supported for symbol extraction
    pub fn is_supported_file(file_path: &Path) -> bool {
        if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
            LanguageRegistry::is_extension_supported(extension)
        } else {
            false
        }
    }

    /// Get all supported file extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        LanguageRegistry::supported_extensions()
    }
}

impl Default for SymbolExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to create default SymbolExtractor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SymbolType;

    #[test]
    fn test_symbol_extractor_creation() {
        let extractor = SymbolExtractor::new();
        assert!(extractor.is_ok(), "Should create SymbolExtractor successfully");
    }

    #[test]
    fn test_is_supported_file() {
        // Rust files
        assert!(SymbolExtractor::is_supported_file(Path::new("test.rs")));
        assert!(SymbolExtractor::is_supported_file(Path::new("/path/to/main.rs")));
        
        // JavaScript files
        assert!(SymbolExtractor::is_supported_file(Path::new("test.js")));
        assert!(SymbolExtractor::is_supported_file(Path::new("module.mjs")));
        assert!(SymbolExtractor::is_supported_file(Path::new("config.cjs")));
        
        // Python files (now supported)
        assert!(SymbolExtractor::is_supported_file(Path::new("test.py")));
        assert!(SymbolExtractor::is_supported_file(Path::new("script.pyw")));
        assert!(SymbolExtractor::is_supported_file(Path::new("types.pyi")));
        
        // Unsupported files
        assert!(!SymbolExtractor::is_supported_file(Path::new("README.md")));
        assert!(!SymbolExtractor::is_supported_file(Path::new("Cargo.toml")));
    }

    #[test]
    fn test_supported_extensions() {
        let extensions = SymbolExtractor::supported_extensions();
        assert!(extensions.contains(&"rs"), "Should support Rust files");
        assert!(extensions.contains(&"js"), "Should support JavaScript files");
        assert!(extensions.contains(&"mjs"), "Should support ES6 module files");
        assert!(extensions.contains(&"cjs"), "Should support CommonJS files");
    }

    #[test]
    fn test_rust_symbol_extraction() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let rust_code = r#"
pub fn hello_world() {
    let message = "Hello, world!";
    println!("{}", message);
}

pub struct User {
    name: String,
    age: u32,
}

pub enum Status {
    Active,
    Inactive,
}

const MAX_SIZE: usize = 100;

impl User {
    pub fn new(name: String, age: u32) -> Self {
        let user = Self { name, age };
        user
    }
    
    pub fn greet(&self, greeting: String) {
        let mut full_greeting = greeting;
        full_greeting.push_str(&self.name);
        println!("{}", full_greeting);
    }
}
"#;

        let symbols = extractor
            .extract_symbols_from_content(rust_code, "test.rs".to_string())
            .expect("Failed to extract symbols");

        assert!(!symbols.is_empty(), "Should extract some symbols");

        // Check that we found the expected symbols
        let symbol_names: Vec<String> = symbols.iter().map(|s| s.content.clone()).collect();
        println!("Extracted Rust symbols: {:?}", symbol_names);

        // We should find function, struct, enum, field, variable, and parameter symbols
        let has_function = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Function);
        let has_struct = symbols.iter().any(|s| s.symbol_type == SymbolType::Struct);
        let has_enum = symbols.iter().any(|s| s.symbol_type == SymbolType::Enum);
        let has_field = symbols.iter().any(|s| s.symbol_type == SymbolType::Field);
        let has_variable = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Variable);
        let has_parameter = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Parameter);

        assert!(has_function, "Should find function symbols");
        assert!(has_struct, "Should find struct symbols");
        assert!(has_enum, "Should find enum symbols");
        assert!(has_field, "Should find field symbols");
        assert!(has_variable, "Should find variable symbols");
        assert!(has_parameter, "Should find parameter symbols");
    }

    #[test]
    fn test_javascript_symbol_extraction() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let javascript_code = r#"
// Function declarations
function greet(name) {
    return "Hello " + name;
}

// Variable declarations
let userName = "John";
var userAge = 30;

// Const declarations
const MAX_USERS = 100;

// Class declarations
class User {
    constructor(name, age) {
        this.name = name;
        this.age = age;
    }
    
    getName() {
        return this.name;
    }
}
"#;

        let symbols = extractor
            .extract_symbols_from_content(javascript_code, "test.js".to_string())
            .expect("Failed to extract JavaScript symbols");

        assert!(!symbols.is_empty(), "Should extract some JavaScript symbols");

        // Check that we found the expected symbol types
        let symbol_names: Vec<String> = symbols.iter().map(|s| s.content.clone()).collect();
        println!("Extracted JavaScript symbols: {:?}", symbol_names);

        // We should find function, variable, class, and parameter symbols
        let has_function = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Function);
        let has_class = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Class);
        let has_variable = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Variable);
        let has_parameter = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Parameter);

        assert!(has_function, "Should find function symbols");
        assert!(has_class, "Should find class symbols");
        assert!(has_variable, "Should find variable symbols");
        assert!(has_parameter, "Should find parameter symbols");
    }

    #[test]
    fn test_javascript_file_extensions() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let js_code = r#"
function testFunction() {
    return "test";
}
"#;

        // Test .js extension
        let symbols_js = extractor
            .extract_symbols_from_content(js_code, "test.js".to_string())
            .expect("Failed to extract from .js file");
        assert!(!symbols_js.is_empty(), "Should extract from .js files");

        // Test .mjs extension
        let symbols_mjs = extractor
            .extract_symbols_from_content(js_code, "test.mjs".to_string())
            .expect("Failed to extract from .mjs file");
        assert!(!symbols_mjs.is_empty(), "Should extract from .mjs files");

        // Test .cjs extension
        let symbols_cjs = extractor
            .extract_symbols_from_content(js_code, "test.cjs".to_string())
            .expect("Failed to extract from .cjs file");
        assert!(!symbols_cjs.is_empty(), "Should extract from .cjs files");
    }

    #[test]
    fn test_python_language_support() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let python_code = r#"
def hello():
    print("Hello")

class MyClass:
    pass
"#;

        let symbols = extractor
            .extract_symbols_from_content(python_code, "test.py".to_string())
            .expect("Should extract from Python code");

        assert!(
            !symbols.is_empty(),
            "Should extract symbols from Python code"
        );
        
        // Check that we found the expected symbols
        let symbol_names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(symbol_names.contains(&"hello"), "Should find 'hello' function");
        assert!(symbol_names.contains(&"MyClass"), "Should find 'MyClass' class");
    }

    #[test]
    fn test_unsupported_language() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        // Use a truly unsupported language like Go
        let go_code = r#"
package main

func main() {
    fmt.Println("Hello")
}

type MyStruct struct {
    name string
}
"#;

        let symbols = extractor
            .extract_symbols_from_content(go_code, "test.go".to_string())
            .expect("Should succeed even for unsupported languages");

        assert!(
            symbols.is_empty(),
            "Should return empty for unsupported languages"
        );
    }

    #[test]
    fn test_default_trait() {
        let _extractor = SymbolExtractor::default();
        // Just verify it can be created - the implementation is simple now
    }

    #[test]
    fn test_extract_symbols_from_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        // Create a temporary Rust file
        let mut temp_file = NamedTempFile::with_suffix(".rs").expect("Failed to create temp file");
        let rust_code = r#"
pub fn test_function() {
    println!("test");
}

pub struct TestStruct {
    field: String,
}
"#;
        temp_file
            .write_all(rust_code.as_bytes())
            .expect("Failed to write to temp file");
        temp_file.flush().expect("Failed to flush temp file");

        let symbols = extractor
            .extract_symbols_from_file(temp_file.path())
            .expect("Failed to extract symbols from file");

        assert!(!symbols.is_empty(), "Should extract symbols from file");

        let has_function = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Function);
        let has_struct = symbols.iter().any(|s| s.symbol_type == SymbolType::Struct);

        assert!(has_function, "Should find function symbols in file");
        assert!(has_struct, "Should find struct symbols in file");
    }

    #[test]
    fn test_extract_symbols_from_nonexistent_file() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let result = extractor.extract_symbols_from_file(Path::new("/non/existent/file.rs"));
        assert!(result.is_err(), "Should return error for non-existent file");
    }

    #[test]
    fn test_empty_content() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let symbols = extractor
            .extract_symbols_from_content("", "test.rs".to_string())
            .expect("Should handle empty content");

        assert!(symbols.is_empty(), "Should return empty for empty content");
    }

    #[test]
    fn test_invalid_rust_syntax() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let invalid_rust = r#"
pub fn incomplete_function(
    // missing closing parenthesis and body
"#;

        // The extractor should handle invalid syntax gracefully
        let result = extractor.extract_symbols_from_content(invalid_rust, "test.rs".to_string());

        // Tree-sitter is robust and can parse partial/invalid syntax
        assert!(result.is_ok(), "Should handle invalid syntax gracefully");
    }
}