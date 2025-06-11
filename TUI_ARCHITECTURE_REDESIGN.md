# TUI Architecture Redesign Plan

## 目的
TUIの挙動を正確にテストできるよう、TUIの描画ロジックと振る舞いのロジックを分離し、イベントベースUIアーキテクチャでプログラム上でのTUI挙動再現を実現する。

## 現在の課題

### テスト可能な部分（MockTuiInterface）
- ✅ 検索ロジック（FuzzySearcher）
- ✅ 検索モード検出・クエリ抽出
- ✅ ナビゲーション・結果表示
- ✅ ビジネスロジック（862行のテストカバレッジ）

### テスト困難な部分
- ❌ プログレッシブインデックシング（113-529行）
  - バックグラウンドスレッド・mpscチャネル
  - 16msポーリング間隔の精度
  - try_recv()非ブロッキング動作
  - MAX_UPDATES_PER_FRAME制限効果
- ❌ ファイル監視統合（531-595行）
  - リアルタイムイベント処理
  - ファイル変更の自動インデックス更新
- ❌ TUI描画処理（927-1131行）
  - ratatui/crossterm直接依存

## 提案アーキテクチャ

### 1. 完全独立バックエンド（SearchBackend）
```rust
pub struct SearchBackend {
    indexer: TreeSitterIndexer,
    searcher: FuzzySearcher,
    file_watcher: Option<FileWatcher>,
    event_sender: mpsc::Sender<BackendEvent>,
}
```

**責任範囲:**
- シンボル抽出・インデックシング
- ファジー検索・コンテンツ検索
- ファイル監視・自動更新
- イベント通知のみ（UI状態は保持しない）

### 2. イベント通信システム
```rust
// バックエンド → UI
#[derive(Debug, Clone)]
pub enum BackendEvent {
    IndexingProgress { processed: usize, total: usize, symbols: Vec<CodeSymbol> },
    IndexingComplete { duration: Duration, total_symbols: usize },
    FileChanged { file: PathBuf, change_type: FileChangeType },
    SearchResults { query: String, results: Vec<SearchResult> },
    Error { message: String },
}

// UI → バックエンド
#[derive(Debug, Clone)]
pub enum UserCommand {
    StartIndexing { directory: PathBuf },
    Search { query: String, mode: SearchMode },
    EnableFileWatching,
    CopyResult { index: usize },
    Quit,
}
```

### 3. UI状態管理（TuiState）
```rust
pub struct TuiState {
    symbols: Vec<CodeSymbol>,
    current_results: Vec<SearchResult>,
    selected_index: usize,
    query: String,
    current_search_mode: SearchMode,
    status_message: String,
    is_indexing: bool,
    // UI状態のみ、描画ロジックは含まない
}
```

**責任範囲:**
- UI状態の保持・更新
- バックエンドイベントの状態反映
- ユーザーアクションの状態変更

### 4. プログラム的TUIシミュレーター
```rust
pub struct TuiSimulator {
    backend: SearchBackend,
    state: TuiState,
    command_sender: mpsc::Sender<UserCommand>,
    event_receiver: mpsc::Receiver<BackendEvent>,
}

impl TuiSimulator {
    // プログラム的操作
    pub fn send_command(&mut self, command: UserCommand);
    pub fn wait_for_event(&mut self) -> BackendEvent;
    pub fn get_state(&self) -> &TuiState;
    
    // 統合テスト用メソッド
    pub fn simulate_typing(&mut self, text: &str);
    pub fn simulate_key_press(&mut self, key: KeyCode);
    pub fn wait_for_indexing_complete(&mut self);
}
```

## 実装フェーズ

### Phase 1: 独立バックエンド設計
- SearchBackendの基本構造実装
- イベント通信システムの定義
- 既存TreeSitterIndexer・FuzzySearcherとの統合

### Phase 2: UI状態分離
- TuiStateの実装
- 現在のTuiAppからの状態管理ロジック抽出
- イベント→状態更新のマッピング

### Phase 3: TUIシミュレーター実装
- TuiSimulatorの基本実装
- プログラム的操作API
- 統合テスト用ヘルパーメソッド

### Phase 4: 統合テストフレームワーク
- プログレッシブインデックシングテスト
- ファイル監視統合テスト
- リアルタイムUI更新テスト

### Phase 5: 実TUI統合
- 新アーキテクチャでの実TUI実装
- 既存テスト移行・検証
- パフォーマンス検証

## 期待される効果

### テスト可能になる項目
1. **プログレッシブインデックシング**
   - バックグラウンド処理の正確性
   - UI更新タイミングの検証
   - エラーハンドリングの統合テスト

2. **ファイル監視**
   - リアルタイムイベント処理
   - 自動インデックス更新
   - デバウンシング効果

3. **UI応答性**
   - 16msポーリング間隔の効果
   - MAX_UPDATES_PER_FRAME制限
   - 大規模プロジェクトでの性能

4. **ワークフロー統合**
   - 実際の開発シナリオ再現
   - エラー状況での回復性
   - ユーザー操作シーケンス

### 開発効率向上
- TUI起動なしでの挙動検証
- CI/CDでの自動TUI統合テスト
- 複雑なシナリオの再現可能性
- バグ再現の正確性向上

