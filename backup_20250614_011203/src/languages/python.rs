use super::common::extract_symbols_by_unified_query;
use super::LanguageExtractor;
use crate::symbol_index::SymbolMetadata;
use crate::types::SymbolType;
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Tree};

pub struct PythonExtractor;

/// Python用シンボルタイプリゾルバー
fn python_resolver(capture_name: &str) -> Option<SymbolType> {
    match capture_name {
        "class" => Some(SymbolType::Class),
        "function" => Some(SymbolType::Function),
        "constant" => Some(SymbolType::Constant),
        _ => None,
    }
}

impl LanguageExtractor for PythonExtractor {
    fn language() -> Language {
        tree_sitter_python::language()
    }
    
    fn matches_extension(extension: &str) -> bool {
        extension == "py"
    }
    
    fn extract_symbols(
        file_path: &Path,
        content: &str,
        tree: &Tree,
        language: Language,
    ) -> Result<Vec<SymbolMetadata>> {
        // Python用統合クエリ
        let query_text = r#"
            (class_definition name: (identifier) @class)
            (function_definition name: (identifier) @function)
            (assignment left: (identifier) @constant)
        "#;
        
        extract_symbols_by_unified_query(
            file_path,
            content,
            tree,
            language,
            query_text,
            python_resolver,
        )
    }
}