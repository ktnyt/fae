# fae - Fast And Elegant code search

妖精のように軽やかで魔法のようにコードを発見するリアルタイム検索ツール

## 概要

**fae** は、コードベースを様々な切り口からリアルタイムで検索できるTUIベースのインタラクティブツールです。大規模プロジェクトでも高速動作し、直感的な操作でコードの発見を支援します。

> **🚧 開発状況**: 現在Phase 4まで完了（コアエンジン実装済み）。TUI実装はPhase 7で予定。

## 主な機能

### マルチモード検索
- **コンテンツ検索** (デフォルト) - ファイル内容のテキスト検索
- **シンボル検索** (`#prefix`) - 関数・クラス・変数名での検索
- **ファイル検索** (`>prefix`) - ファイル名・パスでの検索
- **正規表現検索** (`/prefix`) - 高度なパターンマッチング

### 主要特徴
- **高速シンボル検索** ✅ - Tree-sitter + ファジー検索（4言語対応）
- **並列インデックス構築** ✅ - rayon並列処理による高速インデックシング
- **スマートファイル発見** ✅ - .gitignore対応・バイナリ検出・サイズ制限
- **メモリ効率設計** ✅ - 軽量インデックス + 分離メタデータストレージ
- **包括的テスト** ✅ - 31テスト全通過（ユニット・統合・デバッグ）
- **リアルタイム検索** 🔄 - 入力に応じた即座の結果更新（TUI実装中）
- **直感的操作** 🔄 - ファジー検索とキーボードナビゲーション（TUI実装中）

## インストール

```bash
# Rust環境での開発版インストール
git clone https://github.com/ktnyt/fae.git
cd fae
cargo build --release
cargo install --path .
```

## 使い方

### 現在利用可能な機能（開発版）

```bash
# プロジェクトのビルドとテスト実行
cargo build --release
cargo test

# ライブラリAPIとしての利用（Rust）
use fae::{SearchCoordinator, IndexManager};

// インデックス構築とシンボル検索
let mut coordinator = SearchCoordinator::new(project_root)?;
let result = coordinator.build_index()?;
let hits = coordinator.search_symbols("handleClick", 10);
```

### 将来予定のTUI操作

1. **検索入力**: 検索クエリを入力
2. **モード切替**: プレフィックスで自動切替
   - `#function` → シンボル検索
   - `>main.rs` → ファイル検索  
   - `/regex.*` → 正規表現検索
3. **ナビゲーション**: 
   - `↑/↓` または `Ctrl+P/N` で選択
   - `Enter` で結果をクリップボードにコピー
   - `Esc/Ctrl+C` で終了

### 検索例

```bash
# シンボル検索: 関数名 "handle" を含むシンボル
#handle

# ファイル検索: "component" を含むファイル
>component

# 正規表現検索: import文の検索
/^import.*from

# コンテンツ検索: ファイル内容から "error" を検索
error
```

## 実装状況

### ✅ 完了機能（Phase 1-4）

- **シンボルインデックス**: ファジー検索・メタデータストレージ・重複排除
- **Tree-sitter統合**: 4言語対応・統合クエリ最適化・並列処理
- **ファイル発見エンジン**: .gitignore対応・バイナリ検出・サイズ制限
- **インデックス構築**: 並列シンボル抽出・プログレッシブ構築・進捗報告

### 🔄 次フェーズ（Phase 5-6）

- **マルチモード検索**: コンテンツ・シンボル・ファイル・正規表現検索
- **Git統合**: 変更ファイル検出・ブランチ情報連携

### 対応言語

- **TypeScript** (`.ts`, `.tsx`) ✅ - Interface, Class, Function, Method, Constant対応
- **JavaScript** (`.js`, `.jsx`) ✅ - Class, Function, Method, ArrowFunction, Constant対応  
- **Python** (`.py`) ✅ - Class, Function, Assignment対応
- **Rust** (`.rs`) ✅ - Struct, Enum, Function, Const対応

## 設計哲学

- **リアルタイム・ファースト**: 入力に応じた即座の結果更新
- **メモリ効率**: 巨大プロジェクトでもスマートなキャッシュ戦略
- **非同期設計**: UIブロッキングなしの快適な操作性
- **テスト駆動**: 全機能に対して網羅的なテスト

## 除外対象

- バイナリファイル
- `.gitignore` に記載されたファイル
- 1MB を超える大きなファイル
- 典型的な除外ディレクトリ (`node_modules/`, `target/`, `.git/` 等)

## 開発・貢献

詳細な技術仕様や開発情報については以下のドキュメントを参照してください：

- [ARCHITECTURE.md](./ARCHITECTURE.md) - システム設計・データ構造
- [DEVELOPMENT.md](./DEVELOPMENT.md) - 開発フェーズ・テスト戦略  
- [DESIGN.md](./DESIGN.md) - 概要設計書

## ライセンス

[MIT License](./LICENSE)

---

*妖精のように軽やかで魔法のようにコードを発見する - fae*