# Tree-sitter Language Support Checklist

This document tracks the current status and planned support for programming languages in fae using tree-sitter parsers.

## Currently Supported Languages ✅

- [x] **Rust** (`tree-sitter-rust`)
  - Status: ✅ Fully implemented
  - Symbols: Functions, Structs, Enums, Impl blocks, Constants, Static variables, Type aliases, Modules, Fields, Variables, Parameters
  - File extensions: `.rs`
  - Cargo dependency: `tree-sitter-rust = "0.20"`

- [x] **JavaScript** (`tree-sitter-javascript`)
  - Status: ✅ Fully implemented
  - Symbols: Functions (declarations, expressions, arrow functions), Classes, Variables (let/var/const), Parameters, Modules (imports)
  - File extensions: `.js`, `.mjs`, `.cjs`
  - Cargo dependency: `tree-sitter-javascript = "0.20"`
  - Features: Named function expressions, arrow functions, ES6 imports, default imports

## High Priority Languages 🎯

### Web Development
- [ ] **TypeScript** (`tree-sitter-typescript`)
  - Symbols: Functions, Classes, Interfaces, Types, Variables, Constants, Methods, Properties
  - File extensions: `.ts`, `.tsx`
  - Cargo dependency: `tree-sitter-typescript = "0.20"`

- [ ] **Python** (`tree-sitter-python`)
  - Symbols: Functions, Classes, Variables, Methods, Properties, Imports
  - File extensions: `.py`, `.pyw`
  - Cargo dependency: `tree-sitter-python = "0.20"`

### Systems Programming
- [ ] **C** (`tree-sitter-c`)
  - Symbols: Functions, Structs, Unions, Enums, Variables, Typedefs, Macros
  - File extensions: `.c`, `.h`
  - Cargo dependency: `tree-sitter-c = "0.20"`

- [ ] **C++** (`tree-sitter-cpp`)
  - Symbols: Functions, Classes, Structs, Namespaces, Variables, Methods, Templates
  - File extensions: `.cpp`, `.cxx`, `.cc`, `.hpp`, `.hxx`, `.hh`
  - Cargo dependency: `tree-sitter-cpp = "0.20"`

- [ ] **Go** (`tree-sitter-go`)
  - Symbols: Functions, Types, Variables, Constants, Methods, Interfaces, Packages
  - File extensions: `.go`
  - Cargo dependency: `tree-sitter-go = "0.20"`

## Medium Priority Languages 📝

### Popular Languages
- [ ] **Java** (`tree-sitter-java`)
  - Symbols: Classes, Methods, Fields, Interfaces, Enums, Packages
  - File extensions: `.java`
  - Cargo dependency: `tree-sitter-java = "0.20"`

- [ ] **C#** (`tree-sitter-c-sharp`)
  - Symbols: Classes, Methods, Properties, Fields, Interfaces, Namespaces
  - File extensions: `.cs`
  - Cargo dependency: `tree-sitter-c-sharp = "0.20"`

- [ ] **PHP** (`tree-sitter-php`)
  - Symbols: Functions, Classes, Methods, Properties, Variables, Constants
  - File extensions: `.php`, `.phtml`
  - Cargo dependency: `tree-sitter-php = "0.20"`

- [ ] **Ruby** (`tree-sitter-ruby`)
  - Symbols: Classes, Methods, Modules, Constants, Variables
  - File extensions: `.rb`, `.rake`, `.gemspec`
  - Cargo dependency: `tree-sitter-ruby = "0.20"`

### Functional Languages
- [ ] **Haskell** (`tree-sitter-haskell`)
  - Symbols: Functions, Types, Classes, Instances, Modules
  - File extensions: `.hs`, `.lhs`
  - Cargo dependency: `tree-sitter-haskell = "0.20"`

- [ ] **OCaml** (`tree-sitter-ocaml`)
  - Symbols: Functions, Types, Modules, Values, Classes
  - File extensions: `.ml`, `.mli`
  - Cargo dependency: `tree-sitter-ocaml = "0.20"`

- [ ] **Elixir** (`tree-sitter-elixir`)
  - Symbols: Functions, Modules, Structs, Protocols, Defmacros
  - File extensions: `.ex`, `.exs`
  - Cargo dependency: `tree-sitter-elixir = "0.20"`

### Mobile Development
- [ ] **Swift** (`tree-sitter-swift`)
  - Symbols: Functions, Classes, Structs, Protocols, Extensions, Variables
  - File extensions: `.swift`
  - Cargo dependency: `tree-sitter-swift = "0.20"`

- [ ] **Kotlin** (`tree-sitter-kotlin`)
  - Symbols: Functions, Classes, Objects, Interfaces, Properties, Variables
  - File extensions: `.kt`, `.kts`
  - Cargo dependency: `tree-sitter-kotlin = "0.20"`

## Lower Priority Languages 📋

### Scripting & Automation
- [ ] **Bash** (`tree-sitter-bash`)
  - Symbols: Functions, Variables, Commands
  - File extensions: `.sh`, `.bash`, `.zsh`
  - Cargo dependency: `tree-sitter-bash = "0.20"`

- [ ] **PowerShell** (`tree-sitter-powershell`)
  - Symbols: Functions, Variables, Classes, Commands
  - File extensions: `.ps1`, `.psm1`, `.psd1`
  - Cargo dependency: `tree-sitter-powershell = "0.20"`

- [ ] **Lua** (`tree-sitter-lua`)
  - Symbols: Functions, Variables, Tables, Modules
  - File extensions: `.lua`
  - Cargo dependency: `tree-sitter-lua = "0.20"`

