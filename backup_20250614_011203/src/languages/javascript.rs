use super::common::extract_symbols_by_unified_query;
use super::LanguageExtractor;
use crate::symbol_index::SymbolMetadata;
use crate::types::SymbolType;
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Tree};

pub struct JavaScriptExtractor;

/// JavaScript用シンボルタイプリゾルバー
fn javascript_resolver(capture_name: &str) -> Option<SymbolType> {
    match capture_name {
        "class" => Some(SymbolType::Class),
        "function" => Some(SymbolType::Function),
        "method" => Some(SymbolType::Function),
        "constant" => Some(SymbolType::Constant),
        "arrow_func" => Some(SymbolType::Function),
        _ => None,
    }
}

impl LanguageExtractor for JavaScriptExtractor {
    fn language() -> Language {
        tree_sitter_javascript::language()
    }
    
    fn matches_extension(extension: &str) -> bool {
        matches!(extension, "js" | "jsx")
    }
    
    fn extract_symbols(
        file_path: &Path,
        content: &str,
        tree: &Tree,
        language: Language,
    ) -> Result<Vec<SymbolMetadata>> {
        // JavaScript用統合クエリ
        let query_text = r#"
            (class_declaration name: (identifier) @class)
            (function_declaration name: (identifier) @function)
            (method_definition name: (property_identifier) @method)
            (lexical_declaration (variable_declarator name: (identifier) @constant))
            (variable_declaration (variable_declarator name: (identifier) @arrow_func value: (arrow_function)))
        "#;
        
        extract_symbols_by_unified_query(
            file_path,
            content,
            tree,
            language,
            query_text,
            javascript_resolver,
        )
    }
}