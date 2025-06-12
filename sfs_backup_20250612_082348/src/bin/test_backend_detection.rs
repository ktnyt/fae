use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use which::which;

fn main() {
    println!("🔍 Testing Content Search Backend Detection...\n");

    // Check tool availability manually
    println!("Tool availability:");
    match which("rg") {
        Ok(path) => println!("  ✅ ripgrep found at: {}", path.display()),
        Err(_) => println!("  ❌ ripgrep not found"),
    }

    match which("ag") {
        Ok(path) => println!("  ✅ ag found at: {}", path.display()),
        Err(_) => println!("  ❌ ag not found"),
    }

    // Test with empty symbols (just for backend detection)
    let _searcher = FuzzySearcher::new(vec![]);

    // We can't directly test the private method, but we can infer from behavior
    // Create a test with a known query
    let test_symbol = CodeSymbol {
        name: "test".to_string(),
        symbol_type: SymbolType::Variable,
        file: "./src/lib.rs".into(),
        line: 1,
        column: 1,
        context: None,
    };

    let searcher_with_symbols = FuzzySearcher::new(vec![test_symbol]);

    println!("\n🧪 Testing content search execution:");

    // Test a simple query that should work with any backend
    let results = searcher_with_symbols.search_content("use", &SearchOptions::default());

    if !results.is_empty() {
        println!(
            "  ✅ Content search working: {} results found",
            results.len()
        );
        println!(
            "  📝 First result: {} ({}:{})",
            results[0].symbol.name.chars().take(50).collect::<String>(),
            results[0].symbol.file.display(),
            results[0].symbol.line
        );
    } else {
        println!("  ⚠️  No results found (might be expected)");
    }
}
