use crate::types::*;
use anyhow::{anyhow, Result};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

/// チャンクID: ファイルをグループ化する単位
pub type ChunkId = usize;

/// シンボルチャンクの位置情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkLocation {
    pub chunk_id: ChunkId,
    pub offset: usize,
    pub count: usize,
}

/// ファイルメタデータ（軽量、常時メモリ保持）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub hash: String,
    pub last_modified: String,
    pub size: u64,
    pub symbol_locations: Vec<ChunkLocation>,
}

/// キャッシュインデックス（メタデータのみ）
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheIndex {
    pub version: String,
    pub cache_created: String,
    pub sfs_version: String,
    pub files: HashMap<String, FileMetadata>,
    pub chunk_info: HashMap<ChunkId, ChunkInfo>,
}

/// チャンク情報
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub file_path: String,
    pub symbol_count: usize,
    pub compressed_size: usize,
}

/// メモリ効率的キャッシュマネージャー
pub struct MemoryEfficientCacheManager {
    /// 軽量インデックス（常時メモリ保持）
    index: CacheIndex,

    /// シンボルチャンクのLRUキャッシュ
    symbol_cache: LruCache<ChunkId, Vec<CodeSymbol>>,

    /// キャッシュディレクトリパス
    cache_dir: PathBuf,

    /// 最大メモリ使用量（バイト）
    max_memory_usage: usize,

    /// 現在のメモリ使用量
    current_memory_usage: usize,

    /// チャンクあたりの最大シンボル数
    symbols_per_chunk: usize,
}

impl MemoryEfficientCacheManager {
    /// 新しいキャッシュマネージャーを作成
    pub fn new(cache_dir: PathBuf, max_memory_mb: usize) -> Self {
        let max_memory_usage = max_memory_mb * 1024 * 1024; // MB to bytes
        let max_chunks = max_memory_usage / (1000 * std::mem::size_of::<CodeSymbol>());
        let cache_capacity = NonZeroUsize::new(max_chunks.max(10)).unwrap();

        Self {
            index: CacheIndex {
                version: "2.0".to_string(),
                cache_created: chrono::Utc::now().to_rfc3339(),
                sfs_version: env!("CARGO_PKG_VERSION").to_string(),
                files: HashMap::new(),
                chunk_info: HashMap::new(),
            },
            symbol_cache: LruCache::new(cache_capacity),
            cache_dir,
            max_memory_usage,
            current_memory_usage: 0,
            symbols_per_chunk: 1000, // 1チャンクあたり1000シンボル
        }
    }

    /// キャッシュインデックスをロード
    pub fn load_index(&mut self, directory: &Path) -> Result<CacheStats> {
        let index_path = directory.join(".sfscache_v2.gz");

        if !index_path.exists() {
            return Ok(CacheStats {
                total_files: 0,
                total_symbols: 0,
                cache_created: "N/A".to_string(),
                sfs_version: "N/A".to_string(),
            });
        }

        let cache_content = {
            use flate2::read::GzDecoder;
            use std::io::Read;
            let file = std::fs::File::open(&index_path)?;
            let mut decoder = GzDecoder::new(file);
            let mut decompressed = String::new();
            decoder.read_to_string(&mut decompressed)?;
            decompressed
        };

        self.index = serde_json::from_str(&cache_content)?;
        self.cache_dir = directory.to_path_buf();

        let stats = self.get_stats();
        Ok(stats)
    }

    /// 特定ファイルのシンボルを取得（オンデマンド）
    pub fn get_file_symbols(&mut self, file_path: &str) -> Result<Vec<CodeSymbol>> {
        // 借用チェッカー対応: symbol_locationsを先に取得
        let symbol_locations = self
            .index
            .files
            .get(file_path)
            .ok_or_else(|| anyhow!("File not found in cache: {}", file_path))?
            .symbol_locations
            .clone();

        let mut all_symbols = Vec::new();

        for location in &symbol_locations {
            let symbols = self.get_chunk_symbols(location.chunk_id)?;
            let file_symbols: Vec<CodeSymbol> = symbols
                .into_iter()
                .skip(location.offset)
                .take(location.count)
                .collect();
            all_symbols.extend(file_symbols);
        }

        Ok(all_symbols)
    }

    /// チャンクからシンボルを取得（LRUキャッシュ使用）
    fn get_chunk_symbols(&mut self, chunk_id: ChunkId) -> Result<Vec<CodeSymbol>> {
        // LRUキャッシュから確認
        if let Some(symbols) = self.symbol_cache.get(&chunk_id) {
            return Ok(symbols.clone());
        }

        // ディスクからロード
        let chunk_path = self.cache_dir.join(format!("chunk_{:06}.bin.gz", chunk_id));
        let symbols = self.load_chunk_from_disk(&chunk_path)?;

        // メモリ使用量を更新
        let symbols_memory = symbols.len() * std::mem::size_of::<CodeSymbol>();

        // メモリ制限チェック
        while self.current_memory_usage + symbols_memory > self.max_memory_usage {
            if let Some((_, evicted_symbols)) = self.symbol_cache.pop_lru() {
                self.current_memory_usage -=
                    evicted_symbols.len() * std::mem::size_of::<CodeSymbol>();
            } else {
                break;
            }
        }

        // キャッシュに追加
        self.current_memory_usage += symbols_memory;
        self.symbol_cache.put(chunk_id, symbols.clone());

        Ok(symbols)
    }

