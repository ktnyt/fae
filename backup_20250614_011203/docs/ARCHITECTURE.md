# fae - アーキテクチャ設計書

## システム全体構成

```
┌─────────────────────────────────────────┐
│ TUI (ratatui)                           │
├─────────────────────────────────────────┤
│ • 検索ボックス                           │
│ • 結果リスト (選択可能)                  │
│ • ステータス表示                         │
└─────────────────────────────────────────┘
              ↕️ (イベント)
┌─────────────────────────────────────────┐
│ App State Manager                       │
├─────────────────────────────────────────┤
│ • 検索クエリ管理                         │
│ • 結果リスト管理                         │
│ • 選択状態管理                           │
└─────────────────────────────────────────┘
              ↕️ (非同期)
┌─────────────────────────────────────────┐
│ Search Engine                           │
├─────────────────────────────────────────┤
│ • モード検出 (#, >, /)                  │
│ • マルチモード検索                       │
│ • 結果のフィルタリング・ソート           │
└─────────────────────────────────────────┘
              ↕️
┌─────────────────────────────────────────┐
│ Data Layer                              │
├─────────────────────────────────────────┤
│ • ファイルインデックス                   │
│ • スマートキャッシュ                     │
│ • ファイル監視                           │
│ • Git統合                               │
└─────────────────────────────────────────┘
```

## コンポーネント構成

```
fae/
├── src/
│   ├── main.rs                    # CLI エントリーポイント
│   ├── lib.rs                     # パブリック API・統合インターフェース
│   ├── types.rs                   # 基本データ型・検索結果・表示情報 ✅
│   ├── symbol_index.rs            # 軽量ファジー検索・メタデータストレージ ✅
│   ├── display.rs                 # 結果表示フォーマッター・色分け ✅
│   ├── cache_manager.rs           # スマートキャッシュ・LRU・SymbolIndex統合 ✅
│   ├── index_manager.rs           # ファイル発見・.gitignore・バイナリ検出 ✅
│   ├── search_coordinator.rs      # インデックス構築・並列処理・進捗報告 ✅
│   ├── tree_sitter.rs             # Tree-sitter統合エントリーポイント ✅
│   ├── languages/                 # 言語別Tree-sitterモジュール ✅
│   │   ├── mod.rs                 # LanguageExtractor統一インターフェース ✅
│   │   ├── common.rs              # 統合クエリ・共通ヘルパー ✅
│   │   ├── typescript.rs          # TypeScript シンボル抽出 ✅
│   │   ├── javascript.rs          # JavaScript シンボル抽出 ✅
│   │   ├── python.rs              # Python シンボル抽出 ✅
│   │   └── rust_lang.rs           # Rust シンボル抽出 ✅
│   ├── app.rs                     # TUI Application (状態管理・描画) 🔄
│   └── searchers/                 # マルチモード検索エンジン 🔄
│       ├── mod.rs                 # 検索エンジン統合
│       ├── content_search.rs      # コンテンツ検索（grep風）
│       ├── file_search.rs         # ファイル名検索
│       └── regex_search.rs        # 正規表現検索
└── tests/
    ├── debug_tree_sitter.rs       # Tree-sitter統合デバッグテスト ✅
    ├── tree_sitter_integration_test.rs # Tree-sitter統合テスト ✅
    └── fixtures/                  # テスト用サンプルファイル
```

**実装状況:**
- ✅ **完了**: コア機能実装済み（31テスト全通過）
- 🔄 **進行中**: 次フェーズ実装対象
- ⏳ **未着手**: 将来実装予定

## データフロー

### 起動時
1. index_manager: ファイル発見 + Git状態取得
2. cache_manager: 既存キャッシュ読み込み
3. TUI: デフォルト表示 (git変更ファイル)

### 検索時
1. app: ユーザー入力受信
2. search_coordinator: モード判定 + 検索リクエスト
3. searchers: 並列検索実行
4. cache_manager: 必要に応じてシンボル解析
5. search_coordinator: 結果統合
6. app: リアルタイム表示更新

### ファイル変更時
1. index_manager: 変更検知
2. cache_manager: 該当キャッシュ無効化
3. search_coordinator: アクティブ検索の再実行

## シンボルインデックス設計

### アーキテクチャ概要

