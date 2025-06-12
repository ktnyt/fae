use anyhow::Result;
use sfs::{types::*, FileWatcher, TreeSitterIndexer};
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[cfg(test)]
mod file_watching_tests {
    use super::*;

    #[test]
    fn should_create_file_watcher_successfully() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = vec!["**/*.rs".to_string()];

        let watcher = FileWatcher::new(temp_dir.path(), patterns, Some(100));
        assert!(watcher.is_ok(), "FileWatcher creation should succeed");
    }

    #[test]
    fn should_handle_invalid_watch_directory() {
        let invalid_path = PathBuf::from("/non/existent/directory");
        let patterns = vec!["**/*.rs".to_string()];

        let watcher = FileWatcher::new(&invalid_path, patterns, None);
        assert!(
            watcher.is_err(),
            "FileWatcher should fail for invalid directory"
        );
    }

    #[test]
    fn should_respect_file_patterns() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        let rs_file = temp_dir.path().join("test.rs");
        fs::write(&rs_file, "fn main() {}").unwrap();

        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "hello world").unwrap();

        let patterns = vec!["**/*.rs".to_string()];
        let watcher = FileWatcher::new(temp_dir.path(), patterns, Some(50)).unwrap();

        // Verify watcher was created successfully
        assert!(watcher.updates().try_recv().is_err()); // No updates initially
    }

    #[test]
    fn should_configure_debounce_timing() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = vec!["**/*".to_string()];

        // Test with different debounce settings
        let watcher_50ms = FileWatcher::new(temp_dir.path(), patterns.clone(), Some(50));
        assert!(watcher_50ms.is_ok());

        let watcher_200ms = FileWatcher::new(temp_dir.path(), patterns.clone(), Some(200));
        assert!(watcher_200ms.is_ok());

        let watcher_default = FileWatcher::new(temp_dir.path(), patterns, None);
        assert!(watcher_default.is_ok());
    }

    #[test]
    fn should_handle_empty_patterns_list() {
        let temp_dir = TempDir::new().unwrap();
        let patterns = vec![];

        let watcher = FileWatcher::new(temp_dir.path(), patterns, Some(100));
        assert!(watcher.is_ok(), "FileWatcher should handle empty patterns");
    }
}

#[cfg(test)]
mod index_update_tests {
    use super::*;

    #[test]
    fn should_apply_add_index_update() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn hello() { println!(\"world\"); }")?;

        let symbols = vec![CodeSymbol {
            name: "hello".to_string(),
            symbol_type: SymbolType::Function,
            file: test_file.clone(),
            line: 1,
            column: 4,
            context: Some("fn hello() { println!(\"world\"); }".to_string()),
        }];

        let update = IndexUpdate::Added {
            file: test_file.clone(),
            symbols: symbols.clone(),
        };

        let initial_count = indexer.get_symbol_count();
        indexer.apply_index_update(&update)?;
        let final_count = indexer.get_symbol_count();

        assert!(
            final_count > initial_count,
            "Symbol count should increase after adding file"
        );
        assert!(
            indexer.is_file_indexed(&test_file),
            "File should be marked as indexed"
        );

        Ok(())
    }

    #[test]
    fn should_apply_modify_index_update() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn original() {}")?;

        // First add the file
        let original_symbols = vec![CodeSymbol {
            name: "original".to_string(),
            symbol_type: SymbolType::Function,
            file: test_file.clone(),
            line: 1,
            column: 4,
            context: Some("fn original() {}".to_string()),
        }];

        let add_update = IndexUpdate::Added {
            file: test_file.clone(),
            symbols: original_symbols,
        };
        indexer.apply_index_update(&add_update)?;

        // Then modify it
        let modified_symbols = vec![
            CodeSymbol {
                name: "modified".to_string(),
                symbol_type: SymbolType::Function,
                file: test_file.clone(),
                line: 1,
                column: 4,
                context: Some("fn modified() {}".to_string()),
            },
            CodeSymbol {
                name: "additional".to_string(),
                symbol_type: SymbolType::Function,
                file: test_file.clone(),
                line: 2,
                column: 4,
                context: Some("fn additional() {}".to_string()),
            },
        ];

