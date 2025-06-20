#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Literal,
    Regexp,
    Filepath, // File path/name search mode
    Symbol,   // Symbol/function name search mode (excluding variables/constants)
    Variable, // Variable and constant search mode
}

#[derive(Debug, Clone)]
pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub filename: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
}

/// Type of symbol extracted by tree-sitter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Variable,
    Constant,
    Module,
    Type,
    Field,
    Parameter,
}

impl SymbolType {
    /// Get a human-readable display name for the symbol type
    pub fn display_name(&self) -> &'static str {
        match self {
            SymbolType::Function => "fn",
            SymbolType::Method => "method",
            SymbolType::Class => "class",
            SymbolType::Struct => "struct",
            SymbolType::Enum => "enum",
            SymbolType::Interface => "interface",
            SymbolType::Variable => "var",
            SymbolType::Constant => "const",
            SymbolType::Module => "mod",
            SymbolType::Type => "type",
            SymbolType::Field => "field",
            SymbolType::Parameter => "param",
        }
    }
}

/// Symbol extracted from source code using tree-sitter
#[derive(Debug, Clone)]
pub struct Symbol {
    pub filepath: String,
    pub line: u32,
    pub column: u32,
    pub name: String,
    pub content: String,
    pub symbol_type: SymbolType,
}

impl Symbol {
    /// Create a new Symbol
    pub fn new(
        filepath: String,
        line: u32,
        column: u32,
        name: String,
        content: String,
        symbol_type: SymbolType,
    ) -> Self {
        Self {
            filepath,
            line,
            column,
            name,
            content,
            symbol_type,
        }
    }

