use super::tree_sitter_config::{create_query, get_language_config};
use crate::types::{CodeSymbol, SymbolType};
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Parser, QueryCursor};

pub struct SymbolExtractor {
    verbose: bool,
}

impl SymbolExtractor {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// Extract symbols from source code using Tree-sitter
    pub fn extract_symbols(&self, source: &str, file_path: &Path) -> Result<Vec<CodeSymbol>> {
        let mut symbols = Vec::new();

        // Get file extension for Tree-sitter language selection
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let config = match get_language_config(extension) {
            Some(config) => config,
            None => return Ok(symbols), // Unsupported language
        };

        // Set up Tree-sitter parser
        let mut parser = Parser::new();
        if parser.set_language(config.language).is_err() {
            if self.verbose {
                eprintln!(
                    "Failed to set Tree-sitter language for {}",
                    file_path.display()
                );
            }
            return Ok(symbols);
        }

        // Parse the source code
        let tree = match parser.parse(source, None) {
            Some(tree) => tree,
            None => {
                if self.verbose {
                    eprintln!("Failed to parse {} with Tree-sitter", file_path.display());
                }
                return Ok(symbols);
            }
        };

        // Create and execute query
        let query = match create_query(&config) {
            Ok(query) => query,
            Err(e) => {
                if self.verbose {
                    eprintln!(
                        "Failed to create Tree-sitter query for {}: {}",
                        file_path.display(),
                        e
                    );
                }
                return Ok(symbols);
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
                    let symbol_type = self.map_capture_to_symbol_type(capture_name);

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

        Ok(symbols)
    }

    /// Map Tree-sitter capture names to SymbolType
    fn map_capture_to_symbol_type(&self, capture_name: &str) -> SymbolType {
        match capture_name {
            "function" | "method" | "arrow" | "constructor" => SymbolType::Function,
            "class" | "struct" | "interface" | "trait" => SymbolType::Class,
            "enum" | "type" | "object" => SymbolType::Type,
            "const" | "static" => SymbolType::Constant,
            "variable" | "field" | "use" | "module" | "impl" => SymbolType::Variable,
            _ => SymbolType::Variable, // Default fallback
        }
    }
}
