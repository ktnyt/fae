# fae - Fast And Elegant code search

[![CI](https://github.com/ktnyt/fae/workflows/CI/badge.svg)](https://github.com/ktnyt/fae/actions)
[![Coverage](https://codecov.io/gh/ktnyt/fae/branch/main/graph/badge.svg)](https://codecov.io/gh/ktnyt/fae)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Lightweight and magical code discovery tool with real-time search capabilities

## Overview

**fae** is an interactive TUI-based tool for real-time multi-dimensional code search across large codebases. It provides fast, intuitive code discovery with high performance even on large projects.

> **✅ Development Status**: Phase 4-5 complete (all features implemented, 128 tests passing). Production ready.

## Features

### Multi-Mode Search

- **Content Search** (default) - Full-text search within file contents
- **Symbol Search** (`#prefix`) - Functions, classes, variables by name
- **File Search** (`@prefix`) - File names and paths
- **Regex Search** (`/prefix`) - Advanced pattern matching

### Key Features

- **Fast Symbol Search** ✅ - Tree-sitter + fuzzy search (4 languages)
- **Parallel Index Building** ✅ - High-speed indexing with rayon
- **Smart File Discovery** ✅ - .gitignore support, binary detection, size limits
- **External Backend Integration** ✅ - ripgrep/ag support + fallback
- **Streaming Search** ✅ - Real-time ag/rg-style output
- **Comprehensive Testing** ✅ - 128 tests passing (CLI, E2E, performance, error handling)
- **Production Quality** ✅ - Strategy Pattern, structured logging, Unix philosophy
- **TUI Ready** 🔄 - Same search engine for TUI implementation (Phase 6-7)

## Installation

### Quick Development Setup (Recommended)

```bash
# 1. Clone the repository
git clone https://github.com/ktnyt/fae.git
cd fae

# 2. Install mise (if not already installed)
# macOS: brew install mise
# Other: https://mise.jdx.dev/getting-started.html

# 3. Set up complete development environment
mise install    # Installs Rust + cargo tools
mise run setup  # Builds project + runs tests

# 4. Start development workflow (optional)
mise run dev    # Starts file watching with auto-format + coverage
```

### Manual Installation

```bash
# Development installation with Rust
git clone https://github.com/ktnyt/fae.git
cd fae
cargo build --release
cargo install --path .
```

## Usage

### CLI Commands (All Features Implemented)

```bash
# Development commands (with mise)
mise run setup      # Complete environment setup
mise run dev        # Start file watching workflow
mise run test       # Run tests with coverage
mise run clean      # Clean build artifacts

# Or use make directly
make help           # Show all available commands
make watch-dev      # Watch files + auto-format + coverage
make test-coverage  # Generate coverage report

# Basic usage
fae "search_query"           # Content search (default)
fae "#function_name"         # Symbol search
fae ">file_name"             # File search
fae "/regex_pattern"         # Regex search

# Options
fae "search" --heading       # TTY format (with file headers)
fae "search" | head -10      # Pipeline support
fae --index                  # Build index and show symbol info
fae --backends               # Show external backend info

# Environment variables
RUST_LOG=debug fae "search"  # Debug logging
```

### Library API (Rust)

```rust
use fae::{SearchCoordinator, IndexManager};

// Index building and symbol search
let mut coordinator = SearchCoordinator::new(project_root)?;
let result = coordinator.build_index()?;
let hits = coordinator.search_symbols("handleClick", 10);
```

### Performance Characteristics (Measured)

- **Index Building**: 75.60ms (49 files, 421 symbols)
- **Content Search**: 70-167ms (external backend)
- **Symbol Search**: 393-603ms (Tree-sitter based)
- **Memory Usage**: <100MB (typical projects)
- **External Backends**: ripgrep → ag → built-in fallback

### Search Examples

```bash
# Symbol search: symbols containing "handle"
#handle

# File search: files containing "component"
>component

# Regex search: import statements
/^import.*from

# Content search: "error" in file contents
error
```

## Implementation Status

### ✅ Completed Features (Phase 1-5)

- **4 Search Modes**: Content, Symbol (#), File (>), Regex (/) fully implemented
- **Tree-sitter Integration**: 4 languages, unified query optimization, parallel processing
- **External Backend Integration**: ripgrep/ag support, auto-detection, fallback
- **Streaming Search**: ag/rg-style real-time output, pipeline support
- **Strategy Pattern CLI**: TUI-ready architecture, search mode separation
- **Comprehensive Quality Assurance**: 128 tests (CLI, E2E, performance, error handling)
- **Structured Logging**: RUST_LOG environment variable, debug support

### 🔄 Next Phase (Phase 6-7)

- **TUI Implementation**: ratatui-based real-time search, keyboard navigation
- **Git Integration**: Changed file detection, branch information
- **File Watching**: Real-time index updates, notify integration

### Supported Languages

- **TypeScript** (`.ts`, `.tsx`) ✅ - Interface, Class, Function, Method, Constant
- **JavaScript** (`.js`, `.jsx`) ✅ - Class, Function, Method, ArrowFunction, Constant
- **Python** (`.py`) ✅ - Class, Function, Assignment
- **Rust** (`.rs`) ✅ - Struct, Enum, Function, Const

## Design Philosophy

- **Simplicity First**: Clear, maintainable design avoiding unnecessary complexity
- **Streaming First**: Real-time search result output in ag/rg style
- **Unix Philosophy**: Do one thing well, support pipeline composition
- **Strategy Pattern**: Search mode separation, TUI/CLI reusability
- **External Backend Utilization**: Performance optimization via ripgrep/ag integration
- **Test-Driven Development**: Comprehensive quality assurance with 128 tests

## Exclusions

- Binary files
- Files listed in `.gitignore`
- Large files over 1MB
- Common exclusion directories (`node_modules/`, `target/`, `.git/`, etc.)

## Development & Contributing

For detailed technical specifications and development information, see:

- [Architecture](./docs/ARCHITECTURE.md) - System design and data structures
- [Development Guide](./docs/DEVELOPMENT.md) - Development phases and testing strategy
- [Design Document](./docs/DESIGN.md) - Overview design document

## License

[MIT License](./LICENSE)

---

_Discover code like a fairy - light, magical, and elegant_
