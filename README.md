# sfs - Symbol Fuzzy Search

A blazingly fast fuzzy search tool for code symbols (functions, classes, variables, etc.) across your codebase. Written in Rust for maximum performance and portability.

## Features

- 🔍 **Fuzzy Search**: Find symbols quickly with fuzzy matching
- 🖥️ **Interactive TUI**: Beautiful terminal user interface with real-time search
- 📋 **Clipboard Integration**: Copy symbol locations with Enter key
- 🚀 **Lightning Fast**: Native Rust binary with concurrent processing
- 📁 **Multi-language**: Supports multiple languages through Tree-sitter:
  - Web: TypeScript, JavaScript, PHP
  - Systems: Rust, Go, C, C++
  - JVM: Java, Scala
  - Others: Python, Ruby, C#
- 🎯 **Smart Filtering**: Filter by symbol type with intelligent deduplication
- 🔄 **Multiple Search Modes**: Fuzzy, Symbol-only, File-only, and Regex search
- 🎨 **User-friendly**: Color-coded results with intuitive navigation
- 🚫 **Gitignore Support**: Respects .gitignore files by default
- ⚡ **Progressive Indexing**: Real-time indexing with progress display
- 🔒 **Robust Error Handling**: Comprehensive error management and recovery

## Installation

### Prerequisites

- Rust 1.85.1 or later
- Cargo (comes with Rust)
- Git
- C compiler (for Tree-sitter)
  - gcc/clang on Unix-like systems
  - MSVC on Windows

### From Source (Recommended)

```bash
# Clone repository
git clone https://github.com/ktnyt/sfs
cd sfs

# Build and install
cargo install --path .

# Verify installation
sfs --version
```

### Development Setup

```bash
# Clone repository
git clone https://github.com/ktnyt/sfs
cd sfs

# Install development dependencies
cargo install cargo-watch  # For auto-recompilation
cargo install cargo-audit # For security auditing
cargo install cargo-tarpaulin # For code coverage

# Build debug version
cargo build

# Watch for changes and rebuild
cargo watch -x build

# Run tests with coverage
cargo tarpaulin
```

### Platform-specific Notes

#### Linux

```bash
# Install required dependencies
sudo apt-get update
sudo apt-get install build-essential pkg-config libx11-dev libxcb1-dev
```

#### macOS

```bash
# Install required dependencies
brew install pkg-config
```

#### Windows

- Install Visual Studio Build Tools with C++ support
- Install Git for Windows
- Install Rust using rustup-init.exe

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Usage

### TUI Mode (Interactive)

```bash
# Start interactive mode
sfs

# Start interactive mode in specific directory
sfs -d ./src
```

**TUI Controls:**

- Type to search symbols in real-time
- `↑/↓` or `Ctrl+p/n`: Navigate results
- `Enter`: Copy symbol location to clipboard and clear search
- `Esc`: Exit application
- `F1` or `Ctrl+h`: Show help

**Search Modes:**

- **Fuzzy** (default): `query` - Fuzzy search across all symbols
- **Symbol**: `#query` - Search only symbol names (exclude files/dirs)
- **File**: `>query` - Search only file and directory names
- **Regex**: `/query` - Regular expression search

### CLI Mode

```bash
# Search for symbols containing "function"
sfs "function"

# Search in a specific directory
sfs "Component" -d ./src

# Limit results
sfs "parse" -l 10

# Filter by symbol types
sfs "handler" --types function

# Adjust fuzzy matching threshold (0-1, lower is more fuzzy)
sfs "router" --threshold 0.2
```

### Options

- `-d, --directory <path>`: Directory to search (default: current directory)
- `-t, --types <types>`: Symbol types to include (comma-separated)
  - `function`: Functions and methods
  - `variable`: Variables and fields
  - `class`: Class declarations
  - `interface`: Interface declarations
  - `type`: Type aliases and definitions
  - `enum`: Enumeration declarations
  - `constant`: Constants and immutable values
  - `method`: Class and object methods
  - `property`: Object properties
  - `filename`: File names
  - `dirname`: Directory names
- `--no-files`: Exclude filenames from search
- `--no-dirs`: Exclude directory names from search
- `-l, --limit <number>`: Maximum number of results (default: 10)
- `--threshold <number>`: Fuzzy search threshold 0-1 (default: 0.5)
- `--tui`: Force TUI mode (default when no query provided)
- `-v, --verbose`: Enable verbose output with progress information
- `--include-ignored`: Include files normally ignored by .gitignore

### TUI Mode Examples

```bash
# Interactive search with real-time results
sfs

# Start in specific directory with verbose output
sfs -d ./src -v

# In TUI, try these searches:
# - "user" - fuzzy search for user-related symbols
# - "#Component" - find only Component symbols (no files)
# - ">index" - find only files/dirs named index
# - "/^get.*" - regex search for symbols starting with "get"
# - "@class" - search only for class declarations
# - "!constant" - search only for constants
```

### CLI Mode Examples

```bash
# Find all functions with progress display
sfs "handler" -v --types function

# Search for TypeScript interfaces and types
sfs "User" --types interface,type

# Search with high fuzzy threshold (more exact matches)
sfs "handleClick" --threshold 0.8 --types method

# Find classes with limit
sfs "Service" --types class -l 5

# Search including ignored files
sfs "config" --include-ignored

# Search for enums and constants
sfs "Status" --types enum,constant

# Complex multi-type search
sfs "user" --types class,interface,method --threshold 0.6

# Search in specific directory excluding files
sfs "api" -d ./src/services --no-files
```

### Development

```bash
# Run all tests
cargo test

# Run specific test categories
cargo test --test indexer_test
cargo test --test searcher_test
cargo test --test tui_test

# Run performance tests
cargo bench

# Build with all optimizations
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run

# Format and lint
cargo fmt
cargo clippy

# Run security tests
cargo test --test security_test

# Test real-world scenarios
cargo test --test real_world_scenarios_test
```

## Technical Details

- **Parser**: Tree-sitter based symbol extraction for accuracy and speed
- **Search**: Uses `fuzzy-matcher` crate with optimized regex compilation
- **TUI**: Built with `ratatui` for beautiful terminal interface
- **Clipboard**: Cross-platform clipboard support with `arboard`
- **Performance**:
  - Concurrent file processing with Rayon and Tokio
  - Progressive indexing with status display
  - Optimized regex compilation (3300x performance improvement)
  - Smart deduplication for cleaner results
- **Testing**:
  - Comprehensive test suite with mockall and serial_test
  - Performance benchmarks using criterion
  - Real-world scenario testing
  - Security and error handling coverage
