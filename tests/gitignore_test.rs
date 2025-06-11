// .gitignore機能のテストケース
// indexer.rsの respect_gitignore フラグの動作を検証

use sfs::types::*;
use sfs::indexer::TreeSitterIndexer;
use tempfile::TempDir;
use std::fs;

#[cfg(test)]
mod gitignore_functionality {
    use super::*;

    fn create_test_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();
        
        // Initialize git repository (required for gitignore to work)
        std::process::Command::new("git")
            .arg("init")
            .current_dir(dir_path)
            .output()?;
        
        // Create .gitignore file
        let gitignore_content = "# Build artifacts\n\
                                target/\n\
                                *.log\n\
                                node_modules/\n\
                                dist/\n\
                                \n\
                                # IDE files\n\
                                .vscode/\n\
                                .idea/\n\
                                \n\
                                # Specific files\n\
                                secret.ts\n\
                                config.local.js\n";
        
        fs::write(dir_path.join(".gitignore"), gitignore_content)?;
        
        // Create regular files (should be included)
        fs::write(dir_path.join("main.ts"), "function main() {}\nexport { main };")?;
        fs::write(dir_path.join("utils.js"), "function helper() {}\nmodule.exports = { helper };")?;
        fs::write(dir_path.join("README.md"), "# Test Project")?;
        
        // Create ignored files (should be excluded when respect_gitignore=true)
        fs::write(dir_path.join("secret.ts"), "const SECRET = 'password';\nexport { SECRET };")?;
        fs::write(dir_path.join("config.local.js"), "const config = { api: 'localhost' };")?;
        fs::write(dir_path.join("debug.log"), "Debug information")?;
        
        // Create ignored directories
        fs::create_dir_all(dir_path.join("target"))?;
        fs::write(dir_path.join("target/output.txt"), "Build output")?;
        
        fs::create_dir_all(dir_path.join("node_modules"))?;
        fs::write(dir_path.join("node_modules/package.json"), "{\"name\": \"test\"}")?;
        
        fs::create_dir_all(dir_path.join(".vscode"))?;
        fs::write(dir_path.join(".vscode/settings.json"), "{\"editor.tabSize\": 2}")?;
        
