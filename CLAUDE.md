# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**fae** (Fast And Elegant) is a high-performance code symbol search tool written in Rust. It provides blazingly fast fuzzy search across codebases with Tree-sitter-based symbol extraction, supporting 25+ programming languages with a beautiful TUI interface.

### Design Philosophy
- **Real-time First**: 入力に応じた即座の結果更新
- **Memory Efficient**: 巨大プロジェクトでもスマートなキャッシュ戦略
- **Async Design**: UIブロッキングなしの快適な操作性
- **Test Driven**: 全機能に対して網羅的なテスト

### Multi-mode Search
1. **Content Search** (default) - ファイル内容のテキスト検索
2. **Symbol Search** (`#prefix`) - 関数・クラス・変数名での検索
3. **File Search** (`>prefix`) - ファイル名・パスでの検索
4. **Regex Search** (`/prefix`) - 高度なパターンマッチング

## Development Commands

### Testing
```bash
# Run all tests (RECOMMENDED - full execution)
cargo test --lib -- --test-threads=1

# Quick test run with timeout (for rapid development cycles)
timeout 20s cargo test --lib -- --test-threads=1

# IMPORTANT: Full test suite requires ~60 seconds
# Integration tests with file watching, Actor coordination, and async processing
# need sufficient time to complete properly

# Run with time reporting (nightly only)
# RUST_TEST_TIME_UNIT=500,2000 cargo +nightly test --lib -- --test-threads=1 --ensure-time -Z unstable-options

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
cargo watch -x "test --lib -- --test-threads=1"
```

**Test Timeout Guidelines:**
- **Unit Tests**: 10-20 seconds sufficient
- **Integration Tests**: 60+ seconds required (file system operations, Actor coordination)
- **Full Test Suite**: 60+ seconds (avoid timeout for complete validation)
- **CI/Development**: Use timeouts only for rapid iteration cycles

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

### Code Coverage
```bash
# Install coverage tool
cargo install cargo-llvm-cov

# Run tests with coverage analysis
make test-coverage  # or: cargo llvm-cov --lib --package fae --html -- --test-threads=1

# Open HTML coverage report
make test-coverage-open  # or: open target/llvm-cov/html/index.html

# Command module specific coverage
make test-coverage-command

# CI-friendly coverage (no HTML)
make test-coverage-ci

# Development workflow (format, lint, test, coverage)
make dev
```

**Coverage Targets:**
- **command.rs**: 96.40% line coverage, 79.42% region coverage (excellent)
- **Overall project**: Aim for >85% line coverage
- **Critical modules**: Aim for >90% line coverage

### Development Tools
```bash
# Debug logging
RUST_LOG=debug cargo run

# Profile specific files (custom benchmark tools)
cargo run --bin profile_file -- src/tui.rs
cargo run --bin benchmark_indexing

# Test Tree-sitter symbol extraction
cargo run --bin test_tree_sitter_symbols -- src/

# Development completion notifications (フラクタルスプリント用)
# バナー形式通知 (推奨)
osascript -e 'display notification "実装完了。レビューをお願いします🔍" with title "フラクタルスプリント完了" sound name "Glass"'

# 重要な完了通知
osascript -e 'display notification "全ての実装とテストが完了しました" with title "開発完了" subtitle "次のスプリントに進む準備ができました" sound name "Hero"'

# 品質チェック完了通知
osascript -e 'display notification "cargo test, clippy, fmt すべて完了" with title "品質チェック完了" sound name "Ping"'

# エラー通知
osascript -e 'display notification "テストで問題が発見されました" with title "スプリント中断" sound name "Basso"'
```

**通知音オプション**:
- `"Glass"` - クリアで控えめ（推奨）
- `"Ping"` - 短くシンプル  
- `"Hero"` - 完了感のある音
- `"Purr"` - 柔らかい音
- `"Basso"` - エラー・警告用

## High-Level Architecture

### Core Components
- **`src/main.rs`**: CLI entry point (async support)
- **`src/lib.rs`**: Public API and re-exports
- **`src/types.rs`**: Core data structures (SearchResult, SymbolType, etc.)
- **`src/tui.rs`**: TUI with async iterator + message engine architecture
- **`src/cli/`**: CLI application and search coordination
- **`src/searchers/`**: Multi-mode search engines with backend support
- **`src/languages/`**: Tree-sitter integration for symbol extraction

