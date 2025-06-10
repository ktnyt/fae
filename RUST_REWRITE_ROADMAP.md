# Rust Rewrite Roadmap for SFS (Symbol Fuzzy Search)

## 概要

現在のTypeScript/Node.js実装をRustで書き直すためのロードマップ。パフォーマンス向上、バイナリポータビリティ、TUI安定性を目的とする。

## 目標

- **パフォーマンス**: TypeScript版の10x以上の高速化
- **ポータビリティ**: 単一バイナリでクロスプラットフォーム対応
- **安定性**: blessedの制約から解放され、安定したTUI
- **機能保持**: 現在の全機能を維持・向上

## Phase 1: プロジェクト基盤構築とテスト移植

### 1.1 プロジェクト初期化
- [ ] `cargo new sfs-rs` でRustプロジェクト作成
- [ ] 基本的な`Cargo.toml`設定
- [ ] ディレクトリ構造設計

### 1.2 テスト移植戦略
**重要**: 実装前に既存テストを全てRustに移植し、それをパスするように開発を進める

#### 既存テストの分析・移植順序
1. **型定義テスト** (`types.test.ts`)
2. **ファジー検索テスト** (`fuzzy-searcher.test.ts`) 
3. **Tree-sitterインデクサーテスト** (`tree-sitter-indexer.test.ts`)
4. **pecoインターフェーステスト** (`peco-interface.test.ts`)
5. **統合テスト** (`indexer-searcher.test.ts`)
6. **E2Eテスト** (`search-modes.test.ts`, `real-world.test.ts`)
7. **CLIテスト** (`cli.test.ts`)

```
sfs-rs/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli.rs          # CLIパーサー
│   ├── indexer.rs      # Tree-sitterインデクサー
│   ├── searcher.rs     # ファジー検索エンジン
│   ├── tui/
│   │   ├── mod.rs
│   │   ├── app.rs      # アプリケーション状態
│   │   ├── ui.rs       # UI描画
│   │   └── events.rs   # イベントハンドリング
│   └── types.rs        # 型定義
├── tests/
└── benches/           # パフォーマンステスト
```

### 1.3 既存テスト分析とRust移植計画

#### TypeScript テストスイートの現状分析
```bash
# 現在のテスト数を確認
npm test -- --reporter=verbose
```

| テストファイル | テスト数 | 移植優先度 | Rust移植先 |
|---------------|---------|------------|------------|
| `types.test.ts` | 8 | 最高 | `tests/types_test.rs` |
| `fuzzy-searcher.test.ts` | 15 | 最高 | `tests/searcher_test.rs` |
| `tree-sitter-indexer.test.ts` | 10 | 最高 | `tests/indexer_test.rs` |
| `peco-interface.test.ts` | 23 | 高 | `tests/tui_test.rs` |
| `indexer-searcher.test.ts` | 8 | 高 | `tests/integration_test.rs` |
| `search-modes.test.ts` | 13 | 中 | `tests/e2e_search_modes_test.rs` |
| `real-world.test.ts` | 13 | 中 | `tests/e2e_real_world_test.rs` |
| `cli.test.ts` | 17 | 低 | `tests/cli_test.rs` |

#### テスト移植のアプローチ

1. **Phase 1A: コアロジックテスト移植**
   - [ ] `types.test.ts` → `tests/types_test.rs`
   - [ ] `fuzzy-searcher.test.ts` → `tests/searcher_test.rs` 
   - [ ] `tree-sitter-indexer.test.ts` → `tests/indexer_test.rs`

2. **Phase 1B: インターフェーステスト移植**
   - [ ] `peco-interface.test.ts` → `tests/tui_test.rs` (モック中心)
   - [ ] `indexer-searcher.test.ts` → `tests/integration_test.rs`

3. **Phase 1C: E2Eテスト移植**
   - [ ] `search-modes.test.ts` → `tests/e2e_search_modes_test.rs`
   - [ ] `real-world.test.ts` → `tests/e2e_real_world_test.rs`
   - [ ] `cli.test.ts` → `tests/cli_test.rs`

### 1.4 Rust依存関係セットアップ

```toml
[dependencies]
# TUI
ratatui = "0.24"
crossterm = "0.27"

# Tree-sitter
tree-sitter = "0.20"
tree-sitter-typescript = "0.20"
tree-sitter-javascript = "0.20"
tree-sitter-python = "0.20"

# 検索・ファイル操作
fuzzy-matcher = "0.3"
globwalk = "0.8"
ignore = "0.4"        # .gitignore対応

# クリップボード
arboard = "3.2"

# CLI・設定
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 非同期・並列処理
tokio = { version = "1.0", features = ["full"] }
rayon = "1.8"

# エラーハンドリング
anyhow = "1.0"
thiserror = "1.0"

[dev-dependencies]
criterion = "0.5"     # ベンチマーク
tempfile = "3.8"      # テスト用
mockall = "0.12"      # モック生成
serial_test = "3.0"   # テスト実行順序制御
```