        Ok(())
    }

    #[tokio::test]
    async fn should_respect_gitignore_when_enabled() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(&temp_dir).unwrap();
        
        let mut indexer = TreeSitterIndexer::with_options(false, true); // respect_gitignore = true
        indexer.initialize().await.unwrap();
        
        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string(), "**/*.md".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let all_symbols = indexer.get_all_symbols();
        
        
        // Should find regular files
        assert!(all_symbols.iter().any(|s| s.name == "main.ts"));
        assert!(all_symbols.iter().any(|s| s.name == "utils.js"));
        assert!(all_symbols.iter().any(|s| s.name == "README.md"));
        
        // Should NOT find ignored files
        assert!(!all_symbols.iter().any(|s| s.name == "secret.ts"));
        assert!(!all_symbols.iter().any(|s| s.name == "config.local.js"));
        assert!(!all_symbols.iter().any(|s| s.name == "debug.log"));
        
        // Should NOT find files in ignored directories
        assert!(!all_symbols.iter().any(|s| s.name == "output.txt"));
        assert!(!all_symbols.iter().any(|s| s.name == "package.json"));
        assert!(!all_symbols.iter().any(|s| s.name == "settings.json"));
        
        // Should find functions in regular files
        assert!(all_symbols.iter().any(|s| s.name == "main" && s.symbol_type == SymbolType::Function));
        assert!(all_symbols.iter().any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function));
        
        // Should NOT find functions in ignored files
        assert!(!all_symbols.iter().any(|s| s.name == "SECRET"));
    }

    #[tokio::test]
    async fn should_ignore_gitignore_when_disabled() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(&temp_dir).unwrap();
        
        let mut indexer = TreeSitterIndexer::with_options(false, false); // respect_gitignore = false
        indexer.initialize().await.unwrap();
        
        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let all_symbols = indexer.get_all_symbols();
        
        
        // Should find regular files
        assert!(all_symbols.iter().any(|s| s.name == "main.ts"));
        assert!(all_symbols.iter().any(|s| s.name == "utils.js"));
        
        // Should ALSO find ignored files when gitignore is disabled
        assert!(all_symbols.iter().any(|s| s.name == "secret.ts"));
        assert!(all_symbols.iter().any(|s| s.name == "config.local.js"));
        
        // Should find functions in both regular and ignored files
        assert!(all_symbols.iter().any(|s| s.name == "main" && s.symbol_type == SymbolType::Function));
        assert!(all_symbols.iter().any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function));
        assert!(all_symbols.iter().any(|s| s.name == "SECRET"));
    }

    #[tokio::test]
    async fn should_handle_project_without_gitignore() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        
        // Create project without .gitignore
        fs::write(temp_dir.path().join("main.ts"), "function main() {}")?;
        fs::write(temp_dir.path().join("utils.js"), "function helper() {}")?;
        
        let mut indexer = TreeSitterIndexer::with_options(false, true); // respect_gitignore = true
        indexer.initialize().await.unwrap();
        
        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let all_symbols = indexer.get_all_symbols();
        
        // Should work normally without .gitignore
        assert!(all_symbols.iter().any(|s| s.name == "main.ts"));
        assert!(all_symbols.iter().any(|s| s.name == "utils.js"));
        assert!(all_symbols.iter().any(|s| s.name == "main" && s.symbol_type == SymbolType::Function));
        assert!(all_symbols.iter().any(|s| s.name == "helper" && s.symbol_type == SymbolType::Function));
        
        Ok(())
    }

    #[tokio::test]
    async fn should_respect_nested_gitignore_rules() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();
        
        // Initialize git repository (required for gitignore to work)
        std::process::Command::new("git")
            .arg("init")
            .current_dir(dir_path)
            .output()?;
        
        // Create main .gitignore
        fs::write(dir_path.join(".gitignore"), "*.tmp\n")?;
        
        // Create subdirectory with its own .gitignore
        fs::create_dir_all(dir_path.join("src"))?;
        fs::write(dir_path.join("src/.gitignore"), "local.ts\n")?;
        
        // Create files
        fs::write(dir_path.join("main.ts"), "function main() {}")?;
        fs::write(dir_path.join("temp.tmp"), "temporary file")?;
        fs::write(dir_path.join("src/index.ts"), "function index() {}")?;
        fs::write(dir_path.join("src/local.ts"), "function local() {}")?;
        
        let mut indexer = TreeSitterIndexer::with_options(false, true); // respect_gitignore = true
        indexer.initialize().await.unwrap();
        
        let patterns = vec!["**/*.ts".to_string(), "**/*.tmp".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let all_symbols = indexer.get_all_symbols();
        
        // Should find regular files
        assert!(all_symbols.iter().any(|s| s.name == "main.ts"));
        assert!(all_symbols.iter().any(|s| s.name == "index.ts"));
        
        // Should NOT find files ignored by root .gitignore
        assert!(!all_symbols.iter().any(|s| s.name == "temp.tmp"));
        
        // Should NOT find files ignored by nested .gitignore
        assert!(!all_symbols.iter().any(|s| s.name == "local.ts"));
        
        Ok(())
    }

    #[tokio::test]
    async fn should_count_symbols_correctly_with_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(&temp_dir).unwrap();
        
        // Test with gitignore enabled
        let mut indexer_with_gitignore = TreeSitterIndexer::with_options(false, true);
        indexer_with_gitignore.initialize().await.unwrap();
        
        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string()];
        indexer_with_gitignore.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let symbols_with_gitignore = indexer_with_gitignore.get_all_symbols();
        
        // Test with gitignore disabled
        let mut indexer_without_gitignore = TreeSitterIndexer::with_options(false, false);
        indexer_without_gitignore.initialize().await.unwrap();
        
        indexer_without_gitignore.index_directory(temp_dir.path(), &patterns).await.unwrap();
        
        let symbols_without_gitignore = indexer_without_gitignore.get_all_symbols();
        
        // Should have fewer symbols when gitignore is respected
        assert!(symbols_with_gitignore.len() < symbols_without_gitignore.len());
        
        // Verify the difference is significant (at least the ignored files)
        let ignored_files = ["secret.ts", "config.local.js"];
        let mut expected_difference = 0;
        
        for file in &ignored_files {
            if symbols_without_gitignore.iter().any(|s| s.name == *file) {
                expected_difference += 1;
            }
        }
        
        assert!(expected_difference > 0, "Should have found some ignored files when gitignore is disabled");
    }
}