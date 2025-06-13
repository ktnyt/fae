use crate::symbol_index::SymbolMetadata;
use crate::types::SymbolType;
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::{Language, Query, QueryCursor, Tree};

/// キャプチャ名に基づいてシンボルタイプを決定するタイプ
pub type SymbolTypeResolver = fn(&str) -> Option<SymbolType>;

/// 統合クエリでシンボルを抽出する最適化されたヘルパー
pub fn extract_symbols_by_unified_query(
    file_path: &Path,
    content: &str,
    tree: &Tree,
    language: Language,
    query_text: &str,
    resolver: SymbolTypeResolver,
) -> Result<Vec<SymbolMetadata>> {
    let mut symbols = Vec::new();
    
    // Tree-sitterクエリを作成
    let query = Query::new(language, query_text)
        .with_context(|| format!("Failed to create Tree-sitter query: {}", query_text))?;
    
    let mut cursor = QueryCursor::new();
    let content_bytes = content.as_bytes();
    let matches = cursor.matches(&query, tree.root_node(), content_bytes);
    
    for m in matches {
        for capture in m.captures {
            let node = capture.node;
            let start_point = node.start_position();
            let line = start_point.row + 1; // 1ベースに変換
            let column = start_point.column + 1; // 1ベースに変換
            
            // キャプチャ名からシンボルタイプを取得
            let capture_name = &query.capture_names()[capture.index as usize];
            let Some(symbol_type) = resolver(capture_name) else {
                continue; // 未知のキャプチャ名はスキップ
            };
            
            // ノードのテキストを取得
            let symbol_name = node.utf8_text(content_bytes)
                .with_context(|| "Failed to extract symbol name")?
                .to_string();
            
            symbols.push(SymbolMetadata {
                name: symbol_name,
                file_path: file_path.to_path_buf(),
                line: line as u32,
                column: column as u32,
                symbol_type,
            });
        }
    }
    
    Ok(symbols)
}

