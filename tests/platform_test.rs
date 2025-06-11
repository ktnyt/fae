// プラットフォーム互換性テストスイート
// Windows/Linux/macOSでの動作保証とクロスプラットフォーム機能検証

use sfs::indexer::TreeSitterIndexer;
use sfs::searcher::FuzzySearcher;
use sfs::types::*;
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[cfg(test)]
mod platform_compatibility_tests {
    use super::*;

    /// プラットフォーム固有のパス区切り文字を含むファイル構造を作成
    fn create_cross_platform_project(dir: &TempDir) -> anyhow::Result<()> {
        let dir_path = dir.path();

        // 複数階層のディレクトリ構造（パス区切り文字のテスト）
        fs::create_dir_all(dir_path.join("src").join("components").join("ui"))?;
        fs::create_dir_all(dir_path.join("src").join("utils").join("platform"))?;
        fs::create_dir_all(dir_path.join("tests").join("integration"))?;

        // Windows風のパスを含むファイル（共通テスト）
        fs::write(
            dir_path.join("src/components/ui/Button.tsx"),
            r#"
// UI Component with Windows-style comments
export interface ButtonProps {
    /** Windows compat: onClick handler */
    onClick?: () => void;
    /** Linux compat: className for styling */
    className?: string;
    /** macOS compat: children content */
    children: React.ReactNode;
}

export const Button: React.FC<ButtonProps> = ({ onClick, className, children }) => {
    // Platform-agnostic event handling
    const handleClick = () => {
        if (onClick) {
            onClick();
        }
    };

    return (
        <button 
            className={className}
            onClick={handleClick}
            type="button"
        >
            {children}
        </button>
    );
};
"#,
        )?;

        // Unix系のパーミッション関連コード
        fs::write(
            dir_path.join("src/utils/platform/fileUtils.ts"),
            r#"
// Cross-platform file utilities
import { promises as fs } from 'fs';
import { join } from 'path';

export class FileUtils {
    /** Check if file is executable (Unix-specific but should work on all platforms) */
    static async isExecutable(filePath: string): Promise<boolean> {
        try {
            const stats = await fs.stat(filePath);
            // Platform-specific logic
            if (process.platform === 'win32') {
                return filePath.endsWith('.exe') || filePath.endsWith('.bat');
            }
            return (stats.mode & parseInt('111', 8)) !== 0;
        } catch {
            return false;
        }
    }

    /** Get platform-specific path separator */
    static getPathSeparator(): string {
        return process.platform === 'win32' ? '\\' : '/';
    }

    /** Normalize path for current platform */
    static normalizePath(inputPath: string): string {
        return inputPath.replace(/[\\/]/g, this.getPathSeparator());
    }

    /** Get platform-specific temp directory */
    static getTempDir(): string {
        return process.platform === 'win32' ? 'C:\\temp' : '/tmp';
    }

    /** Platform-specific line ending handling */
    static normalizeLineEndings(content: string): string {
        if (process.platform === 'win32') {
            return content.replace(/\n/g, '\r\n');
        }
        return content.replace(/\r\n/g, '\n');
    }
}
"#,
        )?;

        // 各プラットフォームでの特殊文字ファイル名テスト
        // Windows: 特殊文字制限
        fs::write(
            dir_path.join("windows_compat.ts"),
            r#"
// Windows compatibility test file
const WINDOWS_RESERVED_NAMES = ['CON', 'PRN', 'AUX', 'NUL'];
const WINDOWS_SPECIAL_CHARS = ['<', '>', ':', '"', '|', '?', '*'];

export class WindowsCompatibility {
    static isValidWindowsFileName(name: string): boolean {
        return !WINDOWS_RESERVED_NAMES.includes(name.toUpperCase()) &&
               !WINDOWS_SPECIAL_CHARS.some(char => name.includes(char));
    }
}
"#,
        )?;

        // Linux/Unix: 長いファイル名とUnicode
        let long_filename = "a".repeat(200) + ".ts";
        fs::write(
            dir_path.join(long_filename),
            r#"
// Long filename test for Unix systems
export const UNIX_MAX_FILENAME_LENGTH = 255;
export const UNIX_MAX_PATH_LENGTH = 4096;

export class UnixCompatibility {
    static checkPathLimits(path: string): boolean {
        return path.length <= UNIX_MAX_PATH_LENGTH;
    }
}
"#,
        )?;

        // macOS: 正規化されたUnicode文字
        fs::write(
            dir_path.join("macOS_unicode_test_café.ts"),
            r#"
// macOS Unicode normalization test
// This file tests NFD (decomposed) vs NFC (composed) Unicode handling
export class MacOSUnicodeHandler {
    static normalizeUnicode(text: string): string {
        // macOS file system uses NFD (decomposed) normalization
        return text.normalize('NFD');
    }

    static compareUnicode(text1: string, text2: string): boolean {
        return this.normalizeUnicode(text1) === this.normalizeUnicode(text2);
    }
}
"#,
        )?;

        // クロスプラットフォーム設定ファイル
        fs::write(
            dir_path.join("platform.config.json"),
            r#"{
    "platforms": {
        "windows": {
            "pathSeparator": "\\",
            "lineEnding": "\r\n",
            "caseSensitive": false,
            "maxPathLength": 260,
            "reservedNames": ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1"]
        },
        "linux": {
            "pathSeparator": "/",
            "lineEnding": "\n",
            "caseSensitive": true,
            "maxPathLength": 4096,
            "maxFilename": 255
        },
        "macos": {
            "pathSeparator": "/",
            "lineEnding": "\n",
            "caseSensitive": false,
            "maxPathLength": 1024,
            "unicodeNormalization": "NFD"
        }
    }
}"#,
        )?;

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_platform_specific_path_separators() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec![
            "**/*.ts".to_string(),
            "**/*.tsx".to_string(),
            "**/*.json".to_string(),
        ];
        let start_time = Instant::now();

        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // 基本性能確認
        assert!(
            indexing_duration < Duration::from_secs(10),
            "Should handle cross-platform paths efficiently, took {:?}",
            indexing_duration
        );
        assert!(
            !all_symbols.is_empty(),
            "Should extract symbols from cross-platform project"
        );

