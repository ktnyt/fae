use super::common::extract_symbols_by_unified_query;
use super::LanguageExtractor;
use crate::symbol_index::SymbolMetadata;
use crate::types::SymbolType;
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Tree};

pub struct RustExtractor;

/// Rust用シンボルタイプリゾルバー
fn rust_resolver(capture_name: &str) -> Option<SymbolType> {
    match capture_name {
        "struct" => Some(SymbolType::Class),
        "enum" => Some(SymbolType::Type),
        "function" => Some(SymbolType::Function),
        "constant" => Some(SymbolType::Constant),
        _ => None,
    }
}

impl LanguageExtractor for RustExtractor {
    fn language() -> Language {
        tree_sitter_rust::language()
    }
    
    fn matches_extension(extension: &str) -> bool {
        extension == "rs"
    }
    
    fn extract_symbols(
        file_path: &Path,
        content: &str,
        tree: &Tree,
        language: Language,
    ) -> Result<Vec<SymbolMetadata>> {
        // Rust用統合クエリ
        let query_text = r#"
            (struct_item name: (type_identifier) @struct)
            (enum_item name: (type_identifier) @enum)
            (function_item name: (identifier) @function)
            (const_item name: (identifier) @constant)
        "#;
        
        extract_symbols_by_unified_query(
            file_path,
            content,
            tree,
            language,
            query_text,
            rust_resolver,
        )
    }
}