# fae - リテラル検索 JSON-RPC サーバー

faeはJSON-RPCプロトコルを使用したリテラル検索サーバーです。ripgrepを使用して高速なテキスト検索を提供し、マイクロサービスアーキテクチャの一部として動作するよう設計されています。

## 🚀 機能

- **JSON-RPC 2.0準拠**: LSPスタイルのContent-Lengthフレーミング
- **ripgrep統合**: 高速なリテラル検索
- **非同期処理**: tokioベースの並行処理
- **リアルタイム結果配信**: 検索結果をストリーミングで送信

## 📋 JSON-RPC API仕様

### 通知 (Notifications)

#### `updateQuery`
検索を開始します。

```json
{
  "jsonrpc": "2.0",
  "method": "updateQuery",
  "params": {
    "query": "search_term"
  }
}
```

**パラメータ:**
- `query` (string): 検索するリテラル文字列

**動作:**
1. `clearSearchResults` 通知を送信
2. ripgrepで検索実行開始
3. **リアルタイム**: ripgrepが1行出力するたびに `pushSearchResult` 通知を即座に送信
4. 全ての出力処理完了後に `searchCompleted` 通知を送信

#### サーバーからの通知

#### `clearSearchResults`
検索結果をクリアします。

```json
{
  "jsonrpc": "2.0",
  "method": "clearSearchResults"
}
```

#### `pushSearchResult`
検索結果を1件送信します（リアルタイムストリーミング）。

```json
{
  "jsonrpc": "2.0",
  "method": "pushSearchResult",
  "params": {
    "filename": "src/main.rs",
    "line": 42,
    "offset": 1250,
    "content": "    println!(\"Hello world\");"
  }
}
```

**パラメータ:**
- `filename` (string): ファイルパス
- `line` (number): 行番号
- `offset` (number): バイトオフセット
- `content` (string): マッチした行の内容

**特徴:**
- ripgrepが1行出力するたびに即座に送信
- プロセスの終了を待たずにリアルタイム配信
- 大量結果時は100件ごとにyield（応答性確保）

#### `searchCompleted`
検索が完了したことを通知します。

```json
{
  "jsonrpc": "2.0",
  "method": "searchCompleted",
  "params": {
    "query": "search_term",
    "total_results": 42
  }
}
```

**パラメータ:**
- `query` (string): 実行された検索クエリ
- `total_results` (number): 見つかった結果の総数

### リクエスト (Requests)

#### `search.status`
サーバーの状態を取得します。

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "search.status"
}
```

**レスポンス:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "status": "ready",
    "current_query": "search_term",
    "search_root": "/path/to/search"
  }
}
```

## 🛠 セットアップ

### 必要な依存関係

1. **Rust**: 1.70以上
2. **ripgrep**: システムにインストール済み

```bash
# Ubuntuの場合
sudo apt install ripgrep

# macOSの場合  
brew install ripgrep

# Windowsの場合
choco install ripgrep
```

### ビルドと実行

```bash
# プロジェクトをビルド
cargo build --release

# 利用可能なサービス一覧を表示
cargo run --bin fae-service list

# リテラル検索サーバーを起動（カレントディレクトリで検索）
cargo run --bin fae-service start search:literal

# 特定のディレクトリで検索
cargo run --bin fae-service --root /path/to/search start search:literal

# ログレベルを設定
cargo run --bin fae-service --log-level debug start search:literal
```

### コマンドラインオプション

```
fae-service [OPTIONS] [COMMAND]

COMMANDS:
    start    サービスを起動
    list     利用可能なサービス一覧を表示
    help     ヘルプを表示

OPTIONS:
    -r, --root <ROOT>           検索対象のルートディレクトリ [default: .]
    -l, --log-level <LEVEL>     ログレベル [default: info]
        --ripgrep-path <PATH>   ripgrepバイナリのパス
    -h, --help                  ヘルプを表示
    -V, --version               バージョンを表示

EXAMPLES:
    fae-service start search:literal           # リテラル検索サービス起動
    fae-service list                          # サービス一覧表示
    fae-service --root /path start search:literal  # 指定ディレクトリで起動
```

