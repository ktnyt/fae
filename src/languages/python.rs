//! Python language symbol extraction
//!
//! This module provides symbol extraction for Python source files using tree-sitter.

use super::{LanguageConfig, LanguageExtractor};
use crate::actors::types::SymbolType;
use tree_sitter::Query;

/// Python language symbol extractor
pub struct PythonExtractor;

impl LanguageExtractor for PythonExtractor {
    fn get_config() -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>> {
        let language = tree_sitter_python::language();

        // Tree-sitter query for Python symbols
        let query_source = r#"
        ; Function definitions
        (function_definition
          name: (identifier) @function.name) @function.definition
        
        ; Class definitions
        (class_definition
          name: (identifier) @class.name) @class.definition
        
        ; Method definitions (functions inside classes)
        (class_definition
          body: (block
            (function_definition
              name: (identifier) @method.name) @method.definition))
        
        ; Variable assignments
        (assignment
          left: (identifier) @variable.name) @variable.definition
        
        ; Imports
        (import_statement
          name: (dotted_name) @import.name) @import.definition
        
        (import_from_statement
          module_name: (dotted_name) @import.name) @import.definition
        
        ; Function parameters
        (function_definition
          parameters: (parameters
            (identifier) @parameter.name))
            
        (function_definition
          parameters: (parameters
            (default_parameter
              name: (identifier) @parameter.name)))
        
        ; Method parameters
        (class_definition
          body: (block
            (function_definition
              parameters: (parameters
                (identifier) @parameter.name))))
                
        (class_definition
          body: (block
            (function_definition
              parameters: (parameters
                (default_parameter
                  name: (identifier) @parameter.name)))))
        
        ; Constants (uppercase variables)
        (assignment
          left: (identifier) @constant.name
          (#match? @constant.name "^[A-Z_][A-Z0-9_]*$")) @constant.definition
        "#;

        let query = Query::new(&language, query_source)
            .map_err(|e| format!("Failed to parse Python query: {}", e))?;

        Ok(LanguageConfig { language, query })
    }

    fn supports_extension(extension: &str) -> bool {
        matches!(extension, "py" | "pyw" | "pyi")
    }

    fn language_name() -> &'static str {
        "Python"
    }

    fn map_capture_to_symbol_type(capture_name: &str) -> Option<SymbolType> {
        match capture_name {
            "function.name" => Some(SymbolType::Function),
            "class.name" => Some(SymbolType::Class),
            "method.name" => Some(SymbolType::Method),
            "variable.name" => Some(SymbolType::Variable),
            "constant.name" => Some(SymbolType::Constant),
            "parameter.name" => Some(SymbolType::Parameter),
            "import.name" => Some(SymbolType::Module),
            "decorator.name" => Some(SymbolType::Function), // Decorators are function-like
            _ => None, // Skip unknown captures
        }
    }
}

impl PythonExtractor {
    /// Get supported file extensions for Python
    pub fn get_supported_extensions() -> &'static [&'static str] {
        &["py", "pyw", "pyi"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SymbolType;
    use tree_sitter::Parser;

    #[test]
    fn test_python_extractor_config_creation() {
        let config = PythonExtractor::get_config();
        assert!(config.is_ok(), "Should create Python config successfully");
    }

    #[test]
    fn test_python_extension_support() {
        assert!(PythonExtractor::supports_extension("py"));
        assert!(PythonExtractor::supports_extension("pyw"));
        assert!(PythonExtractor::supports_extension("pyi"));
        assert!(!PythonExtractor::supports_extension("rs"));
        assert!(!PythonExtractor::supports_extension("js"));
    }

    #[test]
    fn test_python_language_name() {
        assert_eq!(PythonExtractor::language_name(), "Python");
    }

    #[test]
    fn test_python_symbol_type_mapping() {
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("function.name"),
            Some(SymbolType::Function)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("class.name"),
            Some(SymbolType::Class)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("method.name"),
            Some(SymbolType::Method)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("variable.name"),
            Some(SymbolType::Variable)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("constant.name"),
            Some(SymbolType::Constant)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("parameter.name"),
            Some(SymbolType::Parameter)
        );
        assert_eq!(
            PythonExtractor::map_capture_to_symbol_type("unknown.capture"),
            None
        );
    }

    #[test]
    fn test_python_symbol_extraction() {
        let mut parser = Parser::new();
        let config = PythonExtractor::get_config().expect("Failed to get Python config");

        let python_code = r#"
import os

MAX_SIZE = 100
default_name = "test"

class User:
    def __init__(self, name, age = 0):
        self.name = name
        self.age = age
    
    def greet(self, message = "Hello"):
        full_message = message + ", " + self.name + "!"
        print(full_message)

def calculate_total(items):
    total = sum(items)
    return total

def main():
    user = User("Alice", 30)
    user.greet()
"#;

        let symbols = PythonExtractor::extract_symbols(&mut parser, &config, python_code, "test.py")
            .expect("Failed to extract Python symbols");

        assert!(!symbols.is_empty(), "Should extract some symbols");

        // Check that we found the expected symbol types
        let symbol_types: std::collections::HashSet<SymbolType> =
            symbols.iter().map(|s| s.symbol_type).collect();

        assert!(
            symbol_types.contains(&SymbolType::Function),
            "Should have function symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Class),
            "Should have class symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Method),
            "Should have method symbols"
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
    fn test_python_create_symbol_content() {
        let lines = vec!["", "def test_function():", "    print('test')", ""];
        let content = PythonExtractor::create_symbol_content("test_function", &lines, 2);
        assert_eq!(content, "def test_function():", "Should return line content");

        // Test with empty line
        let lines = vec![""];
        let content = PythonExtractor::create_symbol_content("test", &lines, 1);
        assert_eq!(content, "test", "Should return symbol name for empty line");

        // Test with out of bounds
        let lines = vec!["def test():"];
        let content = PythonExtractor::create_symbol_content("test", &lines, 10);
        assert_eq!(
            content, "test",
            "Should return symbol name for out of bounds"
        );
    }
}