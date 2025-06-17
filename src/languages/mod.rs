//! Language-specific symbol extraction modules
//!
//! This module provides a common interface for symbol extraction across different
//! programming languages using tree-sitter AST parsing.

pub mod rust;
pub mod javascript;
pub mod python;

use crate::actors::types::{Symbol, SymbolType};
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

/// Configuration for a specific programming language
pub struct LanguageConfig {
    pub language: Language,
    pub query: Query,
}

/// Common trait for language-specific symbol extractors
pub trait LanguageExtractor: Send + Sync {
    /// Get the language configuration (language grammar and queries)
    fn get_config() -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>>;

    /// Check if a file extension is supported by this language
    fn supports_extension(extension: &str) -> bool;

    /// Get the language name for debugging/logging
    fn language_name() -> &'static str;

    /// Extract symbols from source code content using this language's parser
    fn extract_symbols(
        parser: &mut Parser,
        config: &LanguageConfig,
        content: &str,
        filepath: &str,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>> {
        // Set parser language
        parser
            .set_language(&config.language)
            .map_err(|e| format!("Failed to set parser language for {}: {}", Self::language_name(), e))?;

        // Parse the content
        let tree = parser
            .parse(content, None)
            .ok_or_else(|| format!("Failed to parse {} source code", Self::language_name()))?;

        // Extract symbols using queries
        Self::extract_symbols_with_query(content, filepath, &tree, &config.query)
    }

    /// Extract symbols using tree-sitter queries (shared implementation)
    fn extract_symbols_with_query(
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
                if let Some(symbol_type) = Self::map_capture_to_symbol_type(capture_name) {
                    // Create symbol with context information
                    let symbol_content = Self::create_symbol_content(&symbol_text, &lines, line as usize);

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
        }

        Ok(symbols)
    }

    /// Map tree-sitter capture names to symbol types (language-specific)
    fn map_capture_to_symbol_type(capture_name: &str) -> Option<SymbolType>;

    /// Create symbol content with surrounding context (shared implementation)
    fn create_symbol_content(symbol_name: &str, lines: &[&str], line_index: usize) -> String {
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

/// Registry for all supported language extractors
pub struct LanguageRegistry;

impl LanguageRegistry {
    /// Get all supported file extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        let mut extensions = Vec::new();
        extensions.extend(rust::RustExtractor::get_supported_extensions());
        extensions.extend(javascript::JavaScriptExtractor::get_supported_extensions());
        extensions.extend(python::PythonExtractor::get_supported_extensions());
        extensions
    }

    /// Check if a file extension is supported by any language
    pub fn is_extension_supported(extension: &str) -> bool {
        rust::RustExtractor::supports_extension(extension)
            || javascript::JavaScriptExtractor::supports_extension(extension)
            || python::PythonExtractor::supports_extension(extension)
    }

    /// Get the appropriate language extractor for a file path
    pub fn get_extractor_for_path(file_path: &Path) -> Option<Box<dyn LanguageExtractorDyn>> {
        if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
            if rust::RustExtractor::supports_extension(extension) {
                return Some(Box::new(rust::RustExtractor));
            }
            if javascript::JavaScriptExtractor::supports_extension(extension) {
                return Some(Box::new(javascript::JavaScriptExtractor));
            }
            if python::PythonExtractor::supports_extension(extension) {
                return Some(Box::new(python::PythonExtractor));
            }
        }
        None
    }
}

/// Dynamic trait object interface for language extractors
pub trait LanguageExtractorDyn: Send + Sync {
    fn get_config(&self) -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>>;
    fn language_name(&self) -> &'static str;
    fn extract_symbols(
        &self,
        parser: &mut Parser,
        content: &str,
        filepath: &str,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Blanket implementation for all LanguageExtractor implementors
impl<T: LanguageExtractor> LanguageExtractorDyn for T {
    fn get_config(&self) -> Result<LanguageConfig, Box<dyn std::error::Error + Send + Sync>> {
        T::get_config()
    }

    fn language_name(&self) -> &'static str {
        T::language_name()
    }

    fn extract_symbols(
        &self,
        parser: &mut Parser,
        content: &str,
        filepath: &str,
    ) -> Result<Vec<Symbol>, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.get_config()?;
        T::extract_symbols(parser, &config, content, filepath)
    }
}