        // プラットフォーム固有のシンボル確認
        assert!(
            all_symbols.iter().any(|s| s.name == "Button"),
            "Should find Button component"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "FileUtils"),
            "Should find FileUtils class"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "WindowsCompatibility"),
            "Should find Windows compat class"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "UnixCompatibility"),
            "Should find Unix compat class"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "MacOSUnicodeHandler"),
            "Should find macOS Unicode handler"
        );

        // パスの正規化確認
        for symbol in &all_symbols {
            let path_str = symbol.file.to_string_lossy();
            // すべてのパスが有効であることを確認
            assert!(
                symbol.file.exists() || path_str.contains("temp"),
                "All symbol paths should be valid: {}",
                path_str
            );
        }

        println!(
            "✅ Platform-specific paths: {} symbols indexed in {:?}",
            all_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_unicode_filenames_across_platforms() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await?;

        let all_symbols = indexer.get_all_symbols();

        // Unicode ファイル名のシンボルが見つかることを確認
        let unicode_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| {
                s.file.to_string_lossy().contains("café")
                    || s.file.to_string_lossy().contains("unicode")
            })
            .collect();

        assert!(
            !unicode_symbols.is_empty(),
            "Should handle Unicode filenames"
        );

        // macOS Unicode 正規化関連のシンボル確認
        assert!(
            all_symbols.iter().any(|s| s.name == "MacOSUnicodeHandler"),
            "Should find macOS Unicode handler"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "normalizeUnicode"),
            "Should find Unicode normalization methods"
        );

        println!(
            "✅ Unicode filenames: {} symbols, {} Unicode files",
            all_symbols.len(),
            unicode_symbols.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_respect_platform_file_system_limits() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        let start_time = Instant::now();

        indexer.index_directory(temp_dir.path(), &patterns).await?;
        let indexing_duration = start_time.elapsed();

        let all_symbols = indexer.get_all_symbols();

        // 長いファイル名のファイルも適切に処理されることを確認
        let long_filename_symbols: Vec<_> = all_symbols
            .iter()
            .filter(|s| {
                s.file
                    .file_name()
                    .map(|name| name.to_string_lossy().len() > 100)
                    .unwrap_or(false)
            })
            .collect();

        assert!(
            !long_filename_symbols.is_empty(),
            "Should handle long filenames"
        );

        // プラットフォーム制限のシンボル確認
        assert!(
            all_symbols
                .iter()
                .any(|s| s.name == "UNIX_MAX_FILENAME_LENGTH"),
            "Should find Unix limits constants"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "checkPathLimits"),
            "Should find path limit checking functions"
        );

        // 性能が適切であることを確認
        assert!(
            indexing_duration < Duration::from_secs(5),
            "Should handle file system limits efficiently"
        );

        println!(
            "✅ File system limits: {} symbols, {} long filenames in {:?}",
            all_symbols.len(),
            long_filename_symbols.len(),
            indexing_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_case_sensitivity_differences() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        // 大文字小文字の異なるファイルを作成（プラットフォーム依存）
        let test_files = vec![
            ("CaseTest.ts", "export class CaseTestUpper {}"),
            ("casetest.ts", "export class CaseTestLower {}"), // Unix: 別ファイル, Windows: 上書き
        ];

        for (filename, content) in test_files {
            let file_path = temp_dir.path().join(filename);
            if fs::write(&file_path, content).is_err() {
                // ファイル作成に失敗した場合（Windows等で大文字小文字の重複）はスキップ
                println!(
                    "⚠️  Skipped creating {} due to platform case sensitivity",
                    filename
                );
            }
        }

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await?;

        let all_symbols = indexer.get_all_symbols();
        let searcher = FuzzySearcher::new(all_symbols.clone());

        // 大文字小文字の異なる検索
        let upper_results = searcher.search("CaseTest", &SearchOptions::default());
        let lower_results = searcher.search("casetest", &SearchOptions::default());

        // どちらの検索でも何らかの結果が得られることを確認
        assert!(
            !upper_results.is_empty() || !lower_results.is_empty(),
            "Should handle case sensitivity search variations"
        );

        // プラットフォーム固有のクラスが見つかることを確認
        let case_classes: Vec<_> = all_symbols
            .iter()
            .filter(|s| s.name.contains("CaseTest"))
            .collect();

        assert!(!case_classes.is_empty(), "Should find case test classes");

        println!(
            "✅ Case sensitivity: {} symbols, {} case test classes",
            all_symbols.len(),
            case_classes.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_platform_specific_line_endings() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();

        // 各プラットフォームの改行コードでファイルを作成
        let unix_content =
            "export class UnixLineEndings {\n    method() {\n        return 'unix';\n    }\n}";
        let windows_content = "export class WindowsLineEndings {\r\n    method() {\r\n        return 'windows';\r\n    }\r\n}";
        let mixed_content = "export class MixedLineEndings {\n    windowsMethod() {\r\n        return 'mixed';\n    }\r\n}";

        fs::write(temp_dir.path().join("unix_lines.ts"), unix_content)?;
        fs::write(temp_dir.path().join("windows_lines.ts"), windows_content)?;
        fs::write(temp_dir.path().join("mixed_lines.ts"), mixed_content)?;

        let mut indexer = TreeSitterIndexer::with_verbose(false);
        indexer.initialize().await.unwrap();

        let patterns = vec!["**/*.ts".to_string()];
        indexer.index_directory(temp_dir.path(), &patterns).await?;

        let all_symbols = indexer.get_all_symbols();

        // 各改行コードのファイルからシンボルが抽出されることを確認
        assert!(
            all_symbols.iter().any(|s| s.name == "UnixLineEndings"),
            "Should handle Unix line endings"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "WindowsLineEndings"),
            "Should handle Windows line endings"
        );
        assert!(
            all_symbols.iter().any(|s| s.name == "MixedLineEndings"),
            "Should handle mixed line endings"
        );

        // 改行コードに関係なく適切な行番号が取得されることを確認
        for symbol in &all_symbols {
            assert!(
                symbol.line > 0,
                "Line numbers should be positive: {}",
                symbol.line
            );
            // Column is u32, so always non-negative by type definition
        }

        println!(
            "✅ Line endings: {} symbols from files with different line endings",
            all_symbols.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_work_consistently_across_platforms() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        // 複数回のインデックシングで一貫した結果が得られることを確認
        let mut results = Vec::new();

        for i in 0..3 {
            let mut indexer = TreeSitterIndexer::with_verbose(false);
            indexer.initialize().await.unwrap();

            let patterns = vec!["**/*.ts".to_string(), "**/*.tsx".to_string()];
            let start_time = Instant::now();

            indexer.index_directory(temp_dir.path(), &patterns).await?;
            let duration = start_time.elapsed();

            let symbols = indexer.get_all_symbols();
            results.push((symbols.len(), duration));

            println!(
                "🔄 Run {}: {} symbols in {:?}",
                i + 1,
                symbols.len(),
                duration
            );
        }

        // 結果の一貫性を確認
        let first_count = results[0].0;
        for (i, (count, _duration)) in results.iter().enumerate() {
            assert_eq!(
                *count,
                first_count,
                "Run {} should produce consistent results: {} vs {}",
                i + 1,
                count,
                first_count
            );
        }

        // 性能の安定性を確認
        let durations: Vec<Duration> = results.iter().map(|(_, d)| *d).collect();
        let avg_duration = durations.iter().sum::<Duration>() / durations.len() as u32;

        for (i, duration) in durations.iter().enumerate() {
            let variance = if *duration > avg_duration {
                duration.as_millis() as f64 / avg_duration.as_millis() as f64
            } else {
                avg_duration.as_millis() as f64 / duration.as_millis() as f64
            };

            assert!(
                variance < 3.0,
                "Run {} performance should be stable: {:?} vs avg {:?} ({}x variance)",
                i + 1,
                duration,
                avg_duration,
                variance
            );
        }

        println!(
            "✅ Consistency: {} runs with stable {} symbols and avg {:?}",
            results.len(),
            first_count,
            avg_duration
        );

        Ok(())
    }

    #[tokio::test]
    async fn should_handle_concurrent_access_on_current_platform() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().unwrap();
        create_cross_platform_project(&temp_dir)?;

        // プラットフォーム固有の並行アクセステスト
        let temp_path = temp_dir.path().to_path_buf();
        let handles: Vec<_> = (0..3)
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

        // 並行処理での一貫性確認
        assert_eq!(results.len(), 3, "All concurrent tasks should complete");

        let symbol_counts: Vec<usize> = results.iter().map(|(_, count, _)| *count).collect();
        let first_count = symbol_counts[0];

        for (i, count) in symbol_counts.iter().enumerate() {
            assert_eq!(
                *count, first_count,
                "Concurrent task {} should produce consistent results: {} vs {}",
                i, count, first_count
            );
        }

        // プラットフォーム固有の性能確認
        for (i, _, duration) in &results {
            assert!(
                duration < &Duration::from_secs(10),
                "Concurrent task {} should complete within reasonable time: {:?}",
                i,
                duration
            );
        }

        println!(
            "✅ Concurrent access: {} tasks with consistent {} symbols",
            results.len(),
            first_count
        );

        Ok(())
    }
}