    /// Convert this Symbol into a SearchResult for compatibility
    pub fn into_search_result(self) -> SearchResult {
        SearchResult {
            filename: self.filepath,
            line: self.line,
            column: self.column,
            content: format!("[{}] {}", self.symbol_type.display_name(), self.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_mode_variants() {
        // Test all SearchMode variants
        assert_eq!(SearchMode::Literal, SearchMode::Literal);
        assert_eq!(SearchMode::Regexp, SearchMode::Regexp);
        assert_eq!(SearchMode::Filepath, SearchMode::Filepath);
        assert_eq!(SearchMode::Symbol, SearchMode::Symbol);
        assert_eq!(SearchMode::Variable, SearchMode::Variable);

        // Test different variants are not equal
        assert_ne!(SearchMode::Literal, SearchMode::Regexp);
        assert_ne!(SearchMode::Symbol, SearchMode::Variable);
    }

    #[test]
    fn test_search_params_creation() {
        let params = SearchParams {
            query: "test_query".to_string(),
            mode: SearchMode::Symbol,
        };

        assert_eq!(params.query, "test_query");
        assert_eq!(params.mode, SearchMode::Symbol);
    }

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            filename: "test.rs".to_string(),
            line: 42,
            column: 10,
            content: "fn test_function()".to_string(),
        };

        assert_eq!(result.filename, "test.rs");
        assert_eq!(result.line, 42);
        assert_eq!(result.column, 10);
        assert_eq!(result.content, "fn test_function()");
    }

    #[test]
    fn test_symbol_type_display_names() {
        // Test all SymbolType display names
        assert_eq!(SymbolType::Function.display_name(), "fn");
        assert_eq!(SymbolType::Method.display_name(), "method");
        assert_eq!(SymbolType::Class.display_name(), "class");
        assert_eq!(SymbolType::Struct.display_name(), "struct");
        assert_eq!(SymbolType::Enum.display_name(), "enum");
        assert_eq!(SymbolType::Interface.display_name(), "interface");
        assert_eq!(SymbolType::Variable.display_name(), "var");
        assert_eq!(SymbolType::Constant.display_name(), "const");
        assert_eq!(SymbolType::Module.display_name(), "mod");
        assert_eq!(SymbolType::Type.display_name(), "type");
        assert_eq!(SymbolType::Field.display_name(), "field");
        assert_eq!(SymbolType::Parameter.display_name(), "param");
    }

    #[test]
    fn test_symbol_new() {
        let symbol = Symbol::new(
            "src/main.rs".to_string(),
            15,
            8,
            "main".to_string(),
            "fn main() { ... }".to_string(),
            SymbolType::Function,
        );

        assert_eq!(symbol.filepath, "src/main.rs");
        assert_eq!(symbol.line, 15);
        assert_eq!(symbol.column, 8);
        assert_eq!(symbol.name, "main");
        assert_eq!(symbol.content, "fn main() { ... }");
        assert_eq!(symbol.symbol_type, SymbolType::Function);
    }

    #[test]
    fn test_symbol_into_search_result() {
        let symbol = Symbol::new(
            "src/lib.rs".to_string(),
            25,
            4,
            "MyStruct".to_string(),
            "struct MyStruct { ... }".to_string(),
            SymbolType::Struct,
        );

        let result = symbol.into_search_result();

        assert_eq!(result.filename, "src/lib.rs");
        assert_eq!(result.line, 25);
        assert_eq!(result.column, 4);
        assert_eq!(result.content, "[struct] MyStruct");
    }

    #[test]
    fn test_symbol_into_search_result_different_types() {
        // Test various symbol types in search result conversion
        let function_symbol = Symbol::new(
            "test.rs".to_string(),
            1,
            1,
            "test_fn".to_string(),
            "fn test_fn() { ... }".to_string(),
            SymbolType::Function,
        );
        assert_eq!(function_symbol.into_search_result().content, "[fn] test_fn");

        let variable_symbol = Symbol::new(
            "test.rs".to_string(),
            2,
            1,
            "test_var".to_string(),
            "let test_var = ...".to_string(),
            SymbolType::Variable,
        );
        assert_eq!(
            variable_symbol.into_search_result().content,
            "[var] test_var"
        );

        let constant_symbol = Symbol::new(
            "test.rs".to_string(),
            3,
            1,
            "TEST_CONST".to_string(),
            "const TEST_CONST: i32 = ...".to_string(),
            SymbolType::Constant,
        );
        assert_eq!(
            constant_symbol.into_search_result().content,
            "[const] TEST_CONST"
        );
    }

    #[test]
    fn test_symbol_type_equality() {
        // Test SymbolType equality
        assert_eq!(SymbolType::Function, SymbolType::Function);
        assert_eq!(SymbolType::Variable, SymbolType::Variable);
        assert_ne!(SymbolType::Function, SymbolType::Method);
        assert_ne!(SymbolType::Variable, SymbolType::Constant);
    }

    #[test]
    fn test_clone_functionality() {
        // Test Clone trait on SearchParams
        let params = SearchParams {
            query: "clone_test".to_string(),
            mode: SearchMode::Regexp,
        };
        let cloned_params = params.clone();
        assert_eq!(cloned_params.query, "clone_test");
        assert_eq!(cloned_params.mode, SearchMode::Regexp);

        // Test Clone trait on SearchResult
        let result = SearchResult {
            filename: "clone.rs".to_string(),
            line: 10,
            column: 5,
            content: "clone content".to_string(),
        };
        let cloned_result = result.clone();
        assert_eq!(cloned_result.filename, "clone.rs");
        assert_eq!(cloned_result.line, 10);
        assert_eq!(cloned_result.column, 5);
        assert_eq!(cloned_result.content, "clone content");

        // Test Clone trait on Symbol
        let symbol = Symbol::new(
            "clone.rs".to_string(),
            20,
            15,
            "clone_symbol".to_string(),
            "fn clone_symbol(&self) { ... }".to_string(),
            SymbolType::Method,
        );
        let cloned_symbol = symbol.clone();
        assert_eq!(cloned_symbol.filepath, "clone.rs");
        assert_eq!(cloned_symbol.line, 20);
        assert_eq!(cloned_symbol.column, 15);
        assert_eq!(cloned_symbol.name, "clone_symbol");
        assert_eq!(cloned_symbol.content, "fn clone_symbol(&self) { ... }");
        assert_eq!(cloned_symbol.symbol_type, SymbolType::Method);
    }

    #[test]
    fn test_search_mode_debug_format() {
        // Test Debug implementation for SearchMode
        assert_eq!(format!("{:?}", SearchMode::Literal), "Literal");
        assert_eq!(format!("{:?}", SearchMode::Regexp), "Regexp");
        assert_eq!(format!("{:?}", SearchMode::Filepath), "Filepath");
        assert_eq!(format!("{:?}", SearchMode::Symbol), "Symbol");
        assert_eq!(format!("{:?}", SearchMode::Variable), "Variable");
    }

    #[test]
    fn test_search_params_debug_format() {
        let params = SearchParams {
            query: "test_query".to_string(),
            mode: SearchMode::Regexp,
        };
        let debug_output = format!("{:?}", params);
        assert!(debug_output.contains("test_query"));
        assert!(debug_output.contains("Regexp"));
    }

    #[test]
    fn test_search_result_debug_format() {
        let result = SearchResult {
            filename: "test.rs".to_string(),
            line: 42,
            column: 10,
            content: "fn test_function()".to_string(),
        };
        let debug_output = format!("{:?}", result);
        assert!(debug_output.contains("test.rs"));
        assert!(debug_output.contains("42"));
        assert!(debug_output.contains("10"));
        assert!(debug_output.contains("fn test_function()"));
    }

    #[test]
    fn test_symbol_type_hash_and_equality() {
        use std::collections::HashSet;

        // Test that SymbolType can be used in hash collections
        let mut set = HashSet::new();
        set.insert(SymbolType::Function);
        set.insert(SymbolType::Method);
        set.insert(SymbolType::Function); // Duplicate should not increase size

        assert_eq!(set.len(), 2);
        assert!(set.contains(&SymbolType::Function));
        assert!(set.contains(&SymbolType::Method));
        assert!(!set.contains(&SymbolType::Class));
    }

    #[test]
    fn test_symbol_type_all_display_names_complete() {
        // Test all SymbolType display names to ensure complete coverage
        let test_cases = vec![
            (SymbolType::Function, "fn"),
            (SymbolType::Method, "method"),
            (SymbolType::Class, "class"),
            (SymbolType::Struct, "struct"),
            (SymbolType::Enum, "enum"),
            (SymbolType::Interface, "interface"),
            (SymbolType::Variable, "var"),
            (SymbolType::Constant, "const"),
            (SymbolType::Module, "mod"),
            (SymbolType::Type, "type"),
            (SymbolType::Field, "field"),
            (SymbolType::Parameter, "param"),
        ];

        for (symbol_type, expected_name) in test_cases {
            assert_eq!(symbol_type.display_name(), expected_name);
        }
    }

    #[test]
    fn test_search_mode_copy_trait() {
        // Test that SearchMode implements Copy
        let mode1 = SearchMode::Symbol;
        let mode2 = mode1; // This should work with Copy
        assert_eq!(mode1, mode2);

        // Original should still be usable
        assert_eq!(mode1, SearchMode::Symbol);
    }

    #[test]
    fn test_symbol_type_copy_trait() {
        // Test that SymbolType implements Copy
        let type1 = SymbolType::Function;
        let type2 = type1; // This should work with Copy
        assert_eq!(type1, type2);

        // Original should still be usable
        assert_eq!(type1, SymbolType::Function);
    }
}
