use super::common::extract_symbols_by_unified_query;
use super::LanguageExtractor;
use crate::symbol_index::SymbolMetadata;
use crate::types::SymbolType;
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Tree};

pub struct TypeScriptExtractor;

/// TypeScript用シンボルタイプリゾルバー
fn typescript_resolver(capture_name: &str) -> Option<SymbolType> {
    match capture_name {
        "interface" => Some(SymbolType::Interface),
        "class" => Some(SymbolType::Class),
        "function" => Some(SymbolType::Function),
        "method" => Some(SymbolType::Function),
        "constant" => Some(SymbolType::Constant),
        _ => None,
    }
}

impl LanguageExtractor for TypeScriptExtractor {
    fn language() -> Language {
        tree_sitter_typescript::language_typescript()
    }
    
    fn matches_extension(extension: &str) -> bool {
        matches!(extension, "ts" | "tsx")
    }
    
    fn extract_symbols(
        file_path: &Path,
        content: &str,
        tree: &Tree,
        language: Language,
    ) -> Result<Vec<SymbolMetadata>> {
        // TypeScript用統合クエリ
        let query_text = r#"
            (interface_declaration name: (type_identifier) @interface)
            (class_declaration name: (type_identifier) @class)
            (function_declaration name: (identifier) @function)
            (method_definition name: (property_identifier) @method)
            (lexical_declaration (variable_declarator name: (identifier) @constant))
        "#;
        
        extract_symbols_by_unified_query(
            file_path,
            content,
            tree,
            language,
            query_text,
            typescript_resolver,
        )
    }
}