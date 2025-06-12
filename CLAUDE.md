# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SFS (Symbol Fuzzy Search) is a high-performance code symbol search tool written in Rust. It provides blazingly fast fuzzy search across codebases with Tree-sitter-based symbol extraction, supporting 25+ programming languages with a beautiful TUI interface.

## Development Commands

### Testing
```bash
# Run all tests
cargo test

# Run specific test categories
cargo test --test indexer_test
cargo test --test searcher_test  
cargo test --test tui_test
cargo test --test cli_integration_tests
cargo test --test security_test
cargo test --test real_world_scenarios_test

# Run performance benchmarks
cargo bench

# Watch mode for development
cargo install cargo-watch
cargo watch -x test
```

### Code Quality
```bash
# Format code
cargo fmt

# Lint and style checks
cargo clippy

# Check compilation without building
cargo check

# Build optimized release
cargo build --release
```

### Development Tools
```bash
# Debug logging
RUST_LOG=debug cargo run

# Profile specific files (custom benchmark tools)
cargo run --bin profile_file -- src/tui.rs
cargo run --bin benchmark_indexing

# Test Tree-sitter symbol extraction
cargo run --bin test_tree_sitter_symbols -- src/
```

## High-Level Architecture

### Core Components
- **`main.rs`**: CLI entry point with clap argument parsing and mode selection
- **`lib.rs`**: Public API and re-exports for external consumption
- **`types.rs`**: Core data structures (`CodeSymbol`, `SymbolType`, etc.)
- **`indexer.rs`**: `TreeSitterIndexer` with progressive indexing and parallel processing
- **`searcher.rs`**: `FuzzySearcher` with multi-mode search capabilities
- **`tui.rs`**: `TuiApp` with ratatui-based interactive interface

### Modular Architecture
- **`parsers/`**: Tree-sitter integration and language-specific symbol extraction
  - `tree_sitter_config.rs`: Language configuration and parser setup
  - `symbol_extractor.rs`: AST traversal and symbol collection
- **`filters/`**: File processing and filtering logic
  - `file_filter.rs`: Binary detection, size limits, and file type filtering
  - `gitignore_filter.rs`: .gitignore support with ignore crate

