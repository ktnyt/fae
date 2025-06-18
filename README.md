# fae - Fast And Elegant code search

[![CI](https://github.com/ktnyt/fae/workflows/CI/badge.svg)](https://github.com/ktnyt/fae/actions)
[![Coverage](https://codecov.io/gh/ktnyt/fae/branch/main/graph/badge.svg)](https://codecov.io/gh/ktnyt/fae)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

High-performance code search tool with beautiful TUI and real-time capabilities

## Overview

**fae** is a modern code search tool that combines blazing-fast symbol indexing with an intuitive Terminal User Interface (TUI). Built with Rust and powered by Tree-sitter, it provides real-time multi-modal search across large codebases with professional-grade performance and user experience.

> **✅ Development Status**: Phase 8 complete - Full TUI implementation with Actor system, symbol indexing progress display, and polished UX. Production ready with 168+ comprehensive tests.

## Features

### 🔍 Multi-Modal Search

- **Content Search** (default) - Blazing-fast full-text search powered by ripgrep/ag
- **Symbol Search** (`#prefix`) - Tree-sitter-based function, class, variable discovery
- **Variable Search** (`$prefix`) - Focused variable and constant search
- **File Search** (`>prefix`) - File names and paths with fuzzy matching
- **Regex Search** (`/prefix`) - Advanced pattern matching with full regex support

### ✨ Key Features

- **🎨 Beautiful TUI** ✅ - Modern terminal interface with real-time search and progress display
- **⚡ Actor-Based Architecture** ✅ - Concurrent search processing with unified message passing
- **🌳 Tree-sitter Integration** ✅ - Advanced symbol extraction for 4+ languages
- **🚀 High-Performance Indexing** ✅ - Parallel processing with ~70,000 symbols/second
- **🎯 Smart Caching** ✅ - 281x speedup with intelligent file content caching
- **🔧 External Backend Integration** ✅ - ripgrep/ag support with graceful fallback
- **📊 Real-time Progress Display** ✅ - Visual indexing progress with file counts and statistics
- **⌨️ Polished UX** ✅ - Bidirectional Tab navigation, elegant cursor, adaptive UI sizing
- **🧪 Comprehensive Testing** ✅ - 168+ tests covering Actor integration, TUI workflows, edge cases
- **📝 Smart Logging** ✅ - Session-based logging with automatic cleanup

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

### 🎨 TUI Mode (Primary Interface)

Simply run `fae` without arguments to launch the beautiful TUI:

```bash
fae  # Launch interactive TUI
```

**TUI Features:**
- **Real-time search** - Results update as you type
- **Multi-modal switching** - Tab/Shift+Tab to cycle between search modes
- **Visual progress** - See symbol indexing progress with file counts
- **Keyboard navigation** - Arrow keys, Enter to copy, Esc to quit
- **Smart cursor** - Background-highlighted cursor that doesn't disrupt layout
- **Adaptive UI** - Auto-sizing toasts and responsive layout

**Key Bindings:**
- `Tab` - Cycle search modes forward (Literal → Symbol(#) → Variable($) → File(>) → Regex(/))
- `Shift+Tab` - Cycle search modes backward (Literal ← Symbol(#) ← Variable($) ← File(>) ← Regex(/))
- `Enter` - Copy selected result to clipboard
- `Ctrl+C` / `Esc` - Exit
- `Ctrl+S` - Toggle statistics overlay
- `Ctrl+G` - Abort current search

### 💻 CLI Mode (Pipeline & Automation)

For scripting and pipeline integration:

```bash
# Direct search queries
fae "search_query"           # Content search (default)
fae "#function_name"         # Symbol search 
fae "$variable_name"         # Variable search
fae ">file_name"             # File search
fae "/regex_pattern"         # Regex search

# Pipeline support
fae "search" | head -10      # Limit results
fae "error" | grep -v test   # Filter results
```

### 🛠️ Development Commands

```bash
# Development setup (with mise)
mise run setup      # Complete environment setup
mise run dev        # Start file watching workflow
mise run test       # Run tests with coverage
mise run clean      # Clean build artifacts

# Or use make directly
make help           # Show all available commands
make test-coverage  # Generate coverage report
make dev            # Format, lint, test, coverage

# Environment variables
RUST_LOG=debug fae  # Debug logging (especially useful for TUI mode)
```

### Library API (Rust)

```rust
use fae::{SearchCoordinator, IndexManager};

// Index building and symbol search
let mut coordinator = SearchCoordinator::new(project_root)?;
let result = coordinator.build_index()?;
let hits = coordinator.search_symbols("handleClick", 10);
```

### ⚡ Performance Characteristics (Measured)

- **Symbol Indexing**: ~70,000 symbols/second with advanced caching (50% improvement)
- **Cache Performance**: 281x speedup for identical file content, 2.1x for language configs
- **Content Search**: <100ms (ripgrep/ag backends with graceful fallback)
- **TUI Responsiveness**: 16ms polling for real-time search updates
- **Memory Efficiency**: Streaming processing for large codebases
- **Backend Strategy**: ripgrep → ag → native (automatic detection and fallback)

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

## 🏗️ Implementation Status

### ✅ Completed Features (Phase 1-8)

**🎨 Full TUI Implementation**
- **Interactive Terminal Interface**: Beautiful ratatui-based UI with real-time search
- **Multi-modal Search Cycling**: Tab/Shift+Tab bidirectional mode switching
- **Visual Progress Display**: Real-time symbol indexing progress with file counts
- **Polished UX**: Background-highlighted cursor, adaptive toast sizing, smart keyboard shortcuts

**⚡ Actor-Based Architecture**  
- **Unified Search System**: Complete Actor system with message passing coordination
- **Concurrent Processing**: SymbolIndexActor, SymbolSearchActor, ResultHandlerActor coordination
- **File Watching**: Real-time change detection with WatchActor integration
- **Smart State Management**: Race condition prevention, graceful error handling

**🔍 Advanced Search Capabilities**
- **5 Search Modes**: Content, Symbol (#), Variable ($), File (>), Regex (/) 
- **Tree-sitter Integration**: 4+ languages with optimized S-expression queries
- **High-Performance Backends**: ripgrep/ag integration with intelligent fallback
- **Smart Caching**: 281x performance improvement with content-based caching

**🧪 Production Quality**
- **Comprehensive Testing**: 168+ tests covering Actor integration, TUI workflows, edge cases
- **Session-Based Logging**: Smart log management with automatic cleanup
- **Performance Optimization**: ~70,000 symbols/second indexing with memory efficiency

### 🚀 Future Enhancements

- **Git Integration**: Changed file detection, branch-aware search
- **Configuration System**: .fae.toml for project-specific settings  
- **Extended Language Support**: Additional Tree-sitter language integrations
- **Semantic Search**: Code context and relationship analysis

### Supported Languages

- **TypeScript** (`.ts`, `.tsx`) ✅ - Interface, Class, Function, Method, Constant
- **JavaScript** (`.js`, `.jsx`) ✅ - Class, Function, Method, ArrowFunction, Constant
- **Python** (`.py`) ✅ - Class, Function, Assignment
- **Rust** (`.rs`) ✅ - Struct, Enum, Function, Const

## 🎯 Design Philosophy

- **Real-time First**: Immediate visual feedback with progressive indexing and streaming search
- **Actor-Based Concurrency**: Message-driven architecture for responsive, race-condition-free operations
- **Performance Through Intelligence**: Smart caching, parallel processing, and optimized backend selection
- **User Experience Excellence**: Polished TUI with intuitive navigation and adaptive interface elements
- **Unix Philosophy Compatibility**: Excellent CLI mode for pipeline composition and automation
- **Test-Driven Quality**: 168+ comprehensive tests ensuring reliability and performance
- **Progressive Enhancement**: Graceful degradation from optimal (ripgrep) to fallback (native) backends

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
