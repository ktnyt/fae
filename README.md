# sfs - Symbol Fuzzy Search

A blazingly fast fuzzy search tool for code symbols (functions, classes, variables, etc.) across your codebase. Written in Rust for maximum performance and portability.

## Features

- 🔍 **Fuzzy Search**: Find symbols quickly with fuzzy matching
- 🖥️ **Interactive TUI**: Beautiful terminal user interface with real-time search
- 📋 **Clipboard Integration**: Copy symbol locations with Enter key
- 🚀 **Lightning Fast**: Native Rust binary for maximum performance
- 📁 **Multi-language**: Supports TypeScript, JavaScript, Python with regex-based parsing
- 🎯 **Smart Filtering**: Filter by symbol type (function, class, variable, etc.)
- 🔄 **Multiple Search Modes**: Fuzzy, Symbol-only, File-only, and Regex search
- 🎨 **User-friendly**: Color-coded results with intuitive navigation
- 🚫 **Gitignore Support**: Respects .gitignore files by default, with option to include all files

## Installation

### From Binary (Recommended)

```bash
# Build from source
git clone https://github.com/ktnyt/sfs
cd sfs
cargo build --release

# Binary will be available at target/release/sfs
```

### From Source

```bash
# Clone repository
git clone https://github.com/ktnyt/sfs
cd sfs

# Build with Cargo
cargo install --path .
```

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
- `--no-files`: Exclude filenames from search
- `--no-dirs`: Exclude directory names from search
- `-l, --limit <number>`: Maximum number of results (default: 50)
- `--threshold <number>`: Fuzzy search threshold 0-1 (default: 0.4)
- `--tui`: Force TUI mode (default when no query provided)
- `-v, --verbose`: Enable verbose output (detailed progress information)
- `--include-ignored`: Include files normally ignored by .gitignore

### Symbol Types

- `function`: Function declarations, methods, getters, setters
- `class`: Class declarations
- `interface`: Interface declarations (TypeScript)
- `type`: Type aliases (TypeScript)
- `variable`: Variable declarations and constants
- `filename`: File names
- `dirname`: Directory names

## Examples

### TUI Mode Examples
```bash
# Interactive search with real-time results
sfs

# Start in specific directory
sfs -d ./src/components

# In TUI, try these searches:
# - "user" - fuzzy search for user-related symbols
# - "#Component" - find only Component symbols (no files)
# - ">index" - find only files/dirs named index
# - "/^get.*" - regex search for symbols starting with "get"
```

### CLI Mode Examples
```bash
# Find all functions
sfs "" --types function

# Find TypeScript interfaces
sfs "User" --types interface

# Search with high fuzzy threshold (more exact)
sfs "handleClick" --threshold 0.8

# Limit to top 5 results
sfs "component" -l 5

# Search including files ignored by .gitignore
sfs "config" --include-ignored

# Verbose mode to see indexing progress
sfs "handler" -v
```

## Development

```bash
# Run tests
cargo test

# Build release version
cargo build --release

# Run in development mode
cargo run

# Run with debug output
RUST_LOG=debug cargo run

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Technical Details

- **Parser**: Regex-based symbol extraction for reliability
- **Search**: Uses `fuzzy-matcher` crate for fast fuzzy search
- **TUI**: Built with `ratatui` for beautiful terminal interface
- **Clipboard**: Cross-platform clipboard support with `arboard`
- **Performance**: Concurrent file processing and caching

## License

MIT