use crate::types::{CacheEntry, CachedFileInfo, CachedSymbol};
use crate::tree_sitter::extract_symbols_from_file;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::fs;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// LRUキャッシュのデフォルト設定
const DEFAULT_MAX_MEMORY_MB: usize = 100; // 100MB
const DEFAULT_MAX_ENTRIES: usize = 1000;

/// スマートキャッシュマネージャー
/// 
/// メモリ効率を重視した設計:
/// - LRUベースの自動削除
/// - ファイルハッシュによる変更検知
/// - 段階的ロード（必要時のみコンテンツ読み込み）
/// - ディスク永続化（将来実装）
pub struct CacheManager {
    /// メモリ内キャッシュ（パス -> エントリ）
    cache: HashMap<PathBuf, CacheEntry>,
    /// LRU順序管理（最近使用順）
    lru_order: Vec<PathBuf>,
    /// 最大メモリ使用量（バイト）
    max_memory_bytes: usize,
    /// 最大エントリ数
    max_entries: usize,
    /// 現在のメモリ使用量
    current_memory_bytes: usize,
}

impl CacheManager {
    /// 新しいキャッシュマネージャーを作成
    pub fn new() -> Self {
        Self::with_limits(DEFAULT_MAX_MEMORY_MB, DEFAULT_MAX_ENTRIES)
    }

    /// 制限値を指定してキャッシュマネージャーを作成
    pub fn with_limits(max_memory_mb: usize, max_entries: usize) -> Self {
        Self {
            cache: HashMap::new(),
            lru_order: Vec::new(),
            max_memory_bytes: max_memory_mb * 1024 * 1024,
            max_entries,
            current_memory_bytes: 0,
        }
    }

    /// ファイルのシンボル情報を取得（キャッシュ優先）
    pub fn get_symbols(&mut self, file_path: &Path) -> Result<Vec<CachedSymbol>> {
        // キャッシュから取得を試行
        if let Some(cached_symbols) = self.get_cached_symbols(file_path)? {
            return Ok(cached_symbols);
        }

        // キャッシュにない場合は新規解析
        self.analyze_and_cache_file(file_path)
    }

    /// ファイル内容を取得（キャッシュ優先）
    pub fn get_file_content(&mut self, file_path: &Path) -> Result<String> {
        // キャッシュから取得を試行
        let cached_content = self.cache.get(file_path)
            .and_then(|entry| entry.file_info.content.clone());
        
        if let Some(content) = cached_content {
            // LRU更新
            self.update_lru(file_path);
            return Ok(content);
        }

        // キャッシュにない場合はファイルから読み込み
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // キャッシュに保存（コンテンツ付き）
        self.cache_file_with_content(file_path, &content)?;

        Ok(content)
    }

    /// ファイルがキャッシュされているかチェック
    pub fn is_cached(&self, file_path: &Path) -> bool {
        self.cache.contains_key(file_path)
    }

    /// ファイルのキャッシュを無効化
    pub fn invalidate_file(&mut self, file_path: &Path) {
        if let Some(entry) = self.cache.remove(file_path) {
            self.current_memory_bytes = self.current_memory_bytes.saturating_sub(entry.memory_size);
            self.lru_order.retain(|p| p != file_path);
        }
    }

