//! Rust language symbol extraction
//!
//! This module provides symbol extraction for Rust source files using tree-sitter.

use super::{LanguageConfig, LanguageExtractor};
use crate::actors::types::SymbolType;
use tree_sitter::Query;

/// Rust language symbol extractor
pub struct RustExtractor;

impl LanguageExtractor for RustExtractor {
    fn create_config() -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>> {
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

    fn supports_extension(extension: &str) -> bool {
        matches!(extension, "rs")
    }

    fn language_name() -> &'static str {
        "Rust"
    }

    fn map_capture_to_symbol_type(capture_name: &str) -> Option<SymbolType> {
        match capture_name {
            "function.name" => Some(SymbolType::Function),
            "struct.name" => Some(SymbolType::Struct),
            "enum.name" => Some(SymbolType::Enum),
            "method.name" => Some(SymbolType::Method),
            "constant.name" => Some(SymbolType::Constant),
            "static.name" => Some(SymbolType::Variable),
            "type.name" => Some(SymbolType::Type),
            "module.name" => Some(SymbolType::Module),
            "field.name" => Some(SymbolType::Field),
            "variable.name" => Some(SymbolType::Variable),
            "parameter.name" => Some(SymbolType::Parameter),
            _ => None, // Skip unknown captures
        }
    }
}

impl RustExtractor {
    /// Get supported file extensions for Rust
    pub fn get_supported_extensions() -> &'static [&'static str] {
        &["rs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SymbolType;
    use tree_sitter::Parser;

    #[test]
    fn test_rust_extractor_config_creation() {
        let config = RustExtractor::create_config();
        assert!(config.is_ok(), "Should create Rust config successfully");
    }

    #[test]
    fn test_rust_extension_support() {
        assert!(RustExtractor::supports_extension("rs"));
        assert!(!RustExtractor::supports_extension("js"));
        assert!(!RustExtractor::supports_extension("py"));
    }

    #[test]
    fn test_rust_language_name() {
        assert_eq!(RustExtractor::language_name(), "Rust");
    }

    #[test]
    fn test_rust_symbol_type_mapping() {
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("function.name"),
            Some(SymbolType::Function)
        );
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("struct.name"),
            Some(SymbolType::Struct)
        );
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("enum.name"),
            Some(SymbolType::Enum)
        );
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("method.name"),
            Some(SymbolType::Method)
        );
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("constant.name"),
            Some(SymbolType::Constant)
        );
        assert_eq!(
            RustExtractor::map_capture_to_symbol_type("unknown.capture"),
            None
        );
    }

    #[test]
    fn test_rust_symbol_extraction() {
        let mut parser = Parser::new();

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

        let symbols = RustExtractor::extract_symbols(&mut parser, rust_code, "test.rs")
            .expect("Failed to extract Rust symbols");

        assert!(!symbols.is_empty(), "Should extract some symbols");

        // Check that we found the expected symbol types
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
            symbol_types.contains(&SymbolType::Field),
            "Should have field symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Variable),
            "Should have variable symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Parameter),
            "Should have parameter symbols"
        );
    }

    #[test]
    fn test_rust_create_symbol_content() {
        let lines = vec!["", "pub fn test_function() {", "    println!(\"test\");", "}"];
        let content = RustExtractor::create_symbol_content("test_function", &lines, 2);
        assert_eq!(content, "pub fn test_function() {", "Should return line content");

        // Test with empty line
        let lines = vec![""];
        let content = RustExtractor::create_symbol_content("test", &lines, 1);
        assert_eq!(content, "test", "Should return symbol name for empty line");

        // Test with out of bounds
        let lines = vec!["fn test()"];
        let content = RustExtractor::create_symbol_content("test", &lines, 10);
        assert_eq!(
            content, "test",
            "Should return symbol name for out of bounds"
        );
    }
}