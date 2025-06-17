# Tree-sitter Language Support Checklist

This document tracks the current status and planned support for programming languages in fae using tree-sitter parsers.

## Currently Supported Languages ✅

- [x] **Rust** (`tree-sitter-rust`)
  - Status: ✅ Fully implemented
  - Symbols: Functions, Structs, Enums, Impl blocks, Constants, Static variables, Type aliases, Modules, Fields, Variables, Parameters
  - File extensions: `.rs`
  - Cargo dependency: `tree-sitter-rust = "0.20"`

- [x] **JavaScript** (`tree-sitter-javascript`)
  - Status: ✅ Fully implemented | 🏛️ Official tree-sitter parser (419⭐)
  - Symbols: Functions (declarations, expressions, arrow functions), Classes, Variables (let/var/const), Parameters, Modules (imports)
  - File extensions: `.js`, `.mjs`, `.cjs`
  - Repository: `tree-sitter/tree-sitter-javascript`
  - ABI: Version 14+
  - Features: Named function expressions, arrow functions, ES6 imports, default imports

## High Priority Languages 🎯

### Web Development (Official Tree-sitter Parsers)
- [ ] **TypeScript** (`tree-sitter-typescript`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Classes, Interfaces, Types, Variables, Constants, Methods, Properties
  - File extensions: `.ts`, `.tsx`
  - Repository: `tree-sitter/tree-sitter-typescript`
  - ABI: Version 14+

- [ ] **Python** (`tree-sitter-python`)
  - Status: 🏛️ Official tree-sitter parser (446⭐)
  - Symbols: Functions, Classes, Variables, Methods, Properties, Imports, Decorators
  - File extensions: `.py`, `.pyw`
  - Repository: `tree-sitter/tree-sitter-python`
  - ABI: Version 14+

### Core Systems Languages (Official)
- [ ] **C** (`tree-sitter-c`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Structs, Unions, Enums, Variables, Typedefs, Macros
  - File extensions: `.c`, `.h`
  - Repository: `tree-sitter/tree-sitter-c`
  - ABI: Version 14+

- [ ] **C++** (`tree-sitter-cpp`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Classes, Structs, Namespaces, Variables, Methods, Templates
  - File extensions: `.cpp`, `.cxx`, `.cc`, `.hpp`, `.hxx`, `.hh`
  - Repository: `tree-sitter/tree-sitter-cpp`
  - ABI: Version 14+

- [ ] **Go** (`tree-sitter-go`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Types, Variables, Constants, Methods, Interfaces, Packages
  - File extensions: `.go`
  - Repository: `tree-sitter/tree-sitter-go`
  - ABI: Version 14+

## Medium Priority Languages 📝

### Web Frameworks & Component Languages (Community Maintained)
- [ ] **Vue** (`tree-sitter-vue`)
  - Status: 👥 Community maintained
  - Symbols: Vue components, Functions, Variables, Props, Computed, Methods, Data
  - File extensions: `.vue`
  - Repository: Community maintained
  - Features: Single File Component parsing, template/script/style separation

- [ ] **Svelte** (`tree-sitter-svelte`)
  - Status: 👥 Community maintained
  - Symbols: Svelte components, Functions, Variables, Props, Stores, Actions
  - File extensions: `.svelte`
  - Repository: Community maintained
  - Features: Reactive declarations, component props, event handlers

### Popular Languages (Official & Well-Maintained)
- [ ] **Java** (`tree-sitter-java`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Classes, Methods, Fields, Interfaces, Enums, Packages, Annotations
  - File extensions: `.java`
  - Repository: `tree-sitter/tree-sitter-java`
  - ABI: Version 14+

- [ ] **PHP** (`tree-sitter-php`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Classes, Methods, Properties, Variables, Constants, Namespaces
  - File extensions: `.php`, `.phtml`
  - Repository: `tree-sitter/tree-sitter-php`
  - ABI: Version 14+

- [ ] **Ruby** (`tree-sitter-ruby`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Classes, Methods, Modules, Constants, Variables, Blocks
  - File extensions: `.rb`, `.rake`, `.gemspec`
  - Repository: `tree-sitter/tree-sitter-ruby`
  - ABI: Version 14+

- [ ] **Scala** (`tree-sitter-scala`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Classes, Objects, Traits, Variables, Types
  - File extensions: `.scala`, `.sc`
  - Repository: `tree-sitter/tree-sitter-scala`
  - ABI: Version 14+

### Functional Languages (Official)
- [ ] **Haskell** (`tree-sitter-haskell`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Types, Classes, Instances, Modules, Data constructors
  - File extensions: `.hs`, `.lhs`
  - Repository: `tree-sitter/tree-sitter-haskell`
  - ABI: Version 14+

- [ ] **OCaml** (`tree-sitter-ocaml`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Types, Modules, Values, Classes, Variants
  - File extensions: `.ml`, `.mli`
  - Repository: `tree-sitter/tree-sitter-ocaml`
  - ABI: Version 14+

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

### Scripting & Automation (Official)
- [ ] **Bash** (`tree-sitter-bash`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Functions, Variables, Commands, Aliases
  - File extensions: `.sh`, `.bash`, `.zsh`
  - Repository: `tree-sitter/tree-sitter-bash`
  - ABI: Version 14+

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

### Data & Configuration (Official)
- [ ] **JSON** (`tree-sitter-json`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Objects, Arrays, Properties, Values
  - File extensions: `.json`, `.jsonc`
  - Repository: `tree-sitter/tree-sitter-json`
  - ABI: Version 14+

- [ ] **YAML** (`tree-sitter-yaml`)
  - Status: 👥 Community maintained (High quality)
  - Symbols: Keys, Values, Lists, Objects, Anchors
  - File extensions: `.yml`, `.yaml`
  - Repository: Community maintained
  - ABI: Version 14+

- [ ] **TOML** (`tree-sitter-toml`)
  - Status: 👥 Community maintained (High quality)
  - Symbols: Tables, Keys, Values, Arrays
  - File extensions: `.toml`
  - Repository: Community maintained
  - ABI: Version 14+

### Markup & Documentation (Official)
- [ ] **HTML** (`tree-sitter-html`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Elements, IDs, Classes, Attributes, Tags
  - File extensions: `.html`, `.htm`
  - Repository: `tree-sitter/tree-sitter-html`
  - ABI: Version 14+

- [ ] **CSS** (`tree-sitter-css`)
  - Status: 🏛️ Official tree-sitter parser
  - Symbols: Selectors, Properties, Classes, IDs, Functions, Rules
  - File extensions: `.css`
  - Repository: `tree-sitter/tree-sitter-css`
  - ABI: Version 14+

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

**Frontend Framework examples:**
```scheme
; JSX/TSX - React functional components
(function_declaration
  name: (identifier) @function.name
  body: (statement_block
    (return_statement
      (jsx_element) @component.jsx)))

; Vue - Single File Component script
(export_statement
  declaration: (object_expression
    (property
      key: (identifier) @component.property
      value: (function_expression))))

; Svelte - Component script variables
(variable_declaration
  (variable_declarator
    name: (identifier) @variable.reactive))
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

1. **Phase 1** (High Priority - Official Parsers): ✅ JavaScript, TypeScript, Python, C, C++, Go
2. **Phase 2** (Medium Priority - Official Parsers): Java, PHP, Ruby, Scala, Haskell, OCaml
3. **Phase 3** (Web Frameworks - Community): Vue, Svelte, React JSX/TSX
4. **Phase 4** (Lower Priority): Remaining languages based on user demand

### Priority Rationale
- **Official parsers** are prioritized for stability and long-term maintenance
- **High-star repositories** (300+ stars) indicate strong community adoption
- **ABI Version 14+** ensures compatibility with modern tree-sitter versions

## Status Legend

### Implementation Status
- ✅ **Fully implemented** - Language support with comprehensive symbol extraction
- 🚧 **In progress** - Currently being implemented
- 📋 **Planned** - Scheduled for future implementation
- ❓ **Under consideration** - May be implemented based on demand

### Parser Maintenance Status
- 🏛️ **Official tree-sitter parser** - Maintained by tree-sitter organization
- 👥 **Community maintained** - High-quality community parsers
- ⚠️ **Experimental** - Early stage or less stable parsers

---

**Last updated:** 2025-06-17
**Total languages:** 2 implemented, 41+ planned