## Phase 2: TDD による実装 (Test-Driven Development)

### 2.1 型定義・データ構造 (テストファースト)
**目標**: `tests/types_test.rs` の全テストをパス

- [ ] **Step 1**: `types.test.ts` の8テストをRustに移植
- [ ] **Step 2**: 空の型定義で失敗を確認
- [ ] **Step 3**: `CodeSymbol`構造体の実装
- [ ] **Step 4**: `SearchResult`構造体の実装  
- [ ] **Step 5**: `SymbolType`enum定義
- [ ] **Step 6**: エラー型定義
- [ ] **Step 7**: 全テストのパス確認

```rust
// tests/types_test.rs 移植例
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_code_symbol_creation() {
        let symbol = CodeSymbol {
            name: "test_function".to_string(),
            symbol_type: SymbolType::Function,
            file: PathBuf::from("/src/test.ts"),
            line: 10,
            column: 5,
            context: Some("function test_function() {".to_string()),
        };
        
        assert_eq!(symbol.name, "test_function");
        assert_eq!(symbol.symbol_type, SymbolType::Function);
        assert_eq!(symbol.file, PathBuf::from("/src/test.ts"));
        assert_eq!(symbol.line, 10);
        assert_eq!(symbol.column, 5);
    }
    
    // 残り7テストを移植...
}
```

### 2.2 ファジー検索エンジン (テストファースト)
**目標**: `tests/searcher_test.rs` の全15テストをパス

- [ ] **Step 1**: `fuzzy-searcher.test.ts` の15テストをRustに移植
- [ ] **Step 2**: 基本インターフェースの定義
- [ ] **Step 3**: 基本ファジー検索実装
- [ ] **Step 4**: 検索オプション対応
- [ ] **Step 5**: スコアリングアルゴリズム
- [ ] **Step 6**: フィルタリング・ソート機能
- [ ] **Step 7**: 検索モード対応（Symbol, File, Regex）
- [ ] **Step 8**: 全テストのパス確認

### 2.3 Tree-sitterインデクサー (テストファースト)  
**目標**: `tests/indexer_test.rs` の全10テストをパス

- [ ] **Step 1**: `tree-sitter-indexer.test.ts` の10テストをRustに移植
- [ ] **Step 2**: 基本インターフェースの定義
- [ ] **Step 3**: 言語パーサー管理
- [ ] **Step 4**: ファイル解析ロジック
- [ ] **Step 5**: シンボル抽出機能
- [ ] **Step 6**: エラーハンドリング
- [ ] **Step 7**: パフォーマンス最適化
- [ ] **Step 8**: 全テストのパス確認

## Phase 3: TUIインターフェース実装

### 3.1 基本TUI構造
- [ ] ratatui基本セットアップ
- [ ] アプリケーション状態管理
- [ ] イベントループ実装
- [ ] 画面レイアウト設計

### 3.2 検索インターフェース
- [ ] 検索ボックス実装
- [ ] リアルタイム検索
- [ ] 結果リスト表示
- [ ] ナビゲーション（↑↓キー）

### 3.3 pecoライクインターフェース
- [ ] 検索モード切り替え（#, >, /プレフィックス）
- [ ] ステータスバー表示
- [ ] ヘルプダイアログ
- [ ] クリップボード連携

### 3.4 キーボードショートカット
- [ ] 基本ナビゲーション
- [ ] Enterキーでクリップボードコピー
- [ ] 検索ボックスクリア
- [ ] アプリケーション継続動作

## Phase 4: 高度な機能

### 4.1 パフォーマンス最適化
- [ ] インクリメンタルインデックス
- [ ] ファイル変更監視
- [ ] メモリ効率改善
- [ ] 並列処理最適化

### 4.2 設定・拡張性
- [ ] 設定ファイル対応
- [ ] カスタムパターン
- [ ] プラグインシステム設計
- [ ] 言語サポート拡張

### 4.3 テスト・品質保証
- [ ] ユニットテスト
- [ ] 統合テスト
- [ ] ベンチマークテスト
- [ ] メモリリークテスト

## Phase 5: ビルド・デプロイ

### 5.1 クロスコンパイル
- [ ] Linux (x86_64, arm64)
- [ ] macOS (x86_64, arm64)
- [ ] Windows (x86_64)
- [ ] 最適化ビルド設定

