use crate::types::SymbolType;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::path::PathBuf;

/// メモリ内シンボルインデックス（ファジー検索用）
pub struct SymbolIndex {
    /// シンボルメタデータ（完全情報）
    symbols: Vec<SymbolMetadata>,
    /// ファジー検索エンジン
    matcher: SkimMatcherV2,
}

/// ファジー検索結果（完全メタデータ付き）
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// symbols のインデックス
    pub index: usize,
    /// マッチスコア
    pub score: i64,
    /// 完全なシンボルメタデータ
    pub metadata: SymbolMetadata,
}

/// ディスク保存用シンボルメタデータ
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolMetadata {
    /// シンボル名
    pub name: String,
    /// ファイルパス（相対パス）
    pub file_path: PathBuf,
    /// 行番号（1ベース）
    pub line: u32,
    /// 列番号（1ベース）
    pub column: u32,
    /// シンボル種別
    pub symbol_type: SymbolType,
}


impl SymbolIndex {
    /// 新しいシンボルインデックスを作成
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            matcher: SkimMatcherV2::default(),
        }
    }

    /// シンボルリストから構築
    pub fn from_symbols(mut symbols: Vec<SymbolMetadata>) -> Self {
        // アルファベット順ソート
        symbols.sort_by(|a, b| a.name.cmp(&b.name));

        Self {
            symbols,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// ファジー検索を実行
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();

        for (index, symbol) in self.symbols.iter().enumerate() {
            if let Some(score) = self.matcher.fuzzy_match(&symbol.name, query) {
                hits.push(SearchHit {
                    index,
                    score,
                    metadata: symbol.clone(),
                });
            }
        }

        // スコア順でソート（降順）
        hits.sort_by(|a, b| b.score.cmp(&a.score));

        // 上位 N 件を返す
        hits.into_iter().take(limit).collect()
    }

    /// シンボル数を取得
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// 空かどうか
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// 指定インデックスのシンボルメタデータを取得
    pub fn get_symbol(&self, index: usize) -> Option<&SymbolMetadata> {
        self.symbols.get(index)
    }

    /// 全シンボルメタデータを取得
    pub fn symbols(&self) -> &[SymbolMetadata] {
        &self.symbols
    }
}


impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SymbolIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymbolIndex")
            .field("symbol_count", &self.symbols.len())
            .field("symbols", &self.symbols.iter().map(|s| &s.name).collect::<Vec<_>>())
            .finish()
    }
}

impl Clone for SymbolIndex {
    fn clone(&self) -> Self {
        Self {
            symbols: self.symbols.clone(),
            matcher: SkimMatcherV2::default(), // 新しいインスタンスを作成
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_index_creation() {
        let symbols = vec![
            SymbolMetadata {
                name: "handleClick".to_string(),
                file_path: PathBuf::from("src/button.tsx"),
                line: 10,
                column: 8,
                symbol_type: SymbolType::Function,
            },
            SymbolMetadata {
                name: "UserService".to_string(),
                file_path: PathBuf::from("src/user.ts"),
                line: 5,
                column: 7,
                symbol_type: SymbolType::Class,
            },
        ];

        let index = SymbolIndex::from_symbols(symbols.clone());
        
        // シンボル数の確認
        assert_eq!(index.len(), 2);
        
        // シンボルメタデータが正しく保存されているか確認
        let symbol_names: Vec<&str> = index.symbols().iter().map(|s| s.name.as_str()).collect();
        assert!(symbol_names.contains(&"handleClick"));
        assert!(symbol_names.contains(&"UserService"));
    }

    #[test]
    fn test_fuzzy_search() {
        let symbols = vec![
            SymbolMetadata {
                name: "handleClick".to_string(),
                file_path: PathBuf::from("src/button.tsx"),
                line: 10,
                column: 8,
                symbol_type: SymbolType::Function,
            },
            SymbolMetadata {
                name: "handleSubmit".to_string(),
                file_path: PathBuf::from("src/form.tsx"),
                line: 25,
                column: 12,
                symbol_type: SymbolType::Function,
            },
        ];

        let index = SymbolIndex::from_symbols(symbols);
        
        // ファジー検索テスト
        let hits = index.fuzzy_search("handle", 10);
        assert_eq!(hits.len(), 2);
        
        let hits = index.fuzzy_search("click", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].metadata.name, "handleClick");
        assert_eq!(hits[0].metadata.file_path, PathBuf::from("src/button.tsx"));
        assert_eq!(hits[0].metadata.line, 10);
    }

    #[test]
    fn test_metadata_direct_access() {
        let symbols = vec![
            SymbolMetadata {
                name: "B_function".to_string(),
                file_path: PathBuf::from("src/b.rs"),
                line: 1,
                column: 1,
                symbol_type: SymbolType::Function,
            },
            SymbolMetadata {
                name: "A_function".to_string(),
                file_path: PathBuf::from("src/a.rs"),
                line: 2,
                column: 2,
                symbol_type: SymbolType::Function,
            },
        ];

        let index = SymbolIndex::from_symbols(symbols);
        
        // アルファベット順ソート確認
        let sorted_symbols = index.symbols();
        assert_eq!(sorted_symbols[0].name, "A_function");
        assert_eq!(sorted_symbols[1].name, "B_function");
        
        // 検索結果に完全なメタデータが含まれるか確認
        let hits = index.fuzzy_search("A_function", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].metadata.name, "A_function");
        assert_eq!(hits[0].metadata.file_path, PathBuf::from("src/a.rs"));
        assert_eq!(hits[0].metadata.line, 2);
        assert_eq!(hits[0].metadata.column, 2);
    }
}