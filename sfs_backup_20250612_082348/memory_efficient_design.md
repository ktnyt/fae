# メモリ効率的キャッシュシステム設計

## 現在の問題

- scoutyプロジェクト: 84,136シンボルで45.58MB (568 bytes/symbol)
- 大規模プロジェクト予想: 500K シンボルで ~280MB
- 全シンボルを一度にメモリロードしている

## 改善戦略

### 1. 階層キャッシュアーキテクチャ

```
.sfscache.gz (メタデータ + インデックス)
├── file_index: ファイル一覧とハッシュ
├── symbol_index: シンボル名→ファイル位置マッピング  
└── symbol_chunks/: 実際のシンボルデータを分割保存

.sfsdata/ (オンデマンドデータディレクトリ)
├── chunk_001.bin (1000シンボル/チャンク)
├── chunk_002.bin
└── ...
```

### 2. メモリ管理戦略

```rust
pub struct MemoryEfficientIndexer {
    // 常時メモリ保持（軽量）
    file_index: HashMap<PathBuf, FileMetadata>,
    symbol_index: HashMap<String, ChunkLocation>,
    
    // LRUキャッシュ（必要時のみ）
    symbol_cache: LruCache<ChunkId, Vec<CodeSymbol>>,
    max_cache_size: usize, // 例: 50MB
}

struct ChunkLocation {
    chunk_id: ChunkId,
    offset: usize,
    count: usize,
}
```

### 3. 検索フロー最適化

1. **即座検索**: symbol_indexで候補特定 (メモリ内)
2. **遅延ロード**: 必要なチャンクのみディスクから読み込み
3. **キャッシュ**: 最近使用したチャンクをメモリ保持

## 実装オプション

### Option A: 改良JSON + チャンク分割
- 既存コードとの互換性◎
- 実装コスト: 低
- メモリ削減: 80-90%

### Option B: SQLite統合  
- ランダムアクセス性能◎
- 実装コスト: 中
- メモリ削減: 90-95%

### Option C: MessagePack + memmap
- デシリアライゼーション性能◎
- 実装コスト: 中
- メモリ削減: 85-92%

## 推奨実装順序

1. **Phase 1**: チャンク分割 + LRUキャッシュ (Option A)
2. **Phase 2**: パフォーマンス評価
3. **Phase 3**: 必要に応じてSQLite移行 (Option B)