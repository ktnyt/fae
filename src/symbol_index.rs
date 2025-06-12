use crate::types::SymbolType;
use anyhow::{Context, Result};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};

/// メモリ内シンボルインデックス（ファジー検索用）
pub struct SymbolIndex {
    /// ユニークなシンボル名（重複排除済み）
    symbol_names: Vec<String>,
    /// ファジー検索エンジン
    matcher: SkimMatcherV2,
}

/// ファジー検索結果
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// symbol_names のインデックス
    pub index: usize,
    /// マッチスコア
    pub score: i64,
    /// マッチしたシンボル名
    pub symbol_name: String,
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

/// メタデータストレージ（ソート済みバイナリファイル）
pub struct MetadataStorage {
    /// .fae ディレクトリパス
    cache_dir: PathBuf,
    /// メタデータファイルパス
    metadata_file: PathBuf,
    /// メモリ内メタデータ（一時的）
    metadata_cache: Option<Vec<SymbolMetadata>>,
}

impl SymbolIndex {
    /// 新しいシンボルインデックスを作成
    pub fn new() -> Self {
        Self {
            symbol_names: Vec::new(),
            matcher: SkimMatcherV2::default(),
        }
    }

    /// シンボルリストから構築（重複排除）
    pub fn from_symbols(symbols: Vec<SymbolMetadata>) -> Self {
        let mut unique_names = HashSet::new();
        for symbol in symbols {
            unique_names.insert(symbol.name);
        }

        let mut symbol_names: Vec<String> = unique_names.into_iter().collect();
        symbol_names.sort(); // アルファベット順ソート

        Self {
            symbol_names,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// ファジー検索を実行
    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();

        for (index, symbol_name) in self.symbol_names.iter().enumerate() {
            if let Some(score) = self.matcher.fuzzy_match(symbol_name, query) {
                hits.push(SearchHit {
                    index,
                    score,
                    symbol_name: symbol_name.clone(),
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
        self.symbol_names.len()
    }

    /// 空かどうか
    pub fn is_empty(&self) -> bool {
        self.symbol_names.is_empty()
    }

    /// 指定インデックスのシンボル名を取得
    pub fn get_symbol_name(&self, index: usize) -> Option<&str> {
        self.symbol_names.get(index).map(|s| s.as_str())
    }

    /// 全シンボル名を取得
    pub fn symbol_names(&self) -> &[String] {
        &self.symbol_names
    }
}

impl MetadataStorage {
    /// 新しいメタデータストレージを作成
    pub fn new(project_root: &Path) -> Result<Self> {
        let cache_dir = project_root.join(".fae");
        let metadata_file = cache_dir.join("symbols.bin");

        // キャッシュディレクトリを作成
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;

        Ok(Self {
            cache_dir,
            metadata_file,
            metadata_cache: None,
        })
    }

    /// メタデータを保存（アルファベット順ソート済み）
    pub fn save_metadata(&mut self, mut metadata: Vec<SymbolMetadata>) -> Result<()> {
        // アルファベット順でソート
        metadata.sort_by(|a, b| a.name.cmp(&b.name));

        // バイナリ形式で保存
        let file = fs::File::create(&self.metadata_file)
            .with_context(|| format!("Failed to create metadata file: {}", self.metadata_file.display()))?;
        
        let mut writer = BufWriter::new(file);
        
        // エントリ数を保存
        let count = metadata.len() as u32;
        writer.write_all(&count.to_le_bytes())?;

        // 各メタデータエントリを保存
        for entry in &metadata {
            self.write_metadata_entry(&mut writer, entry)?;
        }

        writer.flush()?;

        // メモリキャッシュを更新
        self.metadata_cache = Some(metadata);

        Ok(())
    }

    /// シンボル名でメタデータを検索（バイナリサーチ）
    pub fn find_metadata(&mut self, symbol_name: &str) -> Result<Vec<SymbolMetadata>> {
        // メモリキャッシュが無い場合は読み込み
        if self.metadata_cache.is_none() {
            self.load_metadata()?;
        }

        let metadata = self.metadata_cache.as_ref().unwrap();

        // バイナリサーチで範囲を特定
        let start_index = metadata.binary_search_by(|entry| entry.name.as_str().cmp(symbol_name))
            .unwrap_or_else(|insert_pos| insert_pos);

        let mut results = Vec::new();

        // 同名シンボルをすべて収集
        for entry in metadata.iter().skip(start_index) {
            if entry.name == symbol_name {
                results.push(entry.clone());
            } else {
                break; // ソート済みなので、違う名前が出たら終了
            }
        }

        Ok(results)
    }

    /// メタデータファイルが存在するか
    pub fn exists(&self) -> bool {
        self.metadata_file.exists()
    }

    /// キャッシュディレクトリのパスを取得
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// メタデータをメモリに読み込み
    fn load_metadata(&mut self) -> Result<()> {
        if !self.metadata_file.exists() {
            self.metadata_cache = Some(Vec::new());
            return Ok(());
        }

        let file = fs::File::open(&self.metadata_file)
            .with_context(|| format!("Failed to open metadata file: {}", self.metadata_file.display()))?;
        
        let mut reader = BufReader::new(file);

        // エントリ数を読み込み
        let mut count_bytes = [0u8; 4];
        reader.read_exact(&mut count_bytes)?;
        let count = u32::from_le_bytes(count_bytes) as usize;

        let mut metadata = Vec::with_capacity(count);

        // 各エントリを読み込み
        for _ in 0..count {
            let entry = self.read_metadata_entry(&mut reader)?;
            metadata.push(entry);
        }

        self.metadata_cache = Some(metadata);
        Ok(())
    }

    /// メタデータエントリをバイナリ形式で書き込み
    fn write_metadata_entry(&self, writer: &mut BufWriter<fs::File>, entry: &SymbolMetadata) -> Result<()> {
        // シンボル名
        let name_bytes = entry.name.as_bytes();
        writer.write_all(&(name_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(name_bytes)?;

        // ファイルパス
        let path_str = entry.file_path.to_string_lossy();
        let path_bytes = path_str.as_bytes();
        writer.write_all(&(path_bytes.len() as u32).to_le_bytes())?;
        writer.write_all(path_bytes)?;

        // 行・列番号
        writer.write_all(&entry.line.to_le_bytes())?;
        writer.write_all(&entry.column.to_le_bytes())?;

        // シンボル種別
        let symbol_type_u8 = match entry.symbol_type {
            SymbolType::Function => 0,
            SymbolType::Class => 1,
            SymbolType::Variable => 2,
            SymbolType::Constant => 3,
            SymbolType::Interface => 4,
            SymbolType::Type => 5,
        };
        writer.write_all(&[symbol_type_u8])?;

        Ok(())
    }

    /// メタデータエントリをバイナリ形式で読み込み
    fn read_metadata_entry(&self, reader: &mut BufReader<fs::File>) -> Result<SymbolMetadata> {
        // シンボル名
        let mut name_len_bytes = [0u8; 4];
        reader.read_exact(&mut name_len_bytes)?;
        let name_len = u32::from_le_bytes(name_len_bytes) as usize;
        
        let mut name_bytes = vec![0u8; name_len];
        reader.read_exact(&mut name_bytes)?;
        let name = String::from_utf8(name_bytes)?;

        // ファイルパス
        let mut path_len_bytes = [0u8; 4];
        reader.read_exact(&mut path_len_bytes)?;
        let path_len = u32::from_le_bytes(path_len_bytes) as usize;
        
        let mut path_bytes = vec![0u8; path_len];
        reader.read_exact(&mut path_bytes)?;
        let file_path = PathBuf::from(String::from_utf8(path_bytes)?);

        // 行・列番号
        let mut line_bytes = [0u8; 4];
        reader.read_exact(&mut line_bytes)?;
        let line = u32::from_le_bytes(line_bytes);

        let mut column_bytes = [0u8; 4];
        reader.read_exact(&mut column_bytes)?;
        let column = u32::from_le_bytes(column_bytes);

        // シンボル種別
        let mut symbol_type_bytes = [0u8; 1];
        reader.read_exact(&mut symbol_type_bytes)?;
        let symbol_type = match symbol_type_bytes[0] {
            0 => SymbolType::Function,
            1 => SymbolType::Class,
            2 => SymbolType::Variable,
            3 => SymbolType::Constant,
            4 => SymbolType::Interface,
            5 => SymbolType::Type,
            _ => return Err(anyhow::anyhow!("Invalid symbol type: {}", symbol_type_bytes[0])),
        };

        Ok(SymbolMetadata {
            name,
            file_path,
            line,
            column,
            symbol_type,
        })
    }
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
                name: "handleClick".to_string(), // 重複
                file_path: PathBuf::from("src/form.tsx"),
                line: 25,
                column: 12,
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

        let index = SymbolIndex::from_symbols(symbols);
        
        // 重複排除されているか確認
        assert_eq!(index.len(), 2);
        assert!(index.symbol_names().contains(&"handleClick".to_string()));
        assert!(index.symbol_names().contains(&"UserService".to_string()));
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
        assert_eq!(hits[0].symbol_name, "handleClick");
    }

    #[test]
    fn test_metadata_storage() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let mut storage = MetadataStorage::new(temp_dir.path())?;

        let metadata = vec![
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

        // 保存
        storage.save_metadata(metadata)?;
        assert!(storage.exists());

        // 検索（ソート順確認）
        let results = storage.find_metadata("A_function")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "A_function");

        let results = storage.find_metadata("B_function")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "B_function");

        // 存在しないシンボル
        let results = storage.find_metadata("NonExistent")?;
        assert_eq!(results.len(), 0);

        Ok(())
    }
}