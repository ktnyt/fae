# sfs - Symbol Fuzzy Search

A fast fuzzy search tool for code symbols (functions, classes, variables, etc.) across your codebase.

## Features

- 🔍 **Fuzzy Search**: Find symbols quickly with fuzzy matching
- 🌳 **Tree-sitter Support**: Enhanced parsing with Tree-sitter for 50+ languages
- 🚀 **Fast**: Lightning-fast search using optimized algorithms
- 📁 **Multi-language**: Supports TypeScript, JavaScript, Python, Rust, Go, and more
- 🎯 **Smart Filtering**: Filter by symbol type (function, class, interface, etc.)
- 💡 **Fallback Support**: Automatic fallback to regex parsing if Tree-sitter fails

## Installation

```bash
npm install -g sfs
```

## Usage

### Basic Search

```bash
# Search for symbols containing "function"
sfs "function"

# Search in a specific directory
sfs "Component" -d ./src

# Limit results
sfs "parse" -l 10
```

### Advanced Search

```bash
# Use Tree-sitter for enhanced parsing
sfs "class" --use-tree-sitter

# Filter by symbol types
sfs "handler" -t "function,method"

# Adjust fuzzy matching threshold (0-1, lower is more fuzzy)
sfs "router" --threshold 0.2

# Search specific file patterns
sfs "Config" --patterns "**/*.ts,**/*.tsx"
```

### Options

- `-d, --directory <path>`: Directory to search (default: current directory)
- `-t, --types <types>`: Symbol types to include (comma-separated)
- `--no-files`: Exclude filenames from search
- `--no-dirs`: Exclude directory names from search
- `-l, --limit <number>`: Maximum number of results (default: 50)
- `--threshold <number>`: Fuzzy search threshold 0-1 (default: 0.4)
- `--patterns <patterns>`: File patterns to include
- `--use-tree-sitter`: Use Tree-sitter for enhanced parsing

### Symbol Types

- `function`: Function declarations
- `class`: Class declarations  
- `interface`: Interface declarations (TypeScript)
- `type`: Type aliases (TypeScript)
- `variable`: Variable declarations
- `constant`: Constant declarations
- `method`: Class methods
- `property`: Object properties
- `filename`: File names
- `dirname`: Directory names

## Development

```bash
# Install dependencies
npm install

# Build
npm run build

# Run in development
npm run dev

# Type check
npm run typecheck

# Lint & format
npm run check
```

## License

MIT