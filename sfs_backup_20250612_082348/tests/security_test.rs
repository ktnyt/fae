// セキュリティ・入力バリデーションテストスイート
// 悪意のある入力、不正なファイル、パストラバーサル攻撃等への対処を検証

use sfs::indexer::TreeSitterIndexer;
use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[cfg(test)]
mod security_tests {
    use super::*;

    /// 悪意のあるファイル名・パスを含むプロジェクト構造を作成
    fn create_malicious_files_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // 正常なファイル
        fs::write(
            dir_path.join("normal.ts"),
            "function normal() { return 'safe'; }",
        )?;

        // パストラバーサル攻撃を試みるファイル名（無害化されるべき）
        // 注：実際のファイルシステムでは ../../../ などは無効なので別のアプローチ
        fs::create_dir_all(dir_path.join("subdir"))?;
        fs::write(
            dir_path.join("subdir/..%2e%2f..%2e%2fetc%2epasswd.ts"),
            "// malicious file",
        )?;

        // 非常に長いファイル名（DoS攻撃の可能性）
        let long_filename = "a".repeat(255) + ".ts";
        if long_filename.len() <= 255 {
            fs::write(dir_path.join(&long_filename), "// long filename test")?;
        }

        // 特殊文字を含むファイル名
        fs::write(
            dir_path.join("special'\"<>&;$()|file.ts"),
            "// special chars",
        )?;

        // 空のファイル名は作れないので、隠しファイル
        fs::write(dir_path.join(".hidden_file.ts"), "// hidden file")?;

        // Unicode文字を含むファイル名
        fs::write(
            dir_path.join("unicode_日本語_файл_🎉.ts"),
            "// unicode test",
        )?;

        // ファイル名にnull文字を含む試行（ファイルシステムレベルで防がれる）
        // 代わりに制御文字を含むファイル名をテスト
        let control_char_filename = "control\x01\x02.ts".to_string();
        fs::write(dir_path.join(&control_char_filename), "// control chars")?;

