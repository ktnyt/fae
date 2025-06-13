# JSON-RPC Samples

このディレクトリには、faeプロジェクトのJSON-RPC基盤の動作確認用サンプルコードが含まれています。

## ファイル構成

### `simple_echo_test.rs` - Echo Server Sample
シンプルなJSON-RPCサーバーの実装例。stdio経由で通信し、以下の機能を提供：

**メソッド**:
- `echo`: 受信したパラメータをそのまま返す
- `ping`: "pong"を返す  
- `bye`: "bye"を返してサーバーを停止

**通知**:
- `poke`: クライアントにpingリクエストを送信し、pongレスポンスを期待

### `test_simple_echo.rs` - Test Client Sample
`simple_echo_test`サーバーをテストするクライアントの実装例。以下の機能を持つ：

**特徴**:
- タイムアウト付きリクエスト処理
- サーバープロセスの自動起動と管理
- 段階的テスト実行（echo → poke → ping → bye）
- 緊急時のプロセス強制終了

## 使用方法

### 手動実行
```bash
# サーバー単体実行（stdio mode）
cargo run --bin simple_echo_test

# クライアント実行（サーバーを自動起動）
cargo run --bin test_simple_echo
```

### ログ付き実行
```bash
# 詳細ログでテスト実行
RUST_LOG=debug cargo run --bin test_simple_echo
```

## アーキテクチャの特徴

### 物理的プロセス分離
- サーバーとクライアントは完全に独立したプロセス
- stdio（stdin/stdout）経由のJSON-RPC通信
- プロセス境界による強制的な疎結合

### 双方向通信
- クライアント→サーバー：リクエスト/通知
- サーバー→クライアント：リクエスト/通知（pokeによる逆方向通信）
- JSON-RPC 2.0準拠の通信プロトコル

### タイムアウト・エラーハンドリング
- 2秒のリクエストタイムアウト
- タイムアウト時の子プロセス強制終了
- グレースフルシャットダウン

## 実装のポイント

### JsonRpcBase Usage
- `JsonRpcBase::new_stdio()`: サーバー側（stdio mode）
- `JsonRpcBase::from_child()`: クライアント側（子プロセス管理）
- `MainLoopHandler`トレイトによるイベントループ実装

### LSPスタイルフレーミング
```
Content-Length: 42\r\n\r\n{"jsonrpc":"2.0","method":"ping","id":1}
```

### エラー処理パターン
- `request_timeout()`: タイムアウト付きリクエスト
- 適切なJSON-RPCエラーレスポンス
- プロセス管理とリソースクリーンアップ

これらのサンプルは、faeプロジェクトのJSON-RPC基盤が「どうあがいても疎結合にならざるを得ない」アーキテクチャを実現していることを実証しています。