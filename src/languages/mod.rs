use crate::symbol_index::SymbolMetadata;
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Tree};

pub mod common;
pub mod typescript;
pub mod javascript;
pub mod python;
pub mod rust_lang;

/// 言語ごとのシンボル抽出トレイト
pub trait LanguageExtractor {
    /// Tree-sitter言語を取得
    fn language() -> Language;
    
    /// ファイル拡張子のマッチング
    fn matches_extension(extension: &str) -> bool;
    
    /// シンボル抽出実行
    fn extract_symbols(
        file_path: &Path,
        content: &str,
        tree: &Tree,
        language: Language,
    ) -> Result<Vec<SymbolMetadata>>;
}

/// 言語検出とシンボル抽出の統合インターフェース
pub fn extract_symbols_for_language(
    file_path: &Path,
    content: &str,
    tree: &Tree,
) -> Result<Vec<SymbolMetadata>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    match extension {
        ext if typescript::TypeScriptExtractor::matches_extension(ext) => {
            typescript::TypeScriptExtractor::extract_symbols(
                file_path,
                content,
                tree,
                typescript::TypeScriptExtractor::language(),
            )
        }
        ext if javascript::JavaScriptExtractor::matches_extension(ext) => {
            javascript::JavaScriptExtractor::extract_symbols(
                file_path,
                content,
                tree,
                javascript::JavaScriptExtractor::language(),
            )
        }
        ext if python::PythonExtractor::matches_extension(ext) => {
            python::PythonExtractor::extract_symbols(
                file_path,
                content,
                tree,
                python::PythonExtractor::language(),
            )
        }
        ext if rust_lang::RustExtractor::matches_extension(ext) => {
            rust_lang::RustExtractor::extract_symbols(
                file_path,
                content,
                tree,
                rust_lang::RustExtractor::language(),
            )
        }
        _ => Ok(Vec::new()), // 未対応言語
    }
}

/// ファイル拡張子から言語を検出
pub fn detect_language(file_path: &Path) -> Result<Option<Language>> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    let language = match extension {
        ext if typescript::TypeScriptExtractor::matches_extension(ext) => {
            Some(typescript::TypeScriptExtractor::language())
        }
        ext if javascript::JavaScriptExtractor::matches_extension(ext) => {
            Some(javascript::JavaScriptExtractor::language())
        }
        ext if python::PythonExtractor::matches_extension(ext) => {
            Some(python::PythonExtractor::language())
        }
        ext if rust_lang::RustExtractor::matches_extension(ext) => {
            Some(rust_lang::RustExtractor::language())
        }
        _ => None,
    };

    Ok(language)
}