    /// 全キャッシュをクリア
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_order.clear();
        self.current_memory_bytes = 0;
    }

    /// キャッシュ統計情報を取得
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entry_count: self.cache.len(),
            memory_usage_mb: self.current_memory_bytes / (1024 * 1024),
            max_memory_mb: self.max_memory_bytes / (1024 * 1024),
            hit_ratio: 0.0, // TODO: ヒット率の実装
        }
    }

    /// キャッシュからシンボルを取得（変更検知付き）
    fn get_cached_symbols(&mut self, file_path: &Path) -> Result<Option<Vec<CachedSymbol>>> {
        if !self.cache.contains_key(file_path) {
            return Ok(None);
        }

        // ファイルの変更時刻をチェック
        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
        
        let current_modified = metadata.modified()
            .with_context(|| "Failed to get file modification time")?;

        let (modified_time, symbols) = {
            let entry = self.cache.get(file_path).unwrap();
            (entry.file_info.modified_time, entry.file_info.symbols.clone())
        };
        
        // 変更されている場合はキャッシュを無効化
        if current_modified > modified_time {
            self.invalidate_file(file_path);
            return Ok(None);
        }

        // LRU更新
        self.update_lru(file_path);

        Ok(Some(symbols))
    }

    /// ファイルを解析してキャッシュに保存
    fn analyze_and_cache_file(&mut self, file_path: &Path) -> Result<Vec<CachedSymbol>> {
        // ファイル読み込み
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        // メタデータ取得
        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
        
        let modified_time = metadata.modified()
            .with_context(|| "Failed to get file modification time")?;

        // Tree-sitterベースのシンボル解析
        let symbols = self.extract_symbols_with_tree_sitter(file_path)?;

        // キャッシュエントリ作成
        let file_info = CachedFileInfo {
            path: file_path.to_path_buf(),
            hash: self.calculate_file_hash(&content),
            modified_time,
            content: Some(content), // シンボル解析時はコンテンツも保存
            symbols: symbols.clone(),
            last_accessed: SystemTime::now(),
        };

        let memory_size = CacheEntry::estimate_memory_size(&file_info);
        let entry = CacheEntry { file_info, memory_size };

        // キャッシュに追加
        self.add_to_cache(file_path.to_path_buf(), entry);

        Ok(symbols)
    }

    /// ファイル内容付きでキャッシュに保存
    fn cache_file_with_content(&mut self, file_path: &Path, content: &str) -> Result<()> {
        let metadata = fs::metadata(file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
        
        let modified_time = metadata.modified()
            .with_context(|| "Failed to get file modification time")?;

        let file_info = CachedFileInfo {
            path: file_path.to_path_buf(),
            hash: self.calculate_file_hash(content),
            modified_time,
            content: Some(content.to_string()),
            symbols: Vec::new(), // シンボルは後で解析
            last_accessed: SystemTime::now(),
        };

        let memory_size = CacheEntry::estimate_memory_size(&file_info);
        let entry = CacheEntry { file_info, memory_size };

        self.add_to_cache(file_path.to_path_buf(), entry);

        Ok(())
    }

    /// キャッシュにエントリを追加（LRU管理付き）
    fn add_to_cache(&mut self, path: PathBuf, entry: CacheEntry) {
        // 既存エントリがある場合は削除
        if let Some(old_entry) = self.cache.remove(&path) {
            self.current_memory_bytes = self.current_memory_bytes.saturating_sub(old_entry.memory_size);
            self.lru_order.retain(|p| p != &path);
        }

        // メモリ使用量更新
        self.current_memory_bytes += entry.memory_size;

        // 新しいエントリを追加
        self.cache.insert(path.clone(), entry);
        self.lru_order.push(path);

        // 制限値チェック
        self.enforce_limits();
    }

    /// LRU順序を更新
    fn update_lru(&mut self, file_path: &Path) {
        // 既存位置を削除
        self.lru_order.retain(|p| p != file_path);
        // 最後に追加（最新使用）
        self.lru_order.push(file_path.to_path_buf());
    }

    /// キャッシュ制限を強制
    fn enforce_limits(&mut self) {
        // メモリ制限チェック
        while self.current_memory_bytes > self.max_memory_bytes && !self.lru_order.is_empty() {
            let oldest_path = self.lru_order.remove(0);
            if let Some(entry) = self.cache.remove(&oldest_path) {
                self.current_memory_bytes = self.current_memory_bytes.saturating_sub(entry.memory_size);
            }
        }

        // エントリ数制限チェック
        while self.cache.len() > self.max_entries && !self.lru_order.is_empty() {
            let oldest_path = self.lru_order.remove(0);
            if let Some(entry) = self.cache.remove(&oldest_path) {
                self.current_memory_bytes = self.current_memory_bytes.saturating_sub(entry.memory_size);
            }
        }
    }

    /// ファイルハッシュを計算
    fn calculate_file_hash(&self, content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Tree-sitterベースのシンボル抽出
    fn extract_symbols_with_tree_sitter(&self, file_path: &Path) -> Result<Vec<CachedSymbol>> {
        // Tree-sitterでシンボルを抽出
        let symbol_metadata = extract_symbols_from_file(file_path)?;
        
        // SymbolMetadata を CachedSymbol に変換
        let symbols = symbol_metadata
            .into_iter()
            .map(|meta| CachedSymbol {
                name: meta.name,
                symbol_type: meta.symbol_type,
                line: meta.line,
                column: meta.column,
            })
            .collect();
        
        Ok(symbols)
    }

}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

/// キャッシュ統計情報
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// エントリ数
    pub entry_count: usize,
    /// メモリ使用量（MB）
    pub memory_usage_mb: usize,
    /// 最大メモリ（MB）
    pub max_memory_mb: usize,
    /// ヒット率
    pub hit_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SymbolType;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_cache_manager_creation() {
        let cache = CacheManager::new();
        assert_eq!(cache.cache.len(), 0);
        assert_eq!(cache.current_memory_bytes, 0);
    }

    #[test]
    fn test_file_content_caching() -> Result<()> {
        let mut cache = CacheManager::new();
        
        // 一時ファイル作成
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "fn test_function() {{")?;
        writeln!(temp_file, "    println!(\"Hello\");")?;
        writeln!(temp_file, "}}")?;
        
        let file_path = temp_file.path();
        
        // 初回読み込み
        let content1 = cache.get_file_content(file_path)?;
        assert!(content1.contains("test_function"));
        assert!(cache.is_cached(file_path));
        
        // 2回目読み込み（キャッシュから）
        let content2 = cache.get_file_content(file_path)?;
        assert_eq!(content1, content2);
        
        Ok(())
    }

    #[test]
    fn test_symbol_extraction() -> Result<()> {
        let mut cache = CacheManager::new();
        
        // Rustファイルの一時作成
        let mut temp_file = NamedTempFile::with_suffix(".rs")?;
        writeln!(temp_file, "fn hello_world() {{")?;
        writeln!(temp_file, "    println!(\"Hello\");")?;
        writeln!(temp_file, "}}")?;
        writeln!(temp_file, "struct MyStruct {{")?;
        writeln!(temp_file, "    value: i32,")?;
        writeln!(temp_file, "}}")?;
        
        let file_path = temp_file.path();
        
        // シンボル抽出
        let symbols = cache.get_symbols(file_path)?;
        
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "hello_world");
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[1].name, "MyStruct");
        assert_eq!(symbols[1].symbol_type, SymbolType::Class);
        
        Ok(())
    }
}