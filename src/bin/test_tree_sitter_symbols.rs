use anyhow::Result;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};

fn extract_rust_symbols(file_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(file_path)?;

    let mut parser = Parser::new();
    let language = tree_sitter_rust::language();
    parser.set_language(language)?;

    let tree = parser.parse(&content, None).unwrap();

    // More comprehensive query for Rust symbols
    let query_source = r#"
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

    let query = Query::new(language, query_source)?;
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    println!("🔍 Tree-sitter symbols for: {}", file_path.display());
    println!("==========================================");

    let capture_names = query.capture_names();
    let mut symbol_counts = std::collections::HashMap::new();

    for match_ in matches {
        for capture in match_.captures {
            let node = capture.node;
            let capture_name = &capture_names[capture.index as usize];
            let text = node.utf8_text(content.as_bytes())?;
            let start = node.start_position();

            *symbol_counts.entry(capture_name.as_str()).or_insert(0) += 1;

            println!(
                "{:>12} | {:>4}:{:<4} | {}",
                capture_name,
                start.row + 1,
                start.column + 1,
                text
            );
        }
    }

    println!("\n📊 Summary:");
    for (capture_type, count) in &symbol_counts {
        println!("  {}: {}", capture_type, count);
    }
    let total: i32 = symbol_counts.values().sum();
    println!("  Total: {}", total);

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rust_file_path>", args[0]);
        std::process::exit(1);
    }

    let file_path = Path::new(&args[1]);
    extract_rust_symbols(file_path)?;

    Ok(())
}
