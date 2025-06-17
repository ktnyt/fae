//! Symbol extraction using tree-sitter
//!
//! This module provides functionality to extract symbols (functions, structs, etc.)
//! from source code files using tree-sitter AST parsing.

use crate::actors::types::{Symbol, SymbolType};
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

/// Language-specific symbol extraction configuration
pub struct LanguageConfig {
    pub language: Language,
    pub query: Query,
}

/// Symbol extractor using tree-sitter
pub struct SymbolExtractor {
    parser: Parser,
    rust_config: Option<LanguageConfig>,
}

impl SymbolExtractor {
    /// Create a new SymbolExtractor
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let parser = Parser::new();

        // Initialize Rust language support
        let rust_config = Self::create_rust_config()?;

        Ok(Self {
            parser,
            rust_config: Some(rust_config),
        })
    }

    /// Create Rust language configuration with queries
    fn create_rust_config() -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>> {
        let language = tree_sitter_rust::language();

        // Tree-sitter query for Rust symbols
        let query_source = r#"
        ; Functions
        (function_item
          name: (identifier) @function.name) @function.definition
        
        ; Structs
        (struct_item
          name: (type_identifier) @struct.name) @struct.definition
        
        ; Enums
        (enum_item
          name: (type_identifier) @enum.name) @enum.definition
        
        ; Impl blocks (methods)
        (impl_item
          type: (type_identifier) @impl.type
          body: (declaration_list
            (function_item
              name: (identifier) @method.name) @method.definition))
        
        ; Constants
        (const_item
          name: (identifier) @constant.name) @constant.definition
        
        ; Static variables
        (static_item
          name: (identifier) @static.name) @static.definition
        
        ; Type aliases
        (type_item
          name: (type_identifier) @type.name) @type.definition
        
        ; Modules
        (mod_item
          name: (identifier) @module.name) @module.definition
        
        ; Struct fields
        (field_declaration
          name: (field_identifier) @field.name) @field.definition
        
        ; Let bindings (local variables)
        (let_declaration
          pattern: (identifier) @variable.name) @variable.definition
        
        ; Function parameters
        (function_item
          parameters: (parameters
            (parameter
              pattern: (identifier) @parameter.name)))
        
        ; Method parameters
        (impl_item
          body: (declaration_list
            (function_item
              parameters: (parameters
                (parameter
                  pattern: (identifier) @parameter.name)))))
        "#;

        let query = Query::new(&language, query_source)
            .map_err(|e| format!("Failed to parse Rust query: {}", e))?;

        Ok(LanguageConfig { language, query })
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
        // Determine language based on file extension
        let language_config = if filepath.ends_with(".rs") {
            self.rust_config.as_ref()
        } else {
            return Ok(Vec::new()); // Unsupported language
        };

        let config = match language_config {
            Some(config) => config,
            None => return Ok(Vec::new()),
        };

        // Set parser language
        self.parser
            .set_language(&config.language)
            .map_err(|e| format!("Failed to set parser language: {}", e))?;

        // Parse the content
        let tree = self
            .parser
            .parse(content, None)
            .ok_or("Failed to parse source code")?;

        // Extract symbols using queries
        self.extract_symbols_with_query(content, &filepath, &tree, &config.query)
    }

    /// Extract symbols using tree-sitter queries
    fn extract_symbols_with_query(
        &self,
        content: &str,
        filepath: &str,
        tree: &Tree,
        query: &Query,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>> {
        let mut symbols = Vec::new();
        let mut cursor = QueryCursor::new();

        // Split content into lines for line number calculation
        let lines: Vec<&str> = content.lines().collect();

        // Execute query
        let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        for query_match in matches {
            for capture in query_match.captures {
                let node = capture.node;
                let capture_name = query.capture_names()[capture.index as usize];

                // Get position information
                let start_position = node.start_position();
                let line = start_position.row as u32 + 1; // 1-indexed
                let column = start_position.column as u32;

                // Get symbol content
                let symbol_text = node
                    .utf8_text(content.as_bytes())
                    .unwrap_or("<unknown>")
                    .to_string();

                // Determine symbol type based on capture name
                let symbol_type = match capture_name {
                    "function.name" => SymbolType::Function,
                    "struct.name" => SymbolType::Struct,
                    "enum.name" => SymbolType::Enum,
                    "method.name" => SymbolType::Method,
                    "constant.name" => SymbolType::Constant,
                    "static.name" => SymbolType::Variable,
                    "type.name" => SymbolType::Type,
                    "module.name" => SymbolType::Module,
                    "field.name" => SymbolType::Field,
                    "variable.name" => SymbolType::Variable,
                    "parameter.name" => SymbolType::Parameter,
                    _ => continue, // Skip unknown captures
                };

                // Create symbol with context information
                let symbol_content =
                    self.create_symbol_content(&symbol_text, &lines, line as usize);

                let symbol = Symbol::new(
                    filepath.to_string(),
                    line,
                    column,
                    symbol_content,
                    symbol_type,
                );

                symbols.push(symbol);
            }
        }

        Ok(symbols)
    }

    /// Create symbol content with surrounding context
    fn create_symbol_content(
        &self,
        symbol_name: &str,
        lines: &[&str],
        line_index: usize,
    ) -> String {
        // Get the line content (0-indexed for array access)
        let line_content = if line_index > 0 && line_index <= lines.len() {
            lines[line_index - 1].trim()
        } else {
            symbol_name
        };

        // Return the line content or just the symbol name if line is empty
        if line_content.is_empty() {
            symbol_name.to_string()
        } else {
            line_content.to_string()
        }
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

    #[test]
    fn test_symbol_extractor_creation() {
        let extractor = SymbolExtractor::new();
        assert!(extractor.is_ok());
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

        println!("Extracted symbols: {:?}", symbol_names);

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
    fn test_unsupported_language() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        let python_code = r#"
def hello():
    print("Hello")

class MyClass:
    pass
"#;

        let symbols = extractor
            .extract_symbols_from_content(python_code, "test.py".to_string())
            .expect("Should succeed even for unsupported languages");

        assert!(
            symbols.is_empty(),
            "Should return empty for unsupported languages"
        );
    }

    #[test]
    fn test_default_trait() {
        let extractor = SymbolExtractor::default();
        assert!(
            extractor.rust_config.is_some(),
            "Default extractor should have Rust config"
        );
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
    fn test_create_symbol_content_edge_cases() {
        let extractor = SymbolExtractor::new().expect("Failed to create extractor");

        // Test with empty lines
        let lines = vec!["", "fn test()", ""];
        let content = extractor.create_symbol_content("test", &lines, 2);
        assert_eq!(content, "fn test()", "Should return line content");

        // Test with whitespace only
        let lines = vec!["   ", "  fn test()  ", ""];
        let content = extractor.create_symbol_content("test", &lines, 2);
        assert_eq!(content, "fn test()", "Should trim whitespace");

        // Test with out of bounds line index
        let lines = vec!["fn test()"];
        let content = extractor.create_symbol_content("test", &lines, 10);
        assert_eq!(
            content, "test",
            "Should return symbol name for out of bounds"
        );

        // Test with zero line index
        let lines = vec!["fn test()"];
        let content = extractor.create_symbol_content("test", &lines, 0);
        assert_eq!(content, "test", "Should return symbol name for zero index");

        // Test with empty line content
        let lines = vec![""];
        let content = extractor.create_symbol_content("test", &lines, 1);
        assert_eq!(content, "test", "Should return symbol name for empty line");
    }

    #[test]
    fn test_symbol_types_mapping() {
        let mut extractor = SymbolExtractor::new().expect("Failed to create extractor");

        // Test code with various symbol types
        let rust_code = r#"
pub fn my_function() {}
pub struct MyStruct {}
pub enum MyEnum { A, B }
pub const MY_CONST: i32 = 42;
pub static MY_STATIC: i32 = 42;
pub type MyType = String;
pub mod my_module {}

impl MyStruct {
    pub fn my_method(&self) {}
}

pub struct FieldStruct {
    pub my_field: String,
}
"#;

        let symbols = extractor
            .extract_symbols_from_content(rust_code, "test.rs".to_string())
            .expect("Failed to extract symbols");

        // Check that we have the expected symbol types
        let symbol_types: std::collections::HashSet<SymbolType> =
            symbols.iter().map(|s| s.symbol_type).collect();

        assert!(
            symbol_types.contains(&SymbolType::Function),
            "Should have function symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Struct),
            "Should have struct symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Enum),
            "Should have enum symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Constant),
            "Should have constant symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Variable),
            "Should have static variable symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Type),
            "Should have type symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Module),
            "Should have module symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Method),
            "Should have method symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Field),
            "Should have field symbols"
        );
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