- [ ] **Perl** (`tree-sitter-perl`)
  - Symbols: Subroutines, Variables, Packages, Modules
  - File extensions: `.pl`, `.pm`, `.pod`
  - Cargo dependency: `tree-sitter-perl = "0.20"`

### Data & Configuration
- [ ] **SQL** (`tree-sitter-sql`)
  - Symbols: Tables, Functions, Procedures, Views, Triggers
  - File extensions: `.sql`
  - Cargo dependency: `tree-sitter-sql = "0.20"`

- [ ] **YAML** (`tree-sitter-yaml`)
  - Symbols: Keys, Values, Lists, Objects
  - File extensions: `.yml`, `.yaml`
  - Cargo dependency: `tree-sitter-yaml = "0.20"`

- [ ] **TOML** (`tree-sitter-toml`)
  - Symbols: Tables, Keys, Values, Arrays
  - File extensions: `.toml`
  - Cargo dependency: `tree-sitter-toml = "0.20"`

- [ ] **JSON** (`tree-sitter-json`)
  - Symbols: Objects, Arrays, Properties
  - File extensions: `.json`, `.jsonc`
  - Cargo dependency: `tree-sitter-json = "0.20"`

### Markup & Documentation
- [ ] **HTML** (`tree-sitter-html`)
  - Symbols: Elements, IDs, Classes, Attributes
  - File extensions: `.html`, `.htm`
  - Cargo dependency: `tree-sitter-html = "0.20"`

- [ ] **CSS** (`tree-sitter-css`)
  - Symbols: Selectors, Properties, Classes, IDs, Functions
  - File extensions: `.css`
  - Cargo dependency: `tree-sitter-css = "0.20"`

- [ ] **Markdown** (`tree-sitter-markdown`)
  - Symbols: Headers, Links, Code blocks, Lists
  - File extensions: `.md`, `.markdown`
  - Cargo dependency: `tree-sitter-markdown = "0.20"`

### Specialized Languages
- [ ] **R** (`tree-sitter-r`)
  - Symbols: Functions, Variables, Objects, Classes
  - File extensions: `.r`, `.R`
  - Cargo dependency: `tree-sitter-r = "0.20"`

- [ ] **Scala** (`tree-sitter-scala`)
  - Symbols: Functions, Classes, Objects, Traits, Variables
  - File extensions: `.scala`, `.sc`
  - Cargo dependency: `tree-sitter-scala = "0.20"`

- [ ] **Clojure** (`tree-sitter-clojure`)
  - Symbols: Functions, Macros, Variables, Namespaces
  - File extensions: `.clj`, `.cljs`, `.cljc`
  - Cargo dependency: `tree-sitter-clojure = "0.20"`

- [ ] **Vim Script** (`tree-sitter-vim`)
  - Symbols: Functions, Variables, Commands, Mappings
  - File extensions: `.vim`, `.vimrc`
  - Cargo dependency: `tree-sitter-vim = "0.20"`

- [ ] **Nix** (`tree-sitter-nix`)
  - Symbols: Functions, Variables, Attributes, Sets
  - File extensions: `.nix`
  - Cargo dependency: `tree-sitter-nix = "0.20"`

- [ ] **Zig** (`tree-sitter-zig`)
  - Symbols: Functions, Structs, Unions, Enums, Variables
  - File extensions: `.zig`
  - Cargo dependency: `tree-sitter-zig = "0.20"`

## Implementation Notes

### Query Pattern Examples

Each language requires specific tree-sitter queries to extract symbols. Here are some common patterns:

**Function definitions:**
```scheme
(function_declaration
  name: (identifier) @function.name) @function.definition
```

**Class definitions:**
```scheme
(class_declaration
  name: (identifier) @class.name) @class.definition
```

**Variable declarations:**
```scheme
(variable_declaration
  name: (identifier) @variable.name) @variable.definition
```

**JavaScript-specific examples:**
```scheme
; Arrow functions assigned to variables
(variable_declarator
  name: (identifier) @function.name
  value: (arrow_function))

; Import specifiers
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @module.name))))
```

### File Extension Mapping

The language detection system should map file extensions to appropriate tree-sitter parsers. Multiple extensions per language are supported.

### Symbol Type Mapping

Each language's symbols should be mapped to fae's `SymbolType` enum:
- `Function` - Functions, methods, procedures
- `Class` - Classes, structs, objects
- `Variable` - Variables, constants, properties
- `Type` - Type definitions, interfaces, traits
- `Module` - Modules, namespaces, packages
- `Constant` - Constants, static values
- `Interface` - Interfaces, protocols, traits
- `Enum` - Enumerations, variants
- `Field` - Struct/class fields, properties
- `Method` - Class/object methods
- `Parameter` - Function/method parameters

## Implementation Priority

1. **Phase 1** (High Priority): ✅ JavaScript, TypeScript, Python, C, C++, Go
2. **Phase 2** (Medium Priority): Java, C#, PHP, Ruby, Swift, Kotlin
3. **Phase 3** (Lower Priority): Remaining languages based on user demand

## Status Legend

- ✅ **Fully implemented** - Language support with comprehensive symbol extraction
- 🚧 **In progress** - Currently being implemented
- 📋 **Planned** - Scheduled for future implementation
- ❓ **Under consideration** - May be implemented based on demand

---

**Last updated:** 2025-06-17
**Total languages:** 2 implemented, 36+ planned