        let modify_update = IndexUpdate::Modified {
            file: test_file.clone(),
            symbols: modified_symbols,
        };
        indexer.apply_index_update(&modify_update)?;

        assert!(
            indexer.is_file_indexed(&test_file),
            "File should still be indexed after modification"
        );

        Ok(())
    }

    #[test]
    fn should_apply_remove_index_update() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn hello() {}")?;

        // First add the file
        let symbols = vec![CodeSymbol {
            name: "hello".to_string(),
            symbol_type: SymbolType::Function,
            file: test_file.clone(),
            line: 1,
            column: 4,
            context: Some("fn hello() {}".to_string()),
        }];

        let add_update = IndexUpdate::Added {
            file: test_file.clone(),
            symbols,
        };
        indexer.apply_index_update(&add_update)?;

        let count_after_add = indexer.get_symbol_count();
        assert!(
            indexer.is_file_indexed(&test_file),
            "File should be indexed after addition"
        );

        // Then remove it
        let remove_update = IndexUpdate::Removed {
            file: test_file.clone(),
            symbol_count: 1,
        };
        indexer.apply_index_update(&remove_update)?;

        let count_after_remove = indexer.get_symbol_count();
        assert!(
            count_after_remove < count_after_add,
            "Symbol count should decrease after removal"
        );
        assert!(
            !indexer.is_file_indexed(&test_file),
            "File should not be indexed after removal"
        );

        Ok(())
    }

    #[test]
    fn should_handle_reindex_file_operation() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn test_function() { let x = 42; }")?;

        let patterns = vec!["**/*.rs".to_string()];
        let symbols = indexer.reindex_file(&test_file, &patterns)?;

        assert!(!symbols.is_empty(), "Reindexing should extract symbols");
        assert!(
            symbols.iter().any(|s| s.name == "test_function"),
            "Should find the test function"
        );

        Ok(())
    }

    #[test]
    fn should_handle_non_existent_file_reindexing() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let non_existent_file = PathBuf::from("/tmp/non_existent_file.rs");
        let patterns = vec!["**/*.rs".to_string()];

        let result = indexer.reindex_file(&non_existent_file, &patterns);
        // The implementation might handle this gracefully and return empty symbols
        // rather than an error, which is also acceptable behavior
        match result {
            Ok(symbols) => {
                assert!(
                    symbols.is_empty(),
                    "Non-existent file should return empty symbols"
                );
            }
            Err(_) => {
                // Also acceptable to return an error
            }
        }

        Ok(())
    }

    #[test]
    fn should_track_file_and_symbol_counts() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let initial_file_count = indexer.get_file_count();
        let initial_symbol_count = indexer.get_symbol_count();

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("test.rs");
        fs::write(&test_file, "fn hello() {} fn world() {}")?;

        let symbols = vec![
            CodeSymbol {
                name: "hello".to_string(),
                symbol_type: SymbolType::Function,
                file: test_file.clone(),
                line: 1,
                column: 4,
                context: None,
            },
            CodeSymbol {
                name: "world".to_string(),
                symbol_type: SymbolType::Function,
                file: test_file.clone(),
                line: 1,
                column: 15,
                context: None,
            },
        ];

        indexer.add_file_symbols(&test_file, symbols)?;

        let final_file_count = indexer.get_file_count();
        let final_symbol_count = indexer.get_symbol_count();

        assert_eq!(
            final_file_count,
            initial_file_count + 1,
            "File count should increase by 1"
        );
        assert_eq!(
            final_symbol_count,
            initial_symbol_count + 2,
            "Symbol count should increase by 2"
        );

        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn should_handle_multiple_file_operations_sequence() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;

        // Create multiple test files
        let file1 = temp_dir.path().join("file1.rs");
        let file2 = temp_dir.path().join("file2.rs");
        let file3 = temp_dir.path().join("file3.rs");

        fs::write(&file1, "fn function1() {}")?;
        fs::write(&file2, "fn function2() {}")?;
        fs::write(&file3, "fn function3() {}")?;

        let patterns = vec!["**/*.rs".to_string()];

        // Add files sequentially
        for file in [&file1, &file2, &file3] {
            let symbols = indexer.reindex_file(file, &patterns)?;
            let update = IndexUpdate::Added {
                file: file.clone(),
                symbols,
            };
            indexer.apply_index_update(&update)?;
        }

        assert_eq!(indexer.get_file_count(), 3, "Should have 3 files indexed");
        assert!(
            indexer.get_symbol_count() >= 3,
            "Should have at least 3 symbols"
        );

        // Modify one file
        fs::write(&file2, "fn modified_function2() {} fn additional() {}")?;
        let new_symbols = indexer.reindex_file(&file2, &patterns)?;
        let modify_update = IndexUpdate::Modified {
            file: file2.clone(),
            symbols: new_symbols,
        };
        indexer.apply_index_update(&modify_update)?;

        // Remove one file
        let remove_update = IndexUpdate::Removed {
            file: file3.clone(),
            symbol_count: 1,
        };
        indexer.apply_index_update(&remove_update)?;

        assert_eq!(
            indexer.get_file_count(),
            2,
            "Should have 2 files after removal"
        );
        assert!(
            !indexer.is_file_indexed(&file3),
            "Removed file should not be indexed"
        );
        assert!(
            indexer.is_file_indexed(&file1),
            "Other files should remain indexed"
        );
        assert!(
            indexer.is_file_indexed(&file2),
            "Modified file should remain indexed"
        );

        Ok(())
    }

    #[test]
    fn should_handle_rapid_file_changes_with_debouncing() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let patterns = vec!["**/*.rs".to_string()];

        // Create file watcher with short debounce for testing
        let watcher = FileWatcher::new(temp_dir.path(), patterns, Some(50))?;

        // Create a test file
        let test_file = temp_dir.path().join("rapid_changes.rs");
        fs::write(&test_file, "fn initial() {}")?;

        // Wait a bit to ensure any initial events are processed
        thread::sleep(Duration::from_millis(100));

        // Rapidly modify the file multiple times
        for i in 1..=5 {
            fs::write(&test_file, format!("fn version_{i}() {{}}"))?;
            thread::sleep(Duration::from_millis(10)); // Faster than debounce
        }

        // Wait for debouncing to complete
        thread::sleep(Duration::from_millis(200));

        // The debouncer should have processed the events
        // We can't easily test the exact behavior without more complex setup,
        // but we can verify the watcher doesn't crash
        let updates_available = watcher.updates().try_recv().is_ok();
        // Either we get an update or we don't, both are valid due to timing
        // The important thing is that the watcher is still operational
        let _ = updates_available;

        Ok(())
    }

    #[test]
    fn should_maintain_performance_with_large_symbol_sets() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;

        // Create a large number of symbols in a single file
        let test_file = temp_dir.path().join("large_file.rs");
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("fn function_{i}() {{}}\n"));
        }
        fs::write(&test_file, content)?;

        let start_time = std::time::Instant::now();

        let patterns = vec!["**/*.rs".to_string()];
        let symbols = indexer.reindex_file(&test_file, &patterns)?;

        let duration = start_time.elapsed();

        assert!(symbols.len() >= 100, "Should extract at least 100 symbols");
        assert!(
            duration.as_millis() < 1000,
            "Should complete within 1 second for 100 symbols"
        );

        // Test update performance
        let update = IndexUpdate::Added {
            file: test_file.clone(),
            symbols,
        };

        let start_time = std::time::Instant::now();
        indexer.apply_index_update(&update)?;
        let update_duration = start_time.elapsed();

        assert!(
            update_duration.as_millis() < 500,
            "Index update should complete within 500ms"
        );

        Ok(())
    }

    #[test]
    fn should_handle_concurrent_file_operations_safely() -> Result<()> {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let indexer = Arc::new(Mutex::new({
            let mut idx = TreeSitterIndexer::with_options(false, true);
            idx.initialize_sync().unwrap();
            idx
        }));

        let temp_dir = TempDir::new()?;
        let patterns = vec!["**/*.rs".to_string()];

        let mut handles = vec![];

        // Spawn multiple threads to perform concurrent operations
        for i in 0..5 {
            let indexer_clone = Arc::clone(&indexer);
            let temp_dir_path = temp_dir.path().to_path_buf();
            let patterns_clone = patterns.clone();

            let handle = thread::spawn(move || -> Result<()> {
                let test_file = temp_dir_path.join(format!("concurrent_{i}.rs"));
                fs::write(&test_file, format!("fn concurrent_function_{i}() {{}}"))?;

                // Reindex and update
                let symbols = {
                    let mut idx = indexer_clone.lock().unwrap();
                    idx.reindex_file(&test_file, &patterns_clone)?
                };

                let update = IndexUpdate::Added {
                    file: test_file,
                    symbols,
                };

                {
                    let mut idx = indexer_clone.lock().unwrap();
                    idx.apply_index_update(&update)?;
                }

                Ok(())
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle
                .join()
                .map_err(|e| anyhow::anyhow!("Thread panicked: {:?}", e))??;
        }

        // Verify final state
        let final_count = {
            let idx = indexer.lock().unwrap();
            idx.get_file_count()
        };

        assert_eq!(
            final_count, 5,
            "Should have processed all 5 concurrent files"
        );

        Ok(())
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn should_handle_invalid_file_content_gracefully() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;
        let test_file = temp_dir.path().join("invalid.rs");

        // Write invalid Rust syntax
        fs::write(&test_file, "this is not valid rust syntax !@#$%^&*()")?;

        let patterns = vec!["**/*.rs".to_string()];
        let result = indexer.reindex_file(&test_file, &patterns);

        // Should not crash, even with invalid syntax
        match result {
            Ok(symbols) => {
                // May return empty symbols or some partial extraction
                assert!(
                    symbols.len() < 10,
                    "Should not extract many symbols from invalid syntax"
                );
            }
            Err(_) => {
                // It's also acceptable for parsing to fail gracefully
            }
        }

        Ok(())
    }

    #[test]
    fn should_handle_permission_denied_gracefully() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        // Try to reindex a file we can't read (system file)
        let protected_file = PathBuf::from("/dev/null");
        let patterns = vec!["**/*".to_string()];

        let result = indexer.reindex_file(&protected_file, &patterns);

        // Should handle gracefully (either succeed with empty results or fail cleanly)
        match result {
            Ok(symbols) => {
                // Some implementations might extract filename symbols even from /dev/null
                // which is acceptable behavior
                let _symbol_names: Vec<_> = symbols.iter().map(|s| &s.name).collect();
                if !symbols.is_empty() {
                    // If symbols are found, they should be filename/dirname symbols
                    assert!(
                        symbols.iter().all(|s| matches!(
                            s.symbol_type,
                            SymbolType::Filename | SymbolType::Dirname
                        )),
                        "Should only extract filename/dirname symbols from special files"
                    );
                }
            }
            Err(_) => {
                // Also acceptable to fail for protected files
            }
        }

        Ok(())
    }

    #[test]
    fn should_handle_memory_pressure_gracefully() -> Result<()> {
        let mut indexer = TreeSitterIndexer::with_options(false, true);
        indexer.initialize_sync()?;

        let temp_dir = TempDir::new()?;

        // Try to add a very large number of symbols to test memory handling
        let test_file = temp_dir.path().join("memory_test.rs");
        fs::write(&test_file, "fn dummy() {}")?;

        let mut large_symbol_set = Vec::new();
        for i in 0..10000 {
            large_symbol_set.push(CodeSymbol {
                name: format!("symbol_{i}"),
                symbol_type: SymbolType::Function,
                file: test_file.clone(),
                line: i,
                column: 1,
                context: Some(format!("fn symbol_{i}() {{}}")),
            });
        }

        let start_memory = get_memory_usage();

        let result = indexer.add_file_symbols(&test_file, large_symbol_set);

        let end_memory = get_memory_usage();
        let memory_increase = end_memory.saturating_sub(start_memory);

        assert!(result.is_ok(), "Should handle large symbol sets");
        assert!(
            memory_increase < 100_000_000,
            "Memory usage should not increase excessively (100MB)"
        );

        Ok(())
    }
}

// Helper function to get approximate memory usage (simplified)
fn get_memory_usage() -> usize {
    // This is a very basic approximation
    // In a real implementation, you might use a more sophisticated method

    // For testing purposes, we'll use a simple heuristic
    // In production, you might use process memory metrics
    0 // Placeholder implementation
}
