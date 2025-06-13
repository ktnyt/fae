use crate::languages::{detect_language, extract_symbols_for_language};
use crate::symbol_index::SymbolMetadata;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tree_sitter::Parser;

/// ファイルからTree-sitterを使ってシンボルを抽出
pub fn extract_symbols_from_file(file_path: &Path) -> Result<Vec<SymbolMetadata>> {
    // ファイル拡張子による言語検出
    let language = detect_language(file_path)?;
    let Some(language) = language else {
        // 未対応の言語の場合は空のVecを返す
        return Ok(Vec::new());
    };
    
    // ファイル内容を読み込み
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    
    // 空ファイルの場合は早期リターン
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    // Tree-sitterパーサーでAST解析
    let mut parser = Parser::new();
    parser.set_language(language)
        .with_context(|| "Failed to set Tree-sitter language")?;
    
    let tree = parser.parse(&content, None)
        .with_context(|| "Failed to parse file with Tree-sitter")?;
    
    // 言語別のシンボル抽出
    let symbols = extract_symbols_for_language(file_path, &content, &tree)?;
    
    Ok(symbols)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_extract_empty_file() -> Result<()> {
        let temp_file = NamedTempFile::with_suffix(".ts")?;
        // 空ファイル
        
        let symbols = extract_symbols_from_file(temp_file.path())?;
        assert_eq!(symbols.len(), 0);
        
        Ok(())
    }
}