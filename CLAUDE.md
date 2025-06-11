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

- **`src/indexer.rs`**: Core indexing logic with parallel processing
- **`src/tui.rs`**: Progressive indexing and UI responsiveness
- **`src/parsers/symbol_extractor.rs`**: Tree-sitter query management
- **`src/filters/gitignore_filter.rs`**: File exclusion logic
- **`tests/`**: Comprehensive test suite (92+ tests) with security and real-world scenarios

## Technical Debt & Known Issues

⚠️ **Outdated Configuration Files**: CONTRIBUTING.md, GitHub Actions, and PR templates reference TypeScript/Node.js commands but should use Rust/Cargo commands.

## Performance Metrics

- **Indexing Speed**: ~46,875 symbols/second after regex optimization
- **Memory Usage**: Efficient with large codebases through streaming processing
- **UI Responsiveness**: 16ms polling interval for real-time updates
- **Test Coverage**: 92+ comprehensive tests covering core functionality