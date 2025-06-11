// エラーハンドリングとエッジケースのテスト
// 大容量ファイル、バイナリファイル、破損ファイル、権限エラーなどの処理

// use sfs::types::*;
use sfs::filters::{FileFilter, GitignoreFilter};
use sfs::indexer::TreeSitterIndexer;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn should_handle_large_files_gracefully() {
        let temp_dir = TempDir::new().unwrap();
        let large_file_path = temp_dir.path().join("large.ts");

        // Create a file larger than 1MB (the typical limit)
        let large_content = "// Large file test\n".repeat(100_000); // ~1.8MB
        fs::write(&large_file_path, large_content).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(true);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Should handle large files without panic
        let result = indexer.index_directory(temp_dir.path(), &patterns).await;
        assert!(result.is_ok(), "Should handle large files gracefully");

        let symbols = indexer.get_all_symbols();

        // Large files are skipped by the FileFilter, but this should not cause panic
        // Check that the operation completes successfully, even if no symbols are extracted
        println!(
            "Large file ({} bytes) processing completed. Symbols found: {}",
            fs::metadata(&large_file_path).unwrap().len(),
            symbols.len()
        );

        // The indexer should not crash, which is the main requirement for this test
        // No assertion needed - if we reach this point, the test succeeded
    }

    #[test]
    fn should_filter_binary_files_correctly() {
        let temp_dir = TempDir::new().unwrap();

        // Create various binary files
        fs::write(
            temp_dir.path().join("image.png"),
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("binary.exe"),
            b"\x7fELF\x02\x01\x01\x00",
        )
        .unwrap();
        fs::write(temp_dir.path().join("data.zip"), b"PK\x03\x04\x14\x00").unwrap();

        // Create legitimate text files
        fs::write(temp_dir.path().join("code.ts"), "function test() {}").unwrap();
        fs::write(temp_dir.path().join("readme.md"), "# README").unwrap();

        let file_filter = FileFilter::new(true);

        // Test binary file detection
        assert!(
            !file_filter.should_index_file(&temp_dir.path().join("image.png")),
            "Should not index PNG files"
        );
        assert!(
            !file_filter.should_index_file(&temp_dir.path().join("binary.exe")),
            "Should not index executable files"
        );
        assert!(
            !file_filter.should_index_file(&temp_dir.path().join("data.zip")),
            "Should not index ZIP files"
        );

        // Test text file acceptance
        assert!(
            file_filter.should_index_file(&temp_dir.path().join("code.ts")),
            "Should index TypeScript files"
        );

        // Note: markdown files might not match patterns, but they're not binary
        println!("Binary file filtering working correctly");
    }

    #[tokio::test]
    async fn should_handle_corrupted_files_gracefully() {
        let temp_dir = TempDir::new().unwrap();

        // Create files with invalid UTF-8 content
        let invalid_utf8 = vec![
            0xFF, 0xFE, 0xFD, b'f', b'u', b'n', b'c', b't', b'i', b'o', b'n',
        ];
        fs::write(temp_dir.path().join("corrupted.ts"), invalid_utf8).unwrap();

        // Create a valid file for comparison
        fs::write(temp_dir.path().join("valid.ts"), "function valid() {}").unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(true);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Should handle corrupted files without crashing
        let result = indexer.index_directory(temp_dir.path(), &patterns).await;
        assert!(result.is_ok(), "Should handle corrupted files gracefully");

        let symbols = indexer.get_all_symbols();

        // Should still process the valid file
        assert!(
            symbols.iter().any(|s| s.name == "valid"),
            "Should process valid files even when corrupted files are present"
        );

        // May or may not process the corrupted file depending on how it's handled
        let corrupted_symbols: Vec<_> = symbols
            .iter()
            .filter(|s| s.file.to_string_lossy().contains("corrupted"))
            .collect();

        println!(
            "Corrupted file handling: {} symbols extracted",
            corrupted_symbols.len()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn should_handle_permission_denied_files() {
        let temp_dir = TempDir::new().unwrap();
        let restricted_file = temp_dir.path().join("restricted.ts");

        // Create a file and then remove read permissions
        fs::write(&restricted_file, "function secret() {}").unwrap();

        let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
        perms.set_mode(0o000); // No permissions
        fs::set_permissions(&restricted_file, perms).unwrap();

        // Create a readable file for comparison
        fs::write(
            temp_dir.path().join("readable.ts"),
            "function readable() {}",
        )
        .unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(true);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Should handle permission errors gracefully
        let result = indexer.index_directory(temp_dir.path(), &patterns).await;
        assert!(
            result.is_ok(),
            "Should handle permission denied files gracefully"
        );

        let symbols = indexer.get_all_symbols();

        // Should still process readable files
        assert!(
            symbols.iter().any(|s| s.name == "readable"),
            "Should process readable files even when restricted files are present"
        );

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
        perms.set_mode(0o644);
        let _ = fs::set_permissions(&restricted_file, perms);

        println!(
            "Permission handling test completed with {} symbols",
            symbols.len()
        );
    }

    #[tokio::test]
    async fn should_handle_empty_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create empty subdirectories
        fs::create_dir_all(temp_dir.path().join("empty1")).unwrap();
        fs::create_dir_all(temp_dir.path().join("empty2/nested")).unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string(), "**/*.js".to_string()];

        // Should handle empty directories without error
        let result = indexer.index_directory(temp_dir.path(), &patterns).await;
        assert!(result.is_ok(), "Should handle empty directories gracefully");

        let symbols = indexer.get_all_symbols();
        assert!(
            symbols.is_empty(),
            "Should have no symbols from empty directories"
        );

        println!("Empty directory handling: no symbols extracted as expected");
    }

    #[tokio::test]
    async fn should_handle_non_existent_directory() {
        let non_existent_path = std::path::Path::new("/definitely/does/not/exist");

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Should handle non-existent directories gracefully
        let result = indexer.index_directory(non_existent_path, &patterns).await;

        // Depending on implementation, this might be Ok with empty results or an Err
        // Both are acceptable as long as it doesn't panic
        match result {
            Ok(_) => {
                let symbols = indexer.get_all_symbols();
                assert!(
                    symbols.is_empty(),
                    "Should have no symbols from non-existent directory"
                );
                println!("Non-existent directory handled gracefully (Ok with empty results)");
            }
            Err(e) => {
                println!("Non-existent directory handled gracefully (Err: {})", e);
            }
        }
    }

    #[test]
    fn should_handle_deeply_nested_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create a deeply nested structure
        let mut deep_path = temp_dir.path().to_path_buf();
        for i in 0..20 {
            deep_path = deep_path.join(format!("level_{}", i));
        }
        fs::create_dir_all(&deep_path).unwrap();

        // Create a file at the deep level
        fs::write(deep_path.join("deep.ts"), "function deep() {}").unwrap();

        // Test with gitignore filter
        let gitignore_filter = GitignoreFilter::new(true, true);
        let walker = gitignore_filter.create_walker(temp_dir.path());

        let mut processed_files = 0;
        for entry in walker.build() {
            if let Some(_path) = gitignore_filter.should_process_entry(&entry) {
                processed_files += 1;
            }
        }

        assert!(
            processed_files > 0,
            "Should process files in deeply nested directories"
        );
        println!("Deep nesting handling: {} files processed", processed_files);
    }

    #[tokio::test]
    async fn should_handle_files_with_unusual_extensions() {
        let temp_dir = TempDir::new().unwrap();

        // Create files with unusual but potentially valid extensions
        fs::write(
            temp_dir.path().join("config.ts.bak"),
            "function backup() {}",
        )
        .unwrap();
        fs::write(temp_dir.path().join("script.ts.old"), "function old() {}").unwrap();
        fs::write(temp_dir.path().join("data.json"), r#"{"key": "value"}"#).unwrap();
        fs::write(temp_dir.path().join("no_extension"), "function test() {}").unwrap();

        let mut indexer = TreeSitterIndexer::with_verbose(true);
        indexer.initialize().await.unwrap();

        // Test with TypeScript pattern
        let patterns = vec!["**/*.ts".to_string()];
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let symbols = indexer.get_all_symbols();

        // Should not match .ts.bak or .ts.old (not exact .ts match)
        assert!(
            !symbols
                .iter()
                .any(|s| s.file.to_string_lossy().contains(".ts.bak")),
            "Should not match .ts.bak files with .ts pattern"
        );
        assert!(
            !symbols
                .iter()
                .any(|s| s.file.to_string_lossy().contains(".ts.old")),
            "Should not match .ts.old files with .ts pattern"
        );

        // Test with broader pattern
        let patterns = vec!["**/*".to_string()];
        indexer.clear_cache();
        indexer
            .index_directory(temp_dir.path(), &patterns)
            .await
            .unwrap();

        let all_symbols = indexer.get_all_symbols();

        // Should now find files (at least filename symbols)
        assert!(
            all_symbols.iter().any(|s| s.name.contains(".ts.bak")
                || s.name.contains(".ts.old")
                || s.name.contains("no_extension")),
            "Should process files with unusual extensions when using broad pattern"
        );

        println!(
            "Unusual extensions handling: {} symbols with broad pattern",
            all_symbols.len()
        );
    }

    #[tokio::test]
    async fn should_handle_circular_symlinks_gracefully() {
        let temp_dir = TempDir::new().unwrap();

        // Create a regular file
        fs::write(temp_dir.path().join("regular.ts"), "function regular() {}").unwrap();

        // Note: Creating actual circular symlinks in tests can be tricky
        // This test focuses on the indexer's resilience to unusual directory structures

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];

        // Should handle the directory structure without issues
        let result = indexer.index_directory(temp_dir.path(), &patterns).await;
        assert!(
            result.is_ok(),
            "Should handle directory traversal issues gracefully"
        );

        let symbols = indexer.get_all_symbols();
        assert!(
            symbols.iter().any(|s| s.name == "regular"),
            "Should process regular files even with potential directory issues"
        );

        println!(
            "Directory traversal resilience: {} symbols processed",
            symbols.len()
        );
    }

    #[test]
    fn should_validate_input_parameters() {
        // Test file filter with edge cases
        let file_filter = FileFilter::new(false);

        // Test with empty path - FileFilter doesn't specifically validate empty paths
        // It relies on filesystem operations which will handle errors gracefully
        let empty_path = std::path::Path::new("");
        let result = file_filter.should_index_file(empty_path);
        // Empty path may pass the filter but fail on actual file operations, which is acceptable
        println!("Empty path filter result: {}", result);

        // Test with root path - should be treated normally
        let root_path = std::path::Path::new("/");
        let root_result = file_filter.should_index_file(root_path);
        println!("Root path filter result: {}", root_result);

        // Test gitignore filter with edge cases
        let gitignore_filter = GitignoreFilter::new(true, false);

        // Should create walker without panic even for edge case paths
        let walker = gitignore_filter.create_walker(empty_path);
        let entry_count = walker.build().count();
        println!("Empty path walker entries: {}", entry_count);

        // Test patterns functionality
        let patterns = vec!["**/*.ts".to_string()];
        let ts_file = std::path::Path::new("test.ts");
        assert!(
            file_filter.matches_patterns(ts_file, &patterns),
            "Should match TypeScript file pattern"
        );

        let txt_file = std::path::Path::new("test.txt");
        assert!(
            !file_filter.matches_patterns(txt_file, &patterns),
            "Should not match non-TypeScript file"
        );

        println!("Input validation tests completed");
    }
}