### 5.2 配布・CI/CD
- [ ] GitHub Actions設定
- [ ] リリース自動化
- [ ] バイナリサイズ最適化
- [ ] パッケージマネージャー対応

## 現在のTypeScript版機能対応表

| 機能 | TypeScript | Rust実装予定 | 優先度 |
|------|------------|--------------|--------|
| Tree-sitterパース | ✅ | Phase 2.2 | 高 |
| ファジー検索 | ✅ | Phase 2.3 | 高 |
| 検索モード切り替え | ✅ | Phase 3.3 | 高 |
| TUIインターフェース | ✅ (blessed) | Phase 3.1-3.2 | 高 |
| クリップボードコピー | ✅ | Phase 3.3 | 高 |
| 対話的モード | ✅ | Phase 4.2 | 中 |
| 設定ファイル | ❌ | Phase 4.2 | 中 |
| ファイル監視 | ❌ | Phase 4.1 | 低 |

## マイルストーン (TDD重視)

### M1: コアロジック完成 - 3週間
- **Phase 1A完了**: 基本テスト移植と型定義
- **Phase 2.1-2.3完了**: 全コアロジックのテスト通過
- **達成基準**: 107テスト中、約40テスト (コアロジック) がパス

### M2: インターフェース統合 - 5週間  
- **Phase 1B完了**: UI・統合テスト移植
- **Phase 3.1-3.3完了**: TUIインターフェース実装
- **達成基準**: 107テスト中、約80テストがパス

### M3: 完全移行 - 7週間
- **Phase 1C完了**: E2E・CLIテスト移植
- **全Phase完了**: TypeScript版の全機能を上回る
- **達成基準**: 107テスト全てがパス + Rust追加テスト

## TDD開発サイクル

各機能について以下のサイクルを実行：

```
1. 🔴 Red: TypeScriptテストをRustに移植 (失敗)
   ↓
2. 🟢 Green: 最小限の実装でテストをパス  
   ↓
3. 🔵 Refactor: コードを改善・最適化
   ↓
4. 🔄 Repeat: 次のテストへ
```

### 週次テスト進捗目標

| 週 | 目標テスト数 | 累計 | 主要機能 |
|----|-------------|------|----------|
| W1 | 8 | 8/107 | 型定義 |
| W2 | 15 | 23/107 | ファジー検索 |  
| W3 | 10 | 33/107 | Tree-sitterパース |
| W4 | 23 | 56/107 | TUIインターフェース |
| W5 | 8 | 64/107 | 統合テスト |
| W6 | 26 | 90/107 | E2Eテスト |
| W7 | 17 | 107/107 | CLIテスト |

## パフォーマンス目標

| メトリック | TypeScript版 | Rust目標 |
|------------|--------------|----------|
| インデックス速度 | ~1000 files/sec | ~10000 files/sec |
| 検索レスポンス | ~50ms | ~5ms |
| メモリ使用量 | ~100MB | ~20MB |
| バイナリサイズ | ~50MB (Node.js) | ~5MB |
| 起動時間 | ~500ms | ~50ms |

## 参考実装

- **skim**: Rust製fzfクローン - https://github.com/lotabout/skim
- **fd**: Rust製findクローン - https://github.com/sharkdp/fd
- **ripgrep**: Rust製grepクローン - https://github.com/BurntSushi/ripgrep
- **ratatui examples**: https://github.com/ratatui-org/ratatui/tree/master/examples

## 次のステップ (TDD開始)

### 即座に実行すべきアクション

1. **既存テスト数の正確な把握**
   ```bash
   cd /Users/nano/github.com/ktnyt/sfs
   npm test -- --reporter=json > test-results.json
   cat test-results.json | jq '.numTotalTests'
   ```

2. **Phase 1.1: Rustプロジェクト作成**
   ```bash
   cargo new sfs-rs --bin
   cd sfs-rs
   ```

3. **Phase 1A: 最初のテスト移植開始**
   - `types.test.ts` を読んで理解
   - `tests/types_test.rs` に移植
   - 型定義なしでテスト失敗を確認
   - 段階的に型を実装してテストをパス

### 開発の進行管理

- **日次**: 該当するテストが何個パスしたかを記録
- **週次**: マイルストーン進捗のレビュー  
- **問題発生時**: TypeScript版に立ち戻って動作確認

### 品質保証

- **リグレッション防止**: 全テストが常にパスする状態を維持
- **パフォーマンス計測**: 各フェーズ完了時にベンチマーク実行
- **機能比較**: TypeScript版と同一入力での出力比較テスト

---

*このロードマップは進捗に応じて随時更新されます。*