    /// チャンクをディスクからロード
    fn load_chunk_from_disk(&self, chunk_path: &Path) -> Result<Vec<CodeSymbol>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let file = std::fs::File::open(chunk_path)?;
        let mut decoder = GzDecoder::new(file);
        let mut compressed_data = Vec::new();
        decoder.read_to_end(&mut compressed_data)?;

        let symbols: Vec<CodeSymbol> = serde_json::from_slice(&compressed_data)?;
        Ok(symbols)
    }

    /// ファイルのキャッシュエントリを更新
    pub fn update_file_cache(
        &mut self,
        file_path: &str,
        hash: String,
        symbols: Vec<CodeSymbol>,
    ) -> Result<()> {
        // シンボルをチャンク分割
        let chunks = self.split_symbols_into_chunks(symbols);
        let mut symbol_locations = Vec::new();

        for (chunk_symbols, chunk_id) in chunks {
            // チャンクをディスクに保存
            self.save_chunk_to_disk(chunk_id, &chunk_symbols)?;

            // 位置情報を記録
            symbol_locations.push(ChunkLocation {
                chunk_id,
                offset: 0,
                count: chunk_symbols.len(),
            });

            // チャンク情報を更新
            self.index.chunk_info.insert(
                chunk_id,
                ChunkInfo {
                    file_path: file_path.to_string(),
                    symbol_count: chunk_symbols.len(),
                    compressed_size: 0, // TODO: 実際のサイズを計算
                },
            );
        }

        // ファイルメタデータを更新
        let file_meta = FileMetadata {
            hash,
            last_modified: chrono::Utc::now().to_rfc3339(),
            size: 0, // TODO: ファイルサイズ
            symbol_locations,
        };

        self.index.files.insert(file_path.to_string(), file_meta);
        Ok(())
    }

    /// シンボルをチャンクに分割
    fn split_symbols_into_chunks(
        &self,
        symbols: Vec<CodeSymbol>,
    ) -> Vec<(Vec<CodeSymbol>, ChunkId)> {
        let mut chunks = Vec::new();
        let mut chunk_id = self.get_next_chunk_id();

        for chunk_symbols in symbols.chunks(self.symbols_per_chunk) {
            chunks.push((chunk_symbols.to_vec(), chunk_id));
            chunk_id += 1;
        }

        chunks
    }

    /// 次のチャンクIDを取得
    fn get_next_chunk_id(&self) -> ChunkId {
        self.index.chunk_info.keys().max().unwrap_or(&0) + 1
    }

    /// チャンクをディスクに保存
    fn save_chunk_to_disk(&self, chunk_id: ChunkId, symbols: &[CodeSymbol]) -> Result<()> {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;

        let chunk_path = self.cache_dir.join(format!("chunk_{:06}.bin.gz", chunk_id));

        // JSONシリアライズ + gzip圧縮
        let json_data = serde_json::to_vec(symbols)?;
        let file = std::fs::File::create(chunk_path)?;
        let mut encoder = GzEncoder::new(file, Compression::best());
        encoder.write_all(&json_data)?;
        encoder.finish()?;

        Ok(())
    }

    /// インデックスをディスクに保存
    pub fn save_index(&self, directory: &Path) -> Result<()> {
        use flate2::{write::GzEncoder, Compression};
        use std::io::Write;

        let index_path = directory.join(".sfscache_v2.gz");
        let json_data = serde_json::to_string(&self.index)?;

        let file = std::fs::File::create(index_path)?;
        let mut encoder = GzEncoder::new(file, Compression::best());
        encoder.write_all(json_data.as_bytes())?;
        encoder.finish()?;

        Ok(())
    }

    /// キャッシュ統計を取得
    pub fn get_stats(&self) -> CacheStats {
        let total_files = self.index.files.len();
        let total_symbols: usize = self.index.chunk_info.values().map(|c| c.symbol_count).sum();

        CacheStats {
            total_files,
            total_symbols,
            cache_created: self.index.cache_created.clone(),
            sfs_version: self.index.sfs_version.clone(),
        }
    }

    /// 現在のメモリ使用量（MB）
    pub fn memory_usage_mb(&self) -> f64 {
        self.current_memory_usage as f64 / 1024.0 / 1024.0
    }

    /// キャッシュヒット率
    pub fn cache_hit_rate(&self) -> f64 {
        // TODO: ヒット率の統計を実装
        0.0
    }
}