### Key Design Patterns
- **Event-Driven TUI**: tokio::select! for event multiplexing
- **Async Integration**: spawn_blocking for sync/async bridge
- **Multi-mode Search**: Content, Symbol (#), File (>), and Regex (/) search modes
- **Progressive Indexing**: Non-blocking background processing
- **Parallel Processing**: Rayon-based concurrent file processing

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

## Development Guidelines

### Adding New Languages
1. Add tree-sitter dependency to `Cargo.toml`
2. Update language configuration in `src/languages/`
3. Add S-expression queries for symbol extraction
4. Update file extension patterns

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

### TUI Architecture Patterns (Phase 6-7 実装済み)
- **非同期Iterator + メッセージエンジン**: tokio::select!によるイベント多重化
- **ratatui + crossterm**: ターミナル状態管理とクリーンアップ
- **spawn_blocking**: 同期コードの非同期統合パターン
- **イベント型安全性**: TuiEvent, InputEvent, SearchEvent による型安全な処理

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
cargo test --lib -- --test-threads=1 # All tests must pass (no timeout for reliability)
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

## 重要な学習と記憶 (Lessons Learned)

### テストタイムアウトの重要性 (2025-06-16)
**問題**: 30秒のtimeoutでテスト実行すると、統合テストが途中で中断される

**原因**:
- 統合テスト（WatchActor + SymbolIndexActor）は複雑な非同期処理を含む
- ファイルシステム監視、Actor間協調、複数ファイル操作は時間がかかる
- 実際の実行時間: 全81テストで54.46秒

**解決策**:
- **開発時**: timeoutなしで実行 (`cargo test --lib -- --test-threads=1`)
- **品質保証**: 必ず完全実行でテスト結果を検証
- **素早い確認**: 短いtimeout（20秒）は単体テストのみ

**重要な気づき**:
```bash
# ❌ 危険: 統合テストが途中で止まる可能性
timeout 30s cargo test --lib -- --test-threads=1

# ✅ 安全: 全テストが確実に完了
cargo test --lib -- --test-threads=1
```

**アーキテクチャへの影響**:
- 競合状態防止機能のテストは特に時間がかかる
- 複数ファイルの並行更新テストは十分な実行時間が必要
- Actor間のメッセージ協調テストは非同期待機を含む

### 競合状態防止の実装成果 (2025-06-16)
- **処理中ファイル追跡**: `Arc<Mutex<HashSet<String>>>`による安全な状態管理
- **優雅な中断処理**: 進行中処理の適切な停止と新規処理の開始
- **包括的テストカバレッジ**: 高速連続更新と初期化中断の両方をテスト
- **警告ゼロ達成**: クリーンなコードベース（81テスト、5警告→0警告）

## Current Status (2025-06-16)

### Completed Features ✅
- **Phase 1-7**: Complete TUI implementation with async iterator + message engine
- **Multi-mode Search**: Content, Symbol (#), File (>), and Regex (/) search modes
- **Backend Integration**: ripgrep/ag support with fallback
- **Test Coverage**: 171 total tests (119 existing + 52 new TUI-related)
- **Production Ready**: Full CLI compatibility maintained
- **Symbol Index Architecture**: 完全な階層化アーキテクチャが実装済み
- **TUI Refactoring Phase 1**: Modular architecture implemented (2025-06-13)
  - Separated input handling, text editing, styles, and constants
  - Eliminated 200+ lines of duplicated code from src/tui.rs
  - Created reusable EditableText trait pattern
  - Unified style management with TuiStyles structure
  - Improved maintainability and testability

### Symbol Index Implementation Status (2025-06-13)
- **SearchCoordinator**: プログレッシブインデックス構築、並列シンボル抽出
- **SymbolIndex**: SkimMatcherベースの高速ファジー検索、メタデータ統合
- **CacheManager**: LRUキャッシュ、変更検知、100MBメモリ制限
- **Tree-sitter Integration**: 4言語対応（Rust, TypeScript, JavaScript, Python）
- **IndexManager**: ファイル発見、.gitignore統合、バイナリ除外

### Next Phase Candidates (Phase 8-9)
- **File Watching**: Real-time index updates with notify integration
- **Git Integration**: Changed file detection, branch information
- **Configuration**: .fae.toml support for customization
- **Performance**: Further optimizations for large codebases


## 📚 詳細ドキュメント

**プロジェクト固有の詳細情報は以下のドキュメントを参照**:

- **[.claude/edge_cases.md](.claude/edge_cases.md)**: 包括的エッジケース分析、セキュリティ脆弱性、パフォーマンスボトルネック
- **[.claude/testing.md](.claude/testing.md)**: テスト戦略、カバレッジ分析、推奨テスト実装計画
- **[.claude/development.md](.claude/development.md)**: 開発フェーズの詳細履歴、実装ガイドライン
- **[.claude/tui_implementation.md](.claude/tui_implementation.md)**: TUI実装記録 (Phase 6-7)、アーキテクチャ詳細
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**: システム設計とアーキテクチャ概要
- **[docs/DESIGN.md](docs/DESIGN.md)**: プロジェクト設計哲学と基本フロー

## Performance Metrics

- **Indexing Speed**: ~46,875 symbols/second after regex optimization
- **Memory Usage**: Efficient with large codebases through streaming processing  
- **UI Responsiveness**: 16ms polling interval for real-time updates
- **Test Coverage**: 171 comprehensive tests covering core functionality