### Key Design Patterns
- **Progressive Indexing**: Background thread with mpsc channels for non-blocking UI updates
- **Multi-mode Search**: Content, Symbol (#), File (>), and Regex (/) search modes
- **Strategy Pattern**: `DefaultDisplayStrategy` for customizable result display
- **Parallel Processing**: Rayon-based concurrent file processing for performance

## Important Implementation Details

### Performance Optimizations
- **Regex Pre-compilation**: Use `OnceLock` for 3300x performance improvement
- **Parallel Processing**: `rayon::par_iter()` for CPU-intensive operations
- **Smart Caching**: Pre-compiled Tree-sitter queries and pattern matchers
- **File Filtering**: Early exclusion of binary files, large files (>1MB), and temp files

### Tree-sitter Integration
- **Language Support**: 25+ languages with proper S-expression query syntax
- **Symbol Types**: 11 comprehensive types (Function, Class, Variable, etc.)
- **Query Syntax**: Use field names (`name:`) in capture patterns for accuracy
- **Error Handling**: Graceful fallback when Tree-sitter parsing fails

### Security & Robustness
- **Path Safety**: Protection against path traversal and symlink loops
- **Input Validation**: Sanitization of search queries and file paths
- **Resource Limits**: Memory bounds, file size limits, and processing timeouts
- **Error Recovery**: Robust error handling that continues processing on failures

## Development Guidelines

### Complex System Development Principles
⚠️ **Critical Development Lessons**:
- **Define Expected Behavior First**: Before implementing complex features, clearly specify the complete behavior and interaction patterns
- **Incremental Implementation**: Break complex systems into simple, testable components and build incrementally
- **Debug Strategy**: Use systematic debugging with logging/tracing - assumptions about system behavior are often incorrect
- **End-to-End Testing**: Write tests that validate complete workflows, not just individual components
- **Architecture Understanding**: Ensure all team members (including yourself) fully understand the designed system behavior before implementation

### Adding New Languages
1. Add tree-sitter dependency to `Cargo.toml`
2. Update language configuration in `parsers/tree_sitter_config.rs`
3. Add S-expression queries in `symbol_extractor.rs`
4. Update file extension patterns in `indexer.rs`

### Performance Considerations
- Always use `rayon::par_iter()` for file processing
- Pre-compile regex patterns with `OnceLock`
- Consider memory usage for large codebases
- Profile with custom benchmark tools before optimizing

### Testing Strategy
- **Unit Tests**: Core functionality with mocks and fixtures
- **Integration Tests**: Real file scenarios and TUI workflows
- **Security Tests**: Malicious input and edge cases
- **Performance Tests**: Benchmark regressions and scalability

## CLI Usage Notes

### TUI Mode
- Default when no query provided
- Progressive indexing with real-time updates
- Multi-mode search with prefix detection (#, >, /)
- Keyboard shortcuts: Enter (copy), Esc (quit), Ctrl+N/P (navigate)

### CLI Mode  
- Direct symbol search with output to stdout
- Type filtering with `--types` flag
- Threshold adjustment with `--threshold`
- Verbose output with `--verbose`

## Git Workflow

### Branch Naming
Format: `{issue-number}/{type}/{description}`
- Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`
- Example: `2/feat/clipboard-copy-on-enter`

### Commit Messages
- Use [gitmoji](https://gitmoji.dev/) prefixes recommended
- Focus on clear, meaningful descriptions
- Include `Closes #issue-number` in PR descriptions

### Before Committing
```bash
cargo test           # All tests must pass
cargo clippy         # No linting issues
cargo fmt            # Code must be formatted
cargo build --release # Release build must succeed
```

## Important Files to Understand

- **`src/search_coordinator.rs`**: Core indexing logic with parallel processing
- **`src/tui.rs`**: Progressive indexing and UI responsiveness
- **`src/languages/common.rs`**: Tree-sitter query management
- **`src/index_manager.rs`**: File exclusion logic and discovery
- **`tests/`**: Comprehensive test suite (92+ tests) with security and real-world scenarios

## Edge Case Analysis & Testing Gaps (2025-06-13)

### 🔍 Critical Edge Cases Identified
1. **Error Handling**: `unwrap()` usage in display utilities could panic on empty paths
2. **UTF-8 Processing**: Tree-sitter integration lacks fallback for encoding errors
3. **Resource Management**: No timeout controls for external commands (ripgrep/ag)
4. **Concurrency**: Channel backpressure and Mutex contention in progressive indexing
5. **File System**: Permission errors misclassified as binary files in index_manager

### 📝 Recommended Test Additions
- **High Priority**: UTF-8 error handling, large file streaming, Tree-sitter parse failures
- **Medium Priority**: Terminal state recovery, memory exhaustion scenarios
- **Low Priority**: Network filesystem performance, internationalization edge cases

### 🎯 Specific Vulnerable Code Locations
- `display/utils.rs:75` - Array access without bounds check
- `search_coordinator.rs:151-155` - Incomplete Mutex panic propagation
- `tree_sitter.rs:32` - No partial parse result recovery
- `index_manager.rs:162-164` - Incorrect binary file classification

## セキュリティ脆弱性分析 (2025-06-13)

### 🛡️ セキュリティ分析結果概要
**全体評価**: 基本的なセキュリティ対策は実装済み、中程度のリスクレベル

### 🔴 高リスク脆弱性

#### 1. Path Traversal & Injection
- **場所**: `index_manager.rs:147-149`, `display/utils.rs:52-56`
- **脆弱性**: `strip_prefix()` + `unwrap_or()` でパストラバーサル防止は実装済み
- **リスク**: 低 - ignore crateとWalkBuilderによる適切な保護
- **攻撃例**: `../../../etc/passwd` → WalkBuilderが自動的にブロック

#### 2. Input Sanitization
- **場所**: `ripgrep_backend.rs:78`, `ag_backend.rs:76`
- **脆弱性**: `-F`/`--literal`フラグで正規表現インジェクション防止済み
- **リスク**: 低 - 外部コマンドの引数は適切にエスケープ
- **攻撃例**: `; rm -rf /` → literalフラグにより無害化

#### 3. Resource Exhaustion Attacks
- **場所**: `index_manager.rs:44`, `ripgrep_backend.rs:77`
- **脆弱性**: ファイルサイズ制限（1MB）、並列処理制御あり
- **リスク**: 中 - 大量ファイル攻撃には脆弱な可能性
- **対策**: rayonによる自動並列度制御、ファイルサイズ制限

#### 4. Command Injection
- **場所**: `ripgrep_backend.rs:71-82`, `ag_backend.rs:71-80`
- **脆弱性**: Command::new() + .args()でシェルインジェクション防止済み
- **リスク**: 低 - シェル経由ではない直接実行
- **実装**: 引数配列による安全な外部コマンド実行

### 🟡 中リスク脆弱性

#### 5. File Access Control
- **場所**: `index_manager.rs:161-163`
- **脆弱性**: 読み込み失敗時にバイナリ扱い（権限エラーも含む）
- **リスク**: 中 - 権限エラーの詳細が隠蔽される
- **改善案**: 権限エラーとバイナリファイルの区別

#### 6. Information Disclosure
- **場所**: `search_coordinator.rs:314`, `cli_app.rs`など
- **脆弱性**: エラーメッセージでパス情報が露出
- **リスク**: 低 - ファイルパス以外の機密情報漏洩なし
- **対策**: ログレベル制御、相対パス表示

### 🟢 低リスク・対策済み

#### 7. Memory Safety
- **調査結果**: unsafeコードブロックなし
- **リスク**: 低 - Rustの所有権システムによる保護
- **対策**: コンパイラレベルでの自動メモリ安全性

#### 8. Privilege Escalation
- **調査結果**: 権限昇格機能なし、ユーザー権限内での動作
- **リスク**: 低 - 外部コマンドも同一権限で実行
- **対策**: current_dir()による実行ディレクトリ制限

### 🎯 推奨セキュリティ改善項目
1. **ファイル権限エラー処理の改善** (`index_manager.rs:161-163`)
2. **外部コマンドタイムアウト制御の追加**
3. **並列処理でのリソース制限強化**
4. **エラーメッセージの情報漏洩対策**

### 💡 セキュリティベストプラクティス遵守状況
- ✅ 入力サニタイゼーション（literal検索）
- ✅ パストラバーサル防止（ignore crate）
- ✅ コマンドインジェクション防止（引数配列）
- ✅ メモリ安全性（Rustコンパイラ）
- ⚠️ リソース制限（部分的実装）
- ⚠️ エラー情報制御（改善余地）

## 🎯 包括的エッジケース分析 (2025-06-13)

### Critical Testing Coverage Gaps

**現在の128+テストでカバーされていない重要なエッジケース**:

#### 1. UTF-8処理の詳細エッジケース ❌ 不十分
- BOM (Byte Order Mark) 付きファイル処理
- 複数バイトUTF-8文字境界での切断処理
- Unicode正規化 (NFD vs NFC) の違い
- 異なるエンコーディングファイルの誤検出
- 不正なUTF-8シーケンスの部分的回復

#### 2. Tree-sitterパーサー障害処理 ❌ 不十分
- パーサーメモリ不足・深いネスト構造
- 無限ループを引き起こす文法構造
- パーサータイムアウト処理
- 部分的に解析されたASTからのシンボル抽出
- 自動生成ファイル（webpack bundle等）の処理

#### 3. チャンネル通信とスレッド間エラー ❌ 完全欠如
- mpscチャンネル送信側切断
- 受信側応答なし時のタイムアウト
- インデックス構築中のメインスレッド終了
- 複数スレッドからの同時検索要求
- デッドロック検出とリカバリ

#### 4. メモリ枯渇とリソース制限 ❌ 不十分
- 実際のメモリ枯渇シミュレーション
- ファイルディスクリプタ枯渇
- ディスク容量不足での一時ファイル作成
- 非常に大きなファイル（数GB）での処理
- システムリソース制限でのgraceful degradation

#### 5. ファイルシステム権限とアクセス制御 ❌ 部分的
- アクセス権限なしディレクトリ
- シンボリックリンクループ
- FIFO/named pipe処理
- ネットワークファイルシステム上のファイル
- 所有者権限変更中のファイル

#### 6. プラットフォーム固有問題 ❌ 完全欠如
- Windows: パス区切り文字、予約名、長いパス
- macOS: NFD正規化、リソースフォーク
- Linux: case-sensitive vs case-insensitive ファイルシステム

### 推奨追加テストファイル（最優先8つ）
1. **`utf8_edge_cases_test.rs`** - UTF-8境界ケース
2. **`tree_sitter_failure_test.rs`** - Tree-sitter障害詳細
3. **`channel_communication_test.rs`** - mpsc通信テスト
4. **`resource_limits_test.rs`** - リソース枯渇テスト
5. **`filesystem_permissions_test.rs`** - ファイルシステム権限
6. **`timing_boundary_test.rs`** - 境界条件・タイミング
7. **`platform_specific_test.rs`** - プラットフォーム固有
8. **`backend_integration_test.rs`** - 外部コマンド統合障害

### ⚡ Critical Performance Bottlenecks

#### 1. メモリ割り当て問題
- **CacheManager**: 無制限メモリ増加 (10GB消費可能性)
- **TUI Events**: 非同期チャンネルメッセージ蓄積

#### 2. CPU集約的操作
- **symbol_index.rs**: O(n)線形検索 (100万シンボル→指数的時間)
- **content_search.rs**: LRUキャッシュロック競合

#### 3. I/Oボトルネック
- **index_manager.rs**: 大規模ディレクトリ走査 (10万ファイル)
- **Tree-sitter解析**: 1MBファイルで数秒停止

#### 4. 並列処理非効率性
- **Rayon並列処理**: 粒度不適切 (1つの大ファイルが全スレッドブロック)
- **非同期ストリーム**: UI応答性完全停止

#### 5. 正規表現Catastrophic Backtracking
- `(a+)+b` 等の爆発的バックトラック → CPU 100%消費

#### 6. チャンネル通信デッドロック
- mpscバックプレッシャー処理なし → 無制限メモリ蓄積

### 📋 開発者向け優先度マトリックス

**単体テスト実装優先度**:
1. **最高優先**: チャンネル通信エラー、Tree-sitter障害処理
2. **高優先**: UTF-8境界ケース、リソース枯渇テスト
3. **中優先**: プラットフォーム固有、バックエンド統合テスト

**パフォーマンス改善優先度**:
1. **緊急**: CacheManagerメモリ制限実装
2. **高**: 正規表現タイムアウト・パターン検証
3. **中**: Tree-sitter解析タイムアウト実装

## Technical Debt & Known Issues

⚠️ **Outdated Configuration Files**: CONTRIBUTING.md, GitHub Actions, and PR templates reference TypeScript/Node.js commands but should use Rust/Cargo commands.

## Development Status Analysis (as of 2025-06-13)

### Completed Phases ✅
- **Phase 1-4**: 完全実装済み（基本データ構造、Tree-sitter統合、ファイル発見エンジン、インデックス構築パイプライン）
- **Phase 5**: マルチモード検索実装済み（コンテンツ・シンボル・ファイル・正規表現の全4モード対応）
- **包括的テスト**: 128+テスト実装済み（7つのテストファイル: CLI統合、E2E、パフォーマンス、エラーハンドリング、Tree-sitter統合等）

### Phase 4-5完了マーカー
- README.mdに明記: "Phase 4-5 complete (all features implemented, 128 tests passing). Production ready."
- 最新コミット: "📚 docs: Update README.md to reflect Phase 4-5 completion and production readiness"
- 前回コミット: "🧪 test: Implement comprehensive test suite for Phase 4-5 completion"

### 次フェーズ実装対象 🔄

#### Phase 6-7: TUI実装 (次の主要開発項目)
- **TUI Implementation**: ratatui-based real-time search, keyboard navigation
- **Git Integration**: Changed file detection, branch information  
- **File Watching**: Real-time index updates, notify integration

#### Phase 8-9: 高度な機能
- **非同期・ファイル監視**: 非同期チャンネル通信、notify integration、clipboard integration
- **最適化と洗練**: パフォーマンス最適化、設定ファイル(.fae.toml)サポート

### 現在のテスト構成
```
tests/
├── cli_regression_test.rs         # CLI回帰テスト
├── content_search_test.rs         # コンテンツ検索テスト
├── debug_tree_sitter.rs           # Tree-sitter デバッグ
├── e2e_workflow_test.rs           # エンドツーエンド ワークフロー
├── error_handling_test.rs         # エラーハンドリング
├── performance_test.rs            # パフォーマンステスト
└── tree_sitter_integration_test.rs # Tree-sitter統合テスト
```

### 生産準備完了
- 「Production ready」として明記済み
- 全128テスト通過
- 4言語サポート（TypeScript, JavaScript, Python, Rust）
- 4検索モード実装（Content, Symbol #, File >, Regex /）
- 外部バックエンド統合（ripgrep/ag + フォールバック）
- ストリーミング検索・パイプライン対応

## Performance Metrics

- **Indexing Speed**: ~46,875 symbols/second after regex optimization
- **Memory Usage**: Efficient with large codebases through streaming processing
- **UI Responsiveness**: 16ms polling interval for real-time updates
- **Test Coverage**: 92+ comprehensive tests covering core functionality