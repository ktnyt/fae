//! JavaScript language symbol extraction
//!
//! This module provides symbol extraction for JavaScript source files using tree-sitter.

use super::{LanguageConfig, LanguageExtractor};
use crate::actors::types::SymbolType;
use tree_sitter::Query;

/// JavaScript language symbol extractor
pub struct JavaScriptExtractor;

impl LanguageExtractor for JavaScriptExtractor {
    fn create_config() -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>> {
        let language = tree_sitter_javascript::language();

        // Tree-sitter query for JavaScript symbols (enhanced)
        let query_source = r#"
        ; Function declarations
        (function_declaration
          name: (identifier) @function.name)
        
        ; Function expressions with names
        (function_expression
          name: (identifier) @function.name)
        
        ; Arrow functions assigned to variables (treated as functions)
        (variable_declarator
          name: (identifier) @function.name
          value: (arrow_function))
        
        ; Class declarations
        (class_declaration
          name: (identifier) @class.name)
        
        ; Variable declarations (let, var, const all as variables for now)
        (variable_declarator
          name: (identifier) @variable.name)
        
        ; Function parameters
        (function_declaration
          parameters: (formal_parameters
            (identifier) @parameter.name))
        
        ; Arrow function parameters
        (arrow_function
          parameters: (formal_parameters
            (identifier) @parameter.name))
        
        ; Import specifiers (as modules)
        (import_statement
          (import_clause
            (named_imports
              (import_specifier
                name: (identifier) @module.name))))
        
        ; Default imports (as modules)
        (import_statement
          (import_clause
            (identifier) @module.name))
        "#;

        let query = Query::new(&language, query_source)
            .map_err(|e| format!("Failed to parse JavaScript query: {}", e))?;

        Ok(LanguageConfig { language, query })
    }

    fn supports_extension(extension: &str) -> bool {
        matches!(extension, "js" | "mjs" | "cjs")
    }

    fn language_name() -> &'static str {
        "JavaScript"
    }

    fn map_capture_to_symbol_type(capture_name: &str) -> Option<SymbolType> {
        match capture_name {
            "function.name" => Some(SymbolType::Function),
            "class.name" => Some(SymbolType::Class),
            "variable.name" => Some(SymbolType::Variable),
            "parameter.name" => Some(SymbolType::Parameter),
            "module.name" => Some(SymbolType::Module),
            _ => None, // Skip unknown captures
        }
    }
}