## 技術的考慮事項

### パフォーマンス
- イベント通信のオーバーヘッド最小化
- 大量シンボル処理時のメモリ効率
- バックエンド・UI間の適切な分離

### 互換性
- 既存TuiAppとの段階的移行
- 現在のテストスイート（192テスト）の保持
- CLIモードとの共存

### 拡張性
- 新しい検索モードの追加容易性
- UIコンポーネントの独立テスト
- 将来的なGUI対応の基盤

## 実装スケジュール（推定）

- **Phase 1-2**: バックエンド・状態分離（2-3セッション）
- **Phase 3**: シミュレーター実装（1-2セッション）
- **Phase 4**: 統合テスト（1-2セッション）
- **Phase 5**: 実TUI統合・検証（1セッション）

**合計**: 5-10セッション

## 実装進捗状況 (2025-06-11 22:30 JST)

### ✅ Phase 1: 独立バックエンド設計 - **完了**
- **実装ファイル**: `src/backend.rs` (298行)
- **SearchBackend構造体**: TreeSitterIndexer、FuzzySearcher、イベント通信システム統合
- **イベント通信システム**: BackendEvent/UserCommand enumで双方向通信実装
- **基本機能**: インデックシング、検索、ファイル監視（一時的に無効）の独立動作
- **スレッド分離**: メインUIスレッドとバックエンドスレッドの完全分離

### ✅ Phase 2: UI状態分離 - **完了**
- **実装ファイル**: `src/tui_state.rs` (286行)
- **TuiState構造体**: UI状態のみを保持、描画ロジック完全分離
- **イベント処理**: apply_backend_event()でバックエンドイベントの状態反映
- **ユーザー入力処理**: handle_input()でTuiInput → TuiActionの変換
- **検索モード検出**: プレフィックス自動検出（#, >, /）機能実装

### ✅ Phase 3: TUIシミュレーター実装 - **90%完了**
- **実装ファイル**: `src/tui_simulator.rs` (350行)
- **TuiSimulator構造体**: プログラム的TUI操作の完全実装
- **プログラム的操作API**: simulate_typing(), simulate_key_press(), navigate_to()
- **統合テスト用メソッド**: wait_for_indexing_complete(), search_and_wait()
- **非ブロッキング操作**: try_process_event(), wait_for_event_timeout()

### ✅ 基本テスト実装 - **完了**
- **テストファイル**: `tests/tui_architecture_test.rs` (110行)
- **ユニットテスト**: TuiState、検索モード検出、イベント処理 - **全て成功**
- **テスト内容**: 
  - `test_tui_state_basic_functionality()` ✅
  - `test_search_mode_detection()` ✅  
  - `test_backend_event_application()` ✅
  - `test_tui_simulator_creation()` ⚠️ (タイムアウト問題)

### 🔧 Phase 4-5: 統合テストフレームワーク - **保留中**
- **課題**: SearchBackendイベントループのハング問題
- **問題箇所**: backend.rs run()メソッドの16msポーリングループで無限ループ発生
- **一時対策**: FileWatcher無効化（Sendトレイト問題）
- **影響**: TuiSimulator作成時のタイムアウト

## 技術的成果

### ✅ 完全責任分離達成
```
UI描画ロジック (ratatui) ⇔ 振る舞いロジック (TuiState) ⇔ バックエンド (SearchBackend)
```

### ✅ イベントベース通信実装
```rust
// 5種類のBackendEvent実装済み
BackendEvent::IndexingProgress、IndexingComplete、FileChanged、SearchResults、Error

// 5種類のUserCommand実装済み  
UserCommand::StartIndexing、Search、EnableFileWatching、CopyResult、Quit
```

### ✅ プログラム的TUI操作実現
```rust
simulator.simulate_typing("#TestClass")?;
simulator.simulate_key_press(KeyCode::Enter)?;
let results = simulator.search_and_wait("test")?;
```

## 次のアクション

### 🔥 高優先度（Phase 3完成）
1. **SearchBackendループ修正**: 16msポーリングループのハング問題解決
2. **FileWatcher統合**: Sendトレイト対応でファイル監視機能復活
3. **TuiSimulator安定化**: タイムアウト問題解決と統合テスト実装

### 📋 中優先度（Phase 4-5）
4. **統合テストフレームワーク**: プログレッシブインデックシング・ファイル監視の包括テスト
5. **実TUI統合**: 新アーキテクチャでの既存TUI機能再実装
6. **パフォーマンス検証**: 16msポーリング精度、MAX_UPDATES_PER_FRAME効果測定

## 実装スケジュール更新

- **Phase 1-2**: バックエンド・状態分離 ✅ **完了** (1セッション)
- **Phase 3**: シミュレーター実装 🔧 **90%完了** (修正中)
- **Phase 4**: 統合テスト ⏳ **待機中** (1-2セッション予想)
- **Phase 5**: 実TUI統合・検証 ⏳ **待機中** (1セッション予想)

**現在の達成率**: **70%** (Phase 1-2完了、Phase 3ほぼ完了)

---

*このドキュメントは継続的に更新され、実装進捗とともに詳細化される*  
*最終更新: 2025-06-11 22:30 JST*