```
┌─────────────────────────────────────────┐
│ データソース1: 軽量シンボルインデックス    │
├─────────────────────────────────────────┤
│ • ユニークなシンボル名のみ（重複排除済み） │
│ • メモリ内配列でファジー検索最適化        │
└─────────────────────────────────────────┘
              ↓ fuzzy search
┌─────────────────────────────────────────┐
│ ファジー検索エンジン                      │
├─────────────────────────────────────────┤
│ • fuzzy-matcher による高速マッチング     │
│ • スコアリング・ソート                   │
└─────────────────────────────────────────┘
              ↓ ヒットしたシンボル
┌─────────────────────────────────────────┐
│ データソース2: ソート済みメタデータ       │
├─────────────────────────────────────────┤
│ • .fae/symbols.bin (バイナリ形式)       │
│ • アルファベット順ソート                 │
│ • O(log n) バイナリサーチ               │
└─────────────────────────────────────────┘
```

### 設計原則

1. **分離した責務**: ファジー検索とメタデータ管理を完全分離
2. **メモリ効率**: シンボル名のみメモリ内保持
3. **検索速度**: 純粋な文字列マッチングで最高速度
4. **重複排除**: 同名シンボルはインデックスで排除、メタデータで複数保持

### データフロー

```
1. Query: "handle" 
   ↓
2. SymbolIndex.fuzzy_search() → [SearchHit{index: 42, score: 0.8}, ...]
   ↓
3. symbol_names[42] = "handleClick"
   ↓
4. MetadataStorage.find_metadata("handleClick") → [SymbolMetadata, ...]
   ↓
5. 表示用フォーマット
```

### ファイル構造

```
.fae/
├── symbols.bin          # ソート済みメタデータ（バイナリ）
└── cache_info.json      # キャッシュメタ情報
```

## 核となるデータ構造

### シンボルインデックス関連

```rust
// メモリ内シンボルインデックス（ファジー検索用）
pub struct SymbolIndex {
    symbol_names: Vec<String>,    // ユニークなシンボル名（重複排除済み）
    matcher: SkimMatcherV2,       // ファジー検索エンジン
}

// ファジー検索結果
pub struct SearchHit {
    pub index: usize,             // symbol_names のインデックス
    pub score: i64,               // マッチスコア
    pub symbol_name: String,      // マッチしたシンボル名
}

// ディスク保存用シンボルメタデータ
pub struct SymbolMetadata {
    pub name: String,             // シンボル名
    pub file_path: PathBuf,       // ファイルパス（相対パス）
    pub line: u32,                // 行番号（1ベース）
    pub column: u32,              // 列番号（1ベース）
    pub symbol_type: SymbolType,  // シンボル種別
}

// メタデータストレージ（ソート済みバイナリファイル）
pub struct MetadataStorage {
    cache_dir: PathBuf,           // .fae ディレクトリパス
    metadata_file: PathBuf,       // symbols.bin ファイルパス
    metadata_cache: Option<Vec<SymbolMetadata>>, // メモリキャッシュ
}
```

### 検索・表示関連

```rust
// 検索モード
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMode {
    Content,     // デフォルト
    Symbol,      // #prefix
    File,        // >prefix  
    Regex,       // /prefix
}

// 統一検索結果
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file_path: PathBuf,       // ファイルパス（絶対パス - 表示時に相対パス変換）
    pub line: u32,                // 行番号（1ベース）
    pub column: u32,              // 列番号（1ベース）
    pub display_info: DisplayInfo,// 表示用のコンテキスト情報
    pub score: f64,               // 検索スコア（ソート用）
}

// 表示用情報（検索モード別）
#[derive(Debug, Clone)]
pub enum DisplayInfo {
    Content { 
        line_content: String, 
        match_start: usize, 
        match_end: usize 
    },
    Symbol { 
        name: String, 
        symbol_type: SymbolType 
    },
    File { 
        file_name: String 
    },
    Regex { 
        line_content: String, 
        matched_text: String, 
        match_start: usize, 
        match_end: usize 
    },
}

// シンボル種別
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
    Function,
    Class,
    Variable,
    Constant,
    Interface,
    Type,
}
```

## パフォーマンス設計

### 目標値
- **検索応答時間**: 1000ファイル未満で500ms以内
- **メモリ使用量**: 通常のプロジェクトで100MB以下
- **CPU効率**: 利用可能なCPUコアを効率的に使用

### 最適化戦略
- **軽量シンボルインデックス**: メモリ内保持はシンボル名のみ
- **O(log n)検索**: ソート済みバイナリファイルでのバイナリサーチ
- **並列処理**: ファイル解析・検索での並列実行
- **スマートキャッシュ**: LRUベースの効率的なキャッシュ戦略

---

この設計により、大規模プロジェクトでも高速で直感的なコード検索体験を提供します。