impl JavaScriptExtractor {
    /// Get supported file extensions for JavaScript
    pub fn get_supported_extensions() -> &'static [&'static str] {
        &["js", "mjs", "cjs"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actors::types::SymbolType;
    use tree_sitter::Parser;

    #[test]
    fn test_javascript_extractor_config_creation() {
        let config = JavaScriptExtractor::create_config();
        assert!(
            config.is_ok(),
            "Should create JavaScript config successfully"
        );
    }

    #[test]
    fn test_javascript_extension_support() {
        assert!(JavaScriptExtractor::supports_extension("js"));
        assert!(JavaScriptExtractor::supports_extension("mjs"));
        assert!(JavaScriptExtractor::supports_extension("cjs"));
        assert!(!JavaScriptExtractor::supports_extension("rs"));
        assert!(!JavaScriptExtractor::supports_extension("py"));
    }

    #[test]
    fn test_javascript_language_name() {
        assert_eq!(JavaScriptExtractor::language_name(), "JavaScript");
    }

    #[test]
    fn test_javascript_symbol_type_mapping() {
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("function.name"),
            Some(SymbolType::Function)
        );
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("class.name"),
            Some(SymbolType::Class)
        );
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("variable.name"),
            Some(SymbolType::Variable)
        );
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("parameter.name"),
            Some(SymbolType::Parameter)
        );
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("module.name"),
            Some(SymbolType::Module)
        );
        assert_eq!(
            JavaScriptExtractor::map_capture_to_symbol_type("unknown.capture"),
            None
        );
    }

    #[test]
    fn test_javascript_symbol_extraction() {
        let mut parser = Parser::new();

        let javascript_code = r#"
// Function declarations
function greet(name) {
    return "Hello " + name;
}

// Function expressions
const namedFunc = function myFunction() {
    return "named function";
};

// Arrow functions
const arrowFunc = (x, y) => x + y;

// Variable declarations
let userName = "John";
var userAge = 30;

// Const declarations
const MAX_USERS = 100;
const API_URL = "https://api.example.com";

// Class declarations
class User {
    // Class fields
    name = "";
    age = 0;
    
    constructor(name, age) {
        this.name = name;
        this.age = age;
    }
    
    // Methods
    getName() {
        return this.name;
    }
    
    setAge(newAge) {
        this.age = newAge;
    }
}

// Object with methods
const utils = {
    format: function(text) {
        return text.toUpperCase();
    },
    
    parse: (data) => JSON.parse(data)
};

// Export functions
export function exportedFunction() {
    return "exported";
}

// Export classes
export class ExportedClass {
    method() {
        return "method";
    }
}

// Export constants
export const CONFIG = { debug: true };

// Import statements
import { useState, useEffect } from 'react';
import defaultExport from './module';
"#;

        let symbols = JavaScriptExtractor::extract_symbols(&mut parser, javascript_code, "test.js")
            .expect("Failed to extract JavaScript symbols");

        assert!(!symbols.is_empty(), "Should extract some symbols");

        // Check that we found the expected symbol types
        let symbol_types: std::collections::HashSet<SymbolType> =
            symbols.iter().map(|s| s.symbol_type).collect();

        // Check for expected symbol types
        assert!(
            symbol_types.contains(&SymbolType::Function),
            "Should have function symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Class),
            "Should have class symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Variable),
            "Should have variable symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Parameter),
            "Should have parameter symbols"
        );
        assert!(
            symbol_types.contains(&SymbolType::Module),
            "Should have module symbols"
        );

        // Check specific symbols
        let symbol_names: Vec<String> = symbols.iter().map(|s| s.content.clone()).collect();
        println!("Extracted JavaScript symbols: {:?}", symbol_names);

        // Verify we have some expected function and class names
        let has_greet_function = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Function && s.content.contains("greet"));
        let has_user_class = symbols
            .iter()
            .any(|s| s.symbol_type == SymbolType::Class && s.content.contains("User"));
        assert!(has_greet_function, "Should find greet function");
        assert!(has_user_class, "Should find User class");
    }

    #[test]
    fn test_javascript_file_extensions() {
        let mut parser = Parser::new();

        let js_code = r#"
function testFunction() {
    return "test";
}
"#;

        // Test .js extension
        let symbols_js = JavaScriptExtractor::extract_symbols(&mut parser, js_code, "test.js")
            .expect("Failed to extract from .js file");
        assert!(!symbols_js.is_empty(), "Should extract from .js files");

        // Test .mjs extension
        let symbols_mjs = JavaScriptExtractor::extract_symbols(&mut parser, js_code, "test.mjs")
            .expect("Failed to extract from .mjs file");
        assert!(!symbols_mjs.is_empty(), "Should extract from .mjs files");

        // Test .cjs extension
        let symbols_cjs = JavaScriptExtractor::extract_symbols(&mut parser, js_code, "test.cjs")
            .expect("Failed to extract from .cjs file");
        assert!(!symbols_cjs.is_empty(), "Should extract from .cjs files");
    }

    #[test]
    fn test_javascript_create_symbol_content() {
        let lines = vec!["", "function testFunction() {", "    return \"test\";", "}"];
        let content = JavaScriptExtractor::create_symbol_content("testFunction", &lines, 2);
        assert_eq!(
            content, "function testFunction() {",
            "Should return line content"
        );

        // Test with class declaration
        let lines = vec!["class TestClass {", "    constructor() {}", "}"];
        let content = JavaScriptExtractor::create_symbol_content("TestClass", &lines, 1);
        assert_eq!(
            content, "class TestClass {",
            "Should return class line content"
        );
    }
}