        Ok(())
    }

    /// 悪意のあるファイルコンテンツを含むプロジェクト構造を作成
    fn create_malicious_content_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // 正常なファイル
        fs::write(
            dir_path.join("normal.ts"),
            "function normal() { return 'safe'; }",
        )?;

        // 巨大なファイル（メモリ枯渇攻撃の可能性）
        let large_content = "// ".repeat(500_000) + "large file content\n";
        fs::write(dir_path.join("large_file.ts"), large_content)?;

        // 非常に長い行を含むファイル
        let long_line = format!("const longString = \"{}\";\n", "x".repeat(100_000));
        fs::write(dir_path.join("long_line.ts"), long_line)?;

        // バイナリデータを含むファイル（Tree-sitterクラッシュの可能性）
        let mut binary_content = b"function test() {\n".to_vec();
        binary_content.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x01, 0x80, 0x90]);
        binary_content.extend_from_slice(b"\n}");
        fs::write(dir_path.join("binary_content.ts"), binary_content)?;

        // 無効なUTF-8を含むファイル
        let invalid_utf8 = b"function invalid() {\n    // \xFF\xFE invalid utf8 \x80\x81\n}";
        fs::write(dir_path.join("invalid_utf8.ts"), invalid_utf8)?;

        // 循環参照やスタックオーバーフローを引き起こす可能性のある深いネスト
        let deep_nesting = "{\n".repeat(1000) + "const deep = true;" + &"}".repeat(1000);
        fs::write(dir_path.join("deep_nesting.ts"), deep_nesting)?;

        // 膨大な数のシンボルを含むファイル（パフォーマンス攻撃）
        let mut many_symbols = String::new();
        for i in 0..10_000 {
            many_symbols.push_str(&format!("function func{}() {{ return {}; }}\n", i, i));
        }
        fs::write(dir_path.join("many_symbols.ts"), many_symbols)?;

        // 正規表現攻撃（ReDoS）を引き起こす可能性のあるパターン
        let regex_attack = r#"
        function catastrophicBacktracking() {
            // このコメントは正規表現の性能問題を引き起こす可能性がある
            // (a+)+b のようなパターンで aaaaaaaaaaaaaaaaaaaaaa のような入力
            const maliciousString = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
            const anotherString = "((((((((((((((((((((((((((((((((((((((()))))))))))))))))))))))))))))))))))))))";
            return maliciousString + anotherString;
        }
        "#;
        fs::write(dir_path.join("regex_attack.ts"), regex_attack)?;

        Ok(())
    }

    /// シンボリックリンクやハードリンクを含むプロジェクト構造を作成
    fn create_symlink_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // 正常なファイル
        fs::write(
            dir_path.join("target.ts"),
            "function target() { return 'original'; }",
        )?;

        // シンボリックリンクの作成を試行（プラットフォーム依存）
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            // 相対シンボリックリンク
            if symlink("target.ts", dir_path.join("symlink.ts")).is_err() {
                // シンボリックリンク作成に失敗した場合は通常ファイルで代替
                fs::write(dir_path.join("symlink.ts"), "// symlink fallback")?;
            }

            // 循環シンボリックリンクの作成試行
            if symlink("circular_a.ts", dir_path.join("circular_b.ts")).is_err() {
                fs::write(dir_path.join("circular_b.ts"), "// circular fallback")?;
            }
            if symlink("circular_b.ts", dir_path.join("circular_a.ts")).is_err() {
                fs::write(dir_path.join("circular_a.ts"), "// circular fallback")?;
            }

            // 存在しないファイルへのシンボリックリンク（dangling link）
            if symlink("nonexistent.ts", dir_path.join("dangling.ts")).is_err() {
                fs::write(dir_path.join("dangling.ts"), "// dangling fallback")?;
            }
        }

        #[cfg(not(unix))]
        {
            // Windowsでは代替ファイルを作成
            fs::write(dir_path.join("symlink.ts"), "// Windows symlink fallback")?;
            fs::write(
                dir_path.join("circular_a.ts"),
                "// Windows circular fallback a",
            )?;
            fs::write(
                dir_path.join("circular_b.ts"),
                "// Windows circular fallback b",
            )?;
            fs::write(dir_path.join("dangling.ts"), "// Windows dangling fallback")?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_malicious_filenames_safely() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_malicious_files_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        // 悪意のあるファイル名でもクラッシュしないことを確認
        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // インデックシングが完了することを確認
        assert!(
            indexing_duration < Duration::from_secs(30),
            "Should complete within reasonable time"
        );
        assert!(
            !all_symbols.is_empty(),
            "Should extract symbols from valid files"
        );

        // 正常なファイルのシンボルが見つかることを確認
        assert!(
            all_symbols.iter().any(|s| s.name == "normal"),
            "Should find symbols from normal files"
        );

        // ファイル名に関係なく、ファイルの内容からシンボルが抽出されることを確認
        let file_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Filename)
            .collect();
        assert!(
            !file_symbols.is_empty(),
            "Should handle various filename formats"
        );

        // Unicode文字を含むファイル名も適切に処理されることを確認
        assert!(
            all_symbols.iter().any(|s| s.name.contains("unicode")),
            "Should handle Unicode filenames"
        );

        println!(
            "✅ Malicious filename test: {} symbols extracted in {:?}",
            all_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_malicious_file_content_safely() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_malicious_content_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        // 悪意のあるコンテンツでもクラッシュしないことを確認
        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // 合理的な時間内で完了することを確認（DoS攻撃防止）
        assert!(
            indexing_duration < Duration::from_secs(60),
            "Should complete within 60 seconds even with malicious content, took {:?}",
            indexing_duration
        );

        // 正常なファイルからはシンボルが抽出されることを確認
        assert!(
            all_symbols.iter().any(|s| s.name == "normal"),
            "Should find symbols from normal files"
        );

        // 巨大なファイルや深いネストでもメモリ使用量が制限されることを確認
        // （実際のメモリ測定は困難なので、完了することで代替）
        assert!(
            !all_symbols.is_empty(),
            "Should extract some symbols without crashing"
        );

        // 膨大な数のシンボルも適切に処理されることを確認
        let function_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.symbol_type == SymbolType::Function)
            .collect();
        assert!(
            function_symbols.len() > 100,
            "Should handle files with many symbols"
        );

        println!(
            "✅ Malicious content test: {} symbols extracted in {:?}",
            all_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_symlinks_and_special_files_safely() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_symlink_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        // シンボリックリンクでも無限ループに陥らないことを確認
        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // 合理的な時間内で完了することを確認（無限ループ防止）
        assert!(
            indexing_duration < Duration::from_secs(30),
            "Should complete within 30 seconds without infinite loops, took {:?}",
            indexing_duration
        );

        // 元のファイルからはシンボルが抽出されることを確認
        assert!(
            all_symbols.iter().any(|s| s.name == "target"),
            "Should find symbols from target files"
        );

        // シンボリックリンク関連でクラッシュしないことを確認
        assert!(
            !all_symbols.is_empty(),
            "Should extract symbols without crashing on symlinks"
        );

        println!(
            "✅ Symlink safety test: {} symbols extracted in {:?}",
            all_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_validate_search_inputs_safely() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("test.ts"),
            "function testFunc() { return 'test'; }",
        )?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await?;

        let symbols = indexer.get_all_symbols();
        let searcher = FuzzySearcher::new(symbols);

        // 悪意のある検索クエリのテスト
        let long_query = "a".repeat(10_000);
        let malicious_queries = vec![
            "",                              // 空文字列
            " ",                             // スペースのみ
            "\n\r\t",                        // 制御文字
            "\x00\x01\x02",                  // null文字と制御文字
            long_query.as_str(),             // 極めて長い検索クエリ
            "((((((((((((((((((((",          // 不正な正規表現パターン
            "\\",                            // エスケープ文字
            "💩🎉🔥",                        // Unicode絵文字
            "日本語検索",                    // 非ASCII文字
            "' OR '1'='1",                   // SQLインジェクション風
            "<script>alert('xss')</script>", // XSS風
            "../../../etc/passwd",           // パストラバーサル風
        ];

        for query in &malicious_queries {
            let search_start = Instant::now();
            let results = searcher.search(query, &SearchOptions::default());
            let search_duration = search_start.elapsed();

            // すべての検索が合理的な時間内で完了することを確認
            assert!(
                search_duration < Duration::from_millis(1000),
                "Search for '{}' should complete within 1 second, took {:?}",
                query.chars().take(50).collect::<String>(),
                search_duration
            );

            // 検索結果が安全な形式で返されることを確認
            assert!(results.len() <= 1000, "Search results should be limited");

            // 結果の各シンボルが有効であることを確認
            for result in &results {
                assert!(
                    !result.symbol.name.is_empty(),
                    "Symbol names should not be empty"
                );
                assert!(
                    result.symbol.file.exists()
                        || result.symbol.file.to_string_lossy().contains("test.ts"),
                    "Symbol files should exist or be the test file"
                );
                assert!(result.score.is_finite(), "Scores should be finite numbers");
            }
        }

        println!(
            "✅ Search input validation: All {} malicious queries handled safely",
            malicious_queries.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_enforce_resource_limits() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();

        // 大量のファイルを作成してリソース制限をテスト
        for i in 0..100 {
            let content = format!("function func{}() {{ return {}; }}", i, i);
            fs::write(temp_dir.path().join(format!("file{}.ts", i)), content)?;
        }

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // ファイル数に対して合理的な処理時間であることを確認
        let files_per_second = 100.0 / indexing_duration.as_secs_f64();
        assert!(
            files_per_second > 5.0,
            "Should process at least 5 files per second, got {:.2}",
            files_per_second
        );

        // メモリ使用量が合理的であることを間接的に確認
        assert!(
            all_symbols.len() > 100,
            "Should extract substantial number of symbols"
        );
        assert!(
            all_symbols.len() < 10_000,
            "Should not create excessive symbols"
        );

        // 検索性能も合理的であることを確認
        let searcher = FuzzySearcher::new(all_symbols);
        let search_start = Instant::now();
        let results = searcher.search("func", &SearchOptions::default());
        let search_duration = search_start.elapsed();

        assert!(
            search_duration < Duration::from_millis(100),
            "Search should be fast even with many files, took {:?}",
            search_duration
        );
        assert!(!results.is_empty(), "Should find function symbols");

        println!(
            "✅ Resource limits test: {} files processed in {:?}, search took {:?}",
            100, indexing_duration, search_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_filesystem_edge_cases() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // 各種エッジケースのファイル・ディレクトリを作成

        // 空のディレクトリ
        fs::create_dir_all(dir_path.join("empty_dir"))?;

        // 読み取り専用ファイル（権限が許可する場合）
        fs::write(dir_path.join("readonly.ts"), "function readonly() {}")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dir_path.join("readonly.ts"))?.permissions();
            perms.set_mode(0o444); // 読み取り専用
            fs::set_permissions(dir_path.join("readonly.ts"), perms)?;
        }

        // 非常に深いディレクトリ構造
        let mut deep_path = dir_path.to_path_buf();
        for i in 0..50 {
            deep_path.push(format!("level{}", i));
        }
        fs::create_dir_all(&deep_path)?;
        fs::write(deep_path.join("deep.ts"), "function deep() {}")?;

        // 非常に短いファイル名
        fs::write(dir_path.join("a.ts"), "function a() {}")?;

        // 拡張子なしファイル（TypeScriptパターンにマッチしない）
        fs::write(dir_path.join("noext"), "function noext() {}")?;

        // 0バイトファイル
        fs::write(dir_path.join("empty.ts"), "")?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        // エッジケースでもクラッシュしないことを確認
        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // 合理的な時間内で完了することを確認
        assert!(
            indexing_duration < Duration::from_secs(30),
            "Should handle filesystem edge cases within 30 seconds, took {:?}",
            indexing_duration
        );

        // 有効なファイルからはシンボルが抽出されることを確認
        assert!(
            all_symbols.iter().any(|s| s.name == "readonly"),
            "Should find readonly files"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "deep"),
            "Should find deeply nested files"
        );
        assert!(
            all_symbols
                .iter()
                .any(|s| s.name == "a" || s.name == "a.ts"),
            "Should find short filename files"
        );

        // 0バイトファイルでもクラッシュしないことを確認
        let _empty_file_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.file.to_string_lossy().contains("empty.ts"))
            .collect();
        // 0バイトファイルからはシンボルは抽出されないが、ファイル名は抽出される可能性

        println!(
            "✅ Filesystem edge cases test: {} symbols extracted in {:?}",
            all_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_concurrent_access_safely() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();

        // テスト用ファイルを作成
        for i in 0..20 {
            let content = format!("function concurrentFunc{}() {{ return {}; }}", i, i);
            fs::write(temp_dir.path().join(format!("concurrent{}.ts", i)), content)?;
        }

        // 複数のインデクサーを同時に実行
        let temp_path = temp_dir.path().to_path_buf();
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let path = temp_path.clone();
                tokio::spawn(async move {
                    let mut indexer = TreeSitterIndexer::with_verbose(false);
                    indexer.initialize().await.unwrap();

                    let patterns = vec!["**/*.ts".to_string()];
                    let start = Instant::now();

                    indexer.index_directory(&path, &patterns).await.unwrap();
                    let duration = start.elapsed();

                    let symbols = indexer.get_all_symbols();
                    (i, symbols.len(), duration)
                })
            })
            .collect();

        // すべてのタスクの完了を待つ
        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }

        // すべてのインデクサーが正常に完了したことを確認
        assert_eq!(results.len(), 5, "All concurrent indexers should complete");

        // 結果の一貫性を確認
        let symbol_counts: Vec<usize> = results.iter().map(|(_, count, _)| *count).collect();
        let first_count = symbol_counts[0];

        for (i, count) in symbol_counts.iter().enumerate() {
            assert_eq!(
                *count, first_count,
                "Concurrent indexer {} should produce consistent results: {} vs {}",
                i, count, first_count
            );
        }

        // 合理的な性能を確認
        for (i, _, duration) in &results {
            assert!(
                duration < &Duration::from_secs(10),
                "Concurrent indexer {} should complete within 10 seconds, took {:?}",
                i,
                duration
            );
        }

        println!(
            "✅ Concurrent access test: {} indexers completed with consistent results",
            results.len()
        );

        Ok(())
    }
}
