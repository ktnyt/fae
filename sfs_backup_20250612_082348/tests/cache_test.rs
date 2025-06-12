use anyhow::Result;
use sfs::{types::*, TreeSitterIndexer};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn should_create_new_cache_with_default_values() {
        let cache = IndexCache::new();

        assert_eq!(cache.version, "1.0");
        assert!(cache.is_compatible());
        assert_eq!(cache.files.len(), 0);
        assert!(cache.sfs_version.contains('.'));
    }

    #[test]
    fn should_add_and_retrieve_cached_files() {
        let mut cache = IndexCache::new();

        let symbols = vec![CodeSymbol {
            name: "test_function".to_string(),
            symbol_type: SymbolType::Function,
            file: PathBuf::from("test.rs"),
            line: 1,
            column: 4,
            context: Some("fn test_function() {}".to_string()),
        }];

        let cached_file = CachedFile {
            hash: "sha256:abcd1234".to_string(),
            last_modified: "2025-06-11T18:00:46+09:00".to_string(),
            symbols: symbols.clone(),
            size: 100,
        };

        cache.update_file("test.rs".to_string(), cached_file.clone());

        let retrieved = cache.get_file("test.rs").unwrap();
        assert_eq!(retrieved.hash, "sha256:abcd1234");
        assert_eq!(retrieved.symbols.len(), 1);
        assert_eq!(retrieved.symbols[0].name, "test_function");
    }

    #[test]
    fn should_calculate_file_hash_correctly() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() { println!(\"Hello, world!\"); }")?;

        let indexer = TreeSitterIndexer::new();
        let hash1 = indexer.calculate_file_hash(&test_file)?;
        let hash2 = indexer.calculate_file_hash(&test_file)?;

        // Same file should produce same hash
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"));
        assert_eq!(hash1.len(), 71); // "sha256:" + 64 hex chars

        Ok(())
    }

    #[test]
    fn should_detect_file_changes_via_hash() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn main() {}")?;

        let indexer = TreeSitterIndexer::new();
        let original_hash = indexer.calculate_file_hash(&test_file)?;

        // Modify file content
        fs::write(&test_file, "fn main() { println!(\"changed\"); }")?;
        let new_hash = indexer.calculate_file_hash(&test_file)?;

        assert_ne!(original_hash, new_hash);
        assert!(!indexer.is_cache_valid(&test_file, &original_hash)?);
        assert!(indexer.is_cache_valid(&test_file, &new_hash)?);

        Ok(())
    }

    #[test]
    fn should_save_and_load_cache_to_disk() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn test() {}")?;

        // Create and populate cache
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        // Load symbols and update cache
        let symbols = indexer.create_file_symbols(&test_file)?;
        indexer.update_cache_entry(&test_file, &symbols)?;

        // Save cache to disk
        indexer.save_cache(temp_dir.path())?;

        // Verify compressed cache file exists
        let cache_path = temp_dir.path().join(".sfscache.gz");
        assert!(cache_path.exists());

        // Create new indexer and load cache
        let mut new_indexer = TreeSitterIndexer::with_options(false, true);
        new_indexer.initialize_sync()?;
        let stats = new_indexer.load_cache(temp_dir.path())?;

        assert_eq!(stats.total_files, 1);
        assert!(stats.total_symbols > 0);

        Ok(())
    }

    #[test]
    fn should_use_cached_symbols_when_file_unchanged() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn cached_function() { let x = 42; }")?;

        let mut indexer = TreeSitterIndexer::with_options(true, true); // verbose for testing
        indexer.initialize_sync()?;

        // First load - should index file
        let symbols1 = indexer.load_or_index_file(&test_file)?;
        assert!(!symbols1.is_empty());

        // Second load - should use cache
        let symbols2 = indexer.load_or_index_file(&test_file)?;
        assert_eq!(symbols1.len(), symbols2.len());

        // Verify symbols are identical
        for (s1, s2) in symbols1.iter().zip(symbols2.iter()) {
            assert_eq!(s1.name, s2.name);
            assert_eq!(s1.symbol_type, s2.symbol_type);
            assert_eq!(s1.line, s2.line);
            assert_eq!(s1.column, s2.column);
        }

        Ok(())
    }

    #[test]
    fn should_reindex_when_file_content_changes() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn original_function() {}")?;

        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        // First load
        let symbols1 = indexer.load_or_index_file(&test_file)?;
        let original_count = symbols1.len();

        // Modify file
        fs::write(&test_file, "fn original_function() {} fn new_function() {}")?;

        // Second load - should detect change and re-index
        let symbols2 = indexer.load_or_index_file(&test_file)?;

        // Should have more symbols now
        assert!(symbols2.len() > original_count);

        Ok(())
    }

    #[test]
    fn should_handle_cache_disabled_gracefully() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn test() {}")?;

        let mut indexer = TreeSitterIndexer::new();
        indexer.set_cache_enabled(false);
        indexer.initialize_sync()?;

        // Should still work without cache
        let symbols = indexer.load_or_index_file(&test_file)?;
        assert!(!symbols.is_empty());

        // Cache operations should fail gracefully
        assert!(indexer.save_cache(temp_dir.path()).is_err());
        assert!(indexer.load_cache(temp_dir.path()).is_err());

        Ok(())
    }

    #[test]
    fn should_remove_cache_entries_correctly() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn test() {}")?;

        let mut indexer = TreeSitterIndexer::new();
        indexer.initialize_sync()?;

        // Add cache entry
        let symbols = indexer.create_file_symbols(&test_file)?;
        indexer.update_cache_entry(&test_file, &symbols)?;

        let stats_before = indexer.get_cache_stats();
        assert_eq!(stats_before.total_files, 1);

        // Remove cache entry
        indexer.remove_cache_entry(&test_file)?;

        let stats_after = indexer.get_cache_stats();
        assert_eq!(stats_after.total_files, 0);

        Ok(())
    }

    #[test]
    fn should_handle_corrupted_cache_gracefully() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cache_path = temp_dir.path().join(".sfscache");

        // Create corrupted cache file
        fs::write(&cache_path, "invalid json content")?;

        let mut indexer = TreeSitterIndexer::new();
        indexer.initialize_sync()?;

        // Should handle corrupted cache gracefully
        let result = indexer.load_cache(temp_dir.path());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn should_handle_nonexistent_files_correctly() -> Result<()> {
        let nonexistent_file = PathBuf::from("/tmp/nonexistent_file.rs");
        let indexer = TreeSitterIndexer::new();

        // Should return false for nonexistent files
        assert!(!indexer.is_cache_valid(&nonexistent_file, "sha256:anything")?);

        Ok(())
    }

    #[test]
    fn should_clear_cache_correctly() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn test() {}")?;

        let mut indexer = TreeSitterIndexer::new();
        indexer.initialize_sync()?;

        // Add some data to cache
        let symbols = indexer.create_file_symbols(&test_file)?;
        indexer.update_cache_entry(&test_file, &symbols)?;

        assert_eq!(indexer.get_cache_stats().total_files, 1);

        // Clear cache
        indexer.clear_index_cache();

        assert_eq!(indexer.get_cache_stats().total_files, 0);

        Ok(())
    }

    #[test]
    fn should_delete_cache_file_from_disk() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let compressed_cache_path = temp_dir.path().join(".sfscache.gz");
        let uncompressed_cache_path = temp_dir.path().join(".sfscache");

        // Create both compressed and uncompressed cache files
        fs::write(&compressed_cache_path, b"compressed content")?;
        fs::write(
            &uncompressed_cache_path,
            "{\"version\":\"1.0\",\"files\":{}}",
        )?;
        assert!(compressed_cache_path.exists());
        assert!(uncompressed_cache_path.exists());

        let indexer = TreeSitterIndexer::new();
        indexer.delete_cache_file(temp_dir.path())?;

        // Both files should be deleted
        assert!(!compressed_cache_path.exists());
        assert!(!uncompressed_cache_path.exists());

        Ok(())
    }

    #[test]
    fn should_provide_accurate_cache_stats() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("file1.rs");
        fs::write(&test_file, "fn func1() {} fn func2() {}")?;

        let mut indexer = TreeSitterIndexer::new();
        indexer.initialize_sync()?;

        // Add test data via proper API
        let symbols = indexer.create_file_symbols(&test_file)?;
        indexer.update_cache_entry(&test_file, &symbols)?;

        let stats = indexer.get_cache_stats();
        assert_eq!(stats.total_files, 1);
        assert!(stats.total_symbols >= 2); // Should have at least func1 and func2
        assert!(stats.cache_created.contains("2025"));

        Ok(())
    }

    #[test]
    fn should_compress_cache_effectively() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");

        // Create a larger test file to demonstrate compression
        let large_content = "fn test_function() { println!(\"Hello, world!\"); }\n".repeat(100);
        fs::write(&test_file, &large_content)?;

        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        // Load symbols and update cache
        let symbols = indexer.create_file_symbols(&test_file)?;
        indexer.update_cache_entry(&test_file, &symbols)?;

        // Save cache to disk (should be compressed)
        indexer.save_cache(temp_dir.path())?;

        // Verify compressed cache file exists
        let compressed_cache_path = temp_dir.path().join(".sfscache.gz");
        assert!(compressed_cache_path.exists());

        // Verify we can load the compressed cache
        let mut new_indexer = TreeSitterIndexer::with_options(false, true);
        new_indexer.initialize_sync()?;
        let stats = new_indexer.load_cache(temp_dir.path())?;

        assert_eq!(stats.total_files, 1);
        assert!(stats.total_symbols > 0);

        // Check that compression actually happened (file should be smaller than uncompressed JSON)
        let compressed_size = fs::metadata(&compressed_cache_path)?.len();
        assert!(compressed_size > 0);
        assert!(compressed_size < large_content.len() as u64); // Should be smaller than original content

        Ok(())
    }

    #[test]
    fn should_handle_backward_compatibility_with_uncompressed_cache() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let uncompressed_cache_path = temp_dir.path().join(".sfscache");

        // Create an uncompressed cache file (legacy format)
        let legacy_cache = r#"{
            "version": "1.0",
            "cache_created": "2025-06-11T09:00:00+00:00",
            "sfs_version": "0.1.0",
            "files": {}
        }"#;
        fs::write(&uncompressed_cache_path, legacy_cache)?;

        // Should be able to load legacy uncompressed cache
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;
        let stats = indexer.load_cache(temp_dir.path())?;

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.sfs_version, "0.1.0");

        // After saving, should create compressed version
        indexer.save_cache(temp_dir.path())?;

        let compressed_cache_path = temp_dir.path().join(".sfscache.gz");
        assert!(compressed_cache_path.exists());

        Ok(())
    }
}