## 🧪 テスト

```bash
# 全テストを実行
cargo test

# 特定のテストを実行
cargo test literal_search

# ripgrepを使うテストも含める（ripgrepが必要）
cargo test -- --ignored
```

## 📖 使用例

### Pythonクライアント

```python
#!/usr/bin/env python3
import json
import subprocess

# サーバーを起動
process = subprocess.Popen(
    ["cargo", "run"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    text=True
)

# updateQuery通知を送信
message = {
    "jsonrpc": "2.0",
    "method": "updateQuery",
    "params": {"query": "function"}
}

json_str = json.dumps(message)
content_length = len(json_str)
full_message = f"Content-Length: {content_length}\r\n\r\n{json_str}"

process.stdin.write(full_message)
process.stdin.flush()

# レスポンスを読み取り（省略）
```

### 付属のテストクライアント

```bash
# Pythonテストクライアントを実行
python3 examples/test_client.py "function"

# 特定のディレクトリで検索
cargo run --bin fae-service --root /path/to/project start search:literal &
python3 examples/test_client.py "TODO"
```

## 🏗 アーキテクチャ

### コンポーネント構成

```
┌─────────────────────┐
│   JSON-RPC Client   │
└──────────┬──────────┘
           │ stdin/stdout
           │ (LSP-style framing)
┌──────────▼──────────┐
│ JsonRpcStdioAdapter │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│   JsonRpcEngine     │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ LiteralSearchHandler│
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│      ripgrep        │
└─────────────────────┘
```

### 主要モジュール

- **`jsonrpc`**: JSON-RPC 2.0プロトコル実装
  - `engine.rs`: メッセージ処理エンジン
  - `stdio.rs`: stdin/stdout通信アダプター
  - `message.rs`: メッセージ構造体とエラー処理
  - `handler.rs`: ハンドラートレイト定義

- **`services`**: 各種検索サービス
  - `literal_search`: リテラル検索ハンドラー
    - ripgrep統合
    - 非同期検索処理  
    - 結果ストリーミング
  - 将来追加予定: symbol_search, file_search, regex_search等

## 🔧 開発

### プロジェクト構造

```
src/
├── main.rs              # メインアプリケーション
├── lib.rs               # ライブラリエントリーポイント
├── services/            # 各種検索サービス
│   ├── mod.rs           # サービスモジュール定義
│   └── literal_search.rs # リテラル検索ハンドラー
└── jsonrpc/             # JSON-RPC実装
    ├── mod.rs
    ├── engine.rs        # メッセージ処理エンジン
    ├── handler.rs       # ハンドラートレイト
    ├── message.rs       # メッセージ構造体
    └── stdio.rs         # stdin/stdout統合

examples/
└── test_client.py       # Pythonテストクライアント

tests/
└── integration_tests.rs # 統合テスト（今後追加予定）
```

### 拡張ポイント

1. **新しい検索サービス**: `services/`モジュールで`JsonRpcHandler`トレイトを実装
2. **バックエンド変更**: `ripgrep`の代わりに他の検索ツールを使用
3. **通知拡張**: 検索進捗やエラー通知の追加
4. **フィルタリング**: ファイルタイプや除外パターンの追加

将来の検索サービス例：
- `symbol_search`: Tree-sitterベースのシンボル検索
- `file_search`: ファイル名・パス検索
- `regex_search`: 正規表現検索
- `git_search`: Git履歴統合検索

## 📝 ライセンス

このプロジェクトはMITライセンスの下で公開されています。

## 🤝 貢献

1. Issueを作成して問題や提案を報告
2. フォークしてfeatureブランチを作成
3. テストを追加・実行
4. プルリクエストを送信

## 📞 サポート

- GitHub Issues: バグ報告や機能リクエスト
- ドキュメント: [docs/](docs/) ディレクトリ
- サンプル: [examples/](examples/) ディレクトリ