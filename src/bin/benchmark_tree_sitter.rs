use std::time::Instant;
use std::path::Path;
use tree_sitter::{Parser, Query, QueryCursor};
use anyhow::Result;
use sfs::indexer::TreeSitterIndexer;

fn benchmark_tree_sitter_parsing(file_path: &Path) -> Result<()> {
    println!("🔍 Benchmarking Tree-sitter vs Regex for: {}", file_path.display());
    println!("==========================================");
    
    // Read file content
    let content = std::fs::read_to_string(file_path)?;
    let lines = content.lines().count();
    println!("📄 File size: {} bytes, {} lines", content.len(), lines);
    
    // 1. Benchmark Tree-sitter parsing
    println!("\n⚡ Tree-sitter Performance:");
    
    let start = Instant::now();
    let mut parser = Parser::new();
    let language = tree_sitter_rust::language();
    parser.set_language(language)?;
    let tree_sitter_init_time = start.elapsed();
    println!("  - Parser initialization: {:.3}ms", tree_sitter_init_time.as_secs_f64() * 1000.0);
    
    let start = Instant::now();
    let tree = parser.parse(&content, None).unwrap();
    let parse_time = start.elapsed();
    println!("  - Parsing time: {:.3}ms", parse_time.as_secs_f64() * 1000.0);
    
    // Extract symbols with Tree-sitter
    let start = Instant::now();
    let query_source = r#"
        (struct_item name: (type_identifier) @name)
        (enum_item name: (type_identifier) @name)
        (function_item name: (identifier) @name)
        (impl_item type: (type_identifier) @name)
        (trait_item name: (type_identifier) @name)
        (const_item name: (identifier) @name)
        (static_item name: (identifier) @name)
        (mod_item name: (identifier) @name)
    "#;
    
    let query = Query::new(language, query_source)?;
    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
    
    let mut symbol_count = 0;
    for match_ in matches {
        for capture in match_.captures {
            let _node = capture.node;
            symbol_count += 1;
        }
    }
    let query_time = start.elapsed();
    println!("  - Symbol extraction: {:.3}ms", query_time.as_secs_f64() * 1000.0);
    println!("  - Symbols found: {}", symbol_count);
    
    let total_tree_sitter = tree_sitter_init_time + parse_time + query_time;
    println!("  - Total Tree-sitter time: {:.3}ms", total_tree_sitter.as_secs_f64() * 1000.0);
    
    // 2. Benchmark Regex parsing (current implementation)
    println!("\n🔧 Regex Performance:");
    
    let start = Instant::now();
    let mut indexer = TreeSitterIndexer::new();
    indexer.initialize_sync()?;
    let regex_init_time = start.elapsed();
    println!("  - Indexer initialization: {:.3}ms", regex_init_time.as_secs_f64() * 1000.0);
    
    let start = Instant::now();
    let symbols = indexer.extract_symbols_sync(file_path, false)?;
    let regex_extract_time = start.elapsed();
    println!("  - Symbol extraction: {:.3}ms", regex_extract_time.as_secs_f64() * 1000.0);
    println!("  - Symbols found: {}", symbols.len());
    
    let total_regex = regex_init_time + regex_extract_time;
    println!("  - Total Regex time: {:.3}ms", total_regex.as_secs_f64() * 1000.0);
    
    // 3. Comparison
    println!("\n📊 Performance Comparison:");
    let ratio = total_tree_sitter.as_secs_f64() / total_regex.as_secs_f64();
    println!("  - Tree-sitter is {:.1}x {} than Regex", 
             if ratio > 1.0 { ratio } else { 1.0 / ratio },
             if ratio > 1.0 { "slower" } else { "faster" });
    
    println!("\n💡 Analysis:");
    if symbol_count > symbols.len() {
        println!("  - Tree-sitter found {} more symbols", symbol_count - symbols.len());
    } else if symbols.len() > symbol_count {
        println!("  - Regex found {} more symbols", symbols.len() - symbol_count);
    }
    
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rust_file_path>", args[0]);
        std::process::exit(1);
    }
    
    let file_path = Path::new(&args[1]);
    benchmark_tree_sitter_parsing(file_path)?;
    
    // Benchmark multiple files if provided
    if args.len() > 2 {
        println!("\n\n🔄 Running batch benchmark...");
        let mut total_tree_sitter = std::time::Duration::new(0, 0);
        let mut total_regex = std::time::Duration::new(0, 0);
        
        for path in &args[1..] {
            let file_path = Path::new(path);
            println!("\n📁 {}", file_path.display());
            
            // Quick benchmark without detailed output
            let content = std::fs::read_to_string(file_path)?;
            
            // Tree-sitter
            let start = Instant::now();
            let mut parser = Parser::new();
            parser.set_language(tree_sitter_rust::language())?;
            let tree = parser.parse(&content, None).unwrap();
            let ts_time = start.elapsed();
            total_tree_sitter += ts_time;
            
            // Regex
            let start = Instant::now();
            let mut indexer = TreeSitterIndexer::new();
            indexer.initialize_sync()?;
            let _ = indexer.extract_symbols_sync(file_path, false)?;
            let regex_time = start.elapsed();
            total_regex += regex_time;
            
            println!("  Tree-sitter: {:.1}ms, Regex: {:.1}ms", 
                     ts_time.as_secs_f64() * 1000.0,
                     regex_time.as_secs_f64() * 1000.0);
        }
        
        println!("\n📈 Batch Results:");
        println!("  Total Tree-sitter: {:.1}ms", total_tree_sitter.as_secs_f64() * 1000.0);
        println!("  Total Regex: {:.1}ms", total_regex.as_secs_f64() * 1000.0);
        let ratio = total_tree_sitter.as_secs_f64() / total_regex.as_secs_f64();
        println!("  Tree-sitter is {:.1}x {} overall", 
                 if ratio > 1.0 { ratio } else { 1.0 / ratio },
                 if ratio > 1.0 { "slower" } else { "faster" });
    }
    
    Ok(())
}