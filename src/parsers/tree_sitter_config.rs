use tree_sitter::{Language, Query};
use anyhow::Result;

/// Tree-sitter language configuration for different file types
pub struct LanguageConfig {
    pub language: Language,
    pub query_source: &'static str,
}

/// Get Tree-sitter language and query configuration based on file extension
pub fn get_language_config(extension: &str) -> Option<LanguageConfig> {
    match extension {
        "rs" => Some(LanguageConfig {
            language: tree_sitter_rust::language(),
            query_source: RUST_QUERY,
        }),
        "ts" | "tsx" => Some(LanguageConfig {
            language: tree_sitter_typescript::language_typescript(),
            query_source: TYPESCRIPT_QUERY,
        }),
        "js" | "jsx" => Some(LanguageConfig {
            language: tree_sitter_javascript::language(),
            query_source: JAVASCRIPT_QUERY,
        }),
        "py" => Some(LanguageConfig {
            language: tree_sitter_python::language(),
            query_source: PYTHON_QUERY,
        }),
        "php" => Some(LanguageConfig {
            language: tree_sitter_php::language(),
            query_source: PHP_QUERY,
        }),
        "rb" | "ruby" => Some(LanguageConfig {
            language: tree_sitter_ruby::language(),
            query_source: RUBY_QUERY,
        }),
        "go" => Some(LanguageConfig {
            language: tree_sitter_go::language(),
            query_source: GO_QUERY,
        }),
        "java" => Some(LanguageConfig {
            language: tree_sitter_java::language(),
            query_source: JAVA_QUERY,
        }),
        "c" => Some(LanguageConfig {
            language: tree_sitter_c::language(),
            query_source: C_QUERY,
        }),
        "cpp" | "cc" | "cxx" | "h" | "hpp" => Some(LanguageConfig {
            language: tree_sitter_cpp::language(),
            query_source: CPP_QUERY,
        }),
        "cs" => Some(LanguageConfig {
            language: tree_sitter_c_sharp::language(),
            query_source: CSHARP_QUERY,
        }),
        "scala" => Some(LanguageConfig {
            language: tree_sitter_scala::language(),
            query_source: SCALA_QUERY,
        }),
        _ => None,
    }
}

/// Create a Tree-sitter query for the given language configuration
pub fn create_query(config: &LanguageConfig) -> Result<Query> {
    Query::new(config.language, config.query_source)
        .map_err(|e| anyhow::anyhow!("Failed to create Tree-sitter query: {}", e))
}

// Query definitions for each language
const RUST_QUERY: &str = r#"
    ; Structs
    (struct_item name: (type_identifier) @struct)
    
    ; Enums  
    (enum_item name: (type_identifier) @enum)
    
    ; Functions
    (function_item name: (identifier) @function)
    
    ; Impl blocks
    (impl_item type: (type_identifier) @impl)
    
    ; Traits
    (trait_item name: (type_identifier) @trait)
    
    ; Constants
    (const_item name: (identifier) @const)
    
    ; Statics
    (static_item name: (identifier) @static)
    
    ; Modules
    (mod_item name: (identifier) @module)
    
    ; Type aliases
    (type_item name: (type_identifier) @type)
    
    ; Methods in impl blocks
    (impl_item 
      body: (declaration_list 
        (function_item name: (identifier) @method)))
    
    ; Let bindings
    (let_declaration pattern: (identifier) @variable)
    
    ; Use statements
    (use_declaration argument: (scoped_identifier path: (_) name: (identifier) @use))
    
    ; Field names in structs
    (field_declaration name: (field_identifier) @field)
"#;

const TYPESCRIPT_QUERY: &str = r#"
    ; Classes
    (class_declaration name: (type_identifier) @class)
    
    ; Interfaces
    (interface_declaration name: (type_identifier) @interface)
    
    ; Functions
    (function_declaration name: (identifier) @function)
    
    ; Methods
    (method_definition name: (property_identifier) @method)
    
    ; Type aliases
    (type_alias_declaration name: (type_identifier) @type)
    
    ; Enums
    (enum_declaration name: (identifier) @enum)
    
    ; Arrow functions 
    (lexical_declaration 
      (variable_declarator 
        name: (identifier) @arrow
        value: (arrow_function)))
    
    ; Variables (const, let, var)
    (lexical_declaration 
      (variable_declarator name: (identifier) @variable))
"#;

const JAVASCRIPT_QUERY: &str = r#"
    ; Classes
    (class_declaration name: (identifier) @class)
    
    ; Functions
    (function_declaration name: (identifier) @function)
    
    ; Methods
    (method_definition name: (property_identifier) @method)
    
    ; Arrow functions 
    (lexical_declaration 
      (variable_declarator 
        name: (identifier) @arrow
        value: (arrow_function)))
    
    ; Variables (const, let, var)
    (lexical_declaration 
      (variable_declarator name: (identifier) @variable))
"#;

const PYTHON_QUERY: &str = r#"
    ; Classes
    (class_definition name: (identifier) @class)
    
    ; Functions
    (function_definition name: (identifier) @function)
    
    ; Assignments (variables)
    (assignment left: (identifier) @variable)
"#;

const PHP_QUERY: &str = r#"
    ; Classes
    (class_declaration name: (name) @class)
    
    ; Functions
    (function_definition name: (name) @function)
    
    ; Methods
    (method_declaration name: (name) @method)
"#;

const RUBY_QUERY: &str = r#"
    ; Classes
    (class name: (constant) @class)
    
    ; Methods/Functions
    (method name: (identifier) @function)
    
    ; Modules
    (module name: (constant) @module)
"#;

const GO_QUERY: &str = r#"
    ; Functions
    (function_declaration name: (identifier) @function)
    
    ; Methods
    (method_declaration name: (field_identifier) @method)
    
    ; Types (structs)
    (type_declaration (type_spec name: (type_identifier) @type))
"#;

const JAVA_QUERY: &str = r#"
    ; Classes
    (class_declaration name: (identifier) @class)
    
    ; Methods
    (method_declaration name: (identifier) @method)
    
    ; Constructors
    (constructor_declaration name: (identifier) @constructor)
    
    ; Interfaces
    (interface_declaration name: (identifier) @interface)
"#;

const C_QUERY: &str = r#"
    ; Functions
    (function_definition declarator: (function_declarator declarator: (identifier) @function))
    
    ; Function declarations
    (declaration declarator: (function_declarator declarator: (identifier) @function))
"#;

const CPP_QUERY: &str = r#"
    ; Functions
    (function_definition declarator: (function_declarator declarator: (identifier) @function))
    
    ; Function declarations
    (declaration declarator: (function_declarator declarator: (identifier) @function))
    
    ; Classes
    (class_specifier name: (type_identifier) @class)
"#;

const CSHARP_QUERY: &str = r#"
    ; Classes
    (class_declaration name: (identifier) @class)
    
    ; Methods
    (method_declaration name: (identifier) @method)
    
    ; Interfaces
    (interface_declaration name: (identifier) @interface)
"#;

const SCALA_QUERY: &str = r#"
    ; Classes
    (class_definition name: (identifier) @class)
    
    ; Objects
    (object_definition name: (identifier) @object)
    
    ; Functions
    (function_definition name: (identifier) @function)
"#;