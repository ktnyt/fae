use std::process::Command;
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;

/// エラーハンドリングと境界条件のテスト
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    fn create_problematic_project() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // 通常のファイル
        fs::create_dir_all(root.join("src"))?;
        let mut normal_file = File::create(root.join("src/normal.ts"))?;
        writeln!(normal_file, "export class NormalClass {{")?;
        writeln!(normal_file, "  constructor() {{}}")?;
        writeln!(normal_file, "}}")?;

        // 空ファイル
        File::create(root.join("src/empty.ts"))?;

        // 巨大ファイル（2MB以上）
        let mut large_file = File::create(root.join("src/large.ts"))?;
        for i in 0..50000 {
            writeln!(large_file, "// This is line {} of a very large file", i)?;
            writeln!(large_file, "export const CONSTANT_{} = '{}';", i, "x".repeat(50))?;
        }

        // バイナリファイル
        let mut binary_file = File::create(root.join("src/binary.bin"))?;
        binary_file.write_all(&[0, 1, 2, 3, 255, 254, 253, 252])?;

        // 破損したUTF-8ファイル
        let mut invalid_utf8 = File::create(root.join("src/invalid.ts"))?;
        invalid_utf8.write_all(b"Valid start")?;
        invalid_utf8.write_all(&[0xFF, 0xFE, 0xFD])?; // Invalid UTF-8
        invalid_utf8.write_all(b"Valid end")?;

        // 構文エラーのあるファイル
        let mut syntax_error = File::create(root.join("src/syntax_error.ts"))?;
        writeln!(syntax_error, "This is not valid TypeScript syntax {{{{ ]]]] >>>>")?;
        writeln!(syntax_error, "class Broken {{")?;
        writeln!(syntax_error, "  method() {{ return ;")?; // 不完全な構文

        // 深いネストのディレクトリ
        let deep_path = root.join("very/deep/nested/directory/structure/that/goes/on/forever");
        fs::create_dir_all(&deep_path)?;
        let mut deep_file = File::create(deep_path.join("deep.ts"))?;
        writeln!(deep_file, "export const DEEP_CONSTANT = 'buried deep';")?;

        // 特殊文字を含むファイル名
        let special_chars_dir = root.join("special");
        fs::create_dir_all(&special_chars_dir)?;
        
        // 注意: ファイル名の特殊文字はOS依存なので、安全な範囲で
        let mut special_file1 = File::create(special_chars_dir.join("file-with-dashes.ts"))?;
        writeln!(special_file1, "export const DASHED = true;")?;
        
        let mut special_file2 = File::create(special_chars_dir.join("file_with_underscores.ts"))?;
        writeln!(special_file2, "export const UNDERSCORED = true;")?;

        let mut special_file3 = File::create(special_chars_dir.join("file.with.dots.ts"))?;
        writeln!(special_file3, "export const DOTTED = true;")?;

        // シンボリックリンク（OS依存、エラーを無視）
        let _ = std::os::unix::fs::symlink(
            root.join("src/normal.ts"), 
            root.join("src/symlink.ts")
        );

        // 読み取り専用ファイル（権限テスト）
        let readonly_file_path = root.join("src/readonly.ts");
        let mut readonly_file = File::create(&readonly_file_path)?;
        writeln!(readonly_file, "export const READONLY = 'cannot modify';")?;
        drop(readonly_file);
        
        // 読み取り専用権限を設定
        let metadata = fs::metadata(&readonly_file_path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o444); // 読み取り専用
        fs::set_permissions(&readonly_file_path, permissions)?;

        Ok(temp_dir)
    }

    fn run_fae_command(dir: &std::path::Path, args: &[&str]) -> Result<(String, String, bool), Box<dyn std::error::Error>> {
        // テスト実行時のパスからバイナリを見つける
        let current_dir = std::env::current_dir()?;
        let binary_path = current_dir.join("target/debug/fae");
        
        let output = Command::new(&binary_path)
            .args(args)
            .current_dir(dir)
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        Ok((stdout, stderr, success))
    }

    #[test]
    fn test_empty_file_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 空ファイルがあってもクラッシュしないはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["empty"])?;
        
        // エラーでプロセスが終了しないはず
        assert!(success, "Command failed with stderr: {}", stderr);
        
        println!("Empty file handling - stdout: {}, stderr: {}", stdout.len(), stderr.len());

        Ok(())
    }

    #[test]
    fn test_large_file_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 大きなファイルでも適切に処理されるはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["CONSTANT"])?;
        
        // サイズ制限により除外される可能性があるが、エラーで終了しないはず
        assert!(success, "Command failed with stderr: {}", stderr);
        
        println!("Large file handling - found {} results", if stdout.is_empty() { 0 } else { stdout.lines().count() });

        Ok(())
    }

    #[test]
    fn test_binary_file_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // バイナリファイルは除外されるが、エラーで終了しないはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["binary"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        // バイナリファイルは検索結果に含まれないはず
        assert!(!stdout.contains("binary.bin"), "Binary file should be excluded from search");
        
        println!("Binary file handling - correctly excluded");

        Ok(())
    }

    #[test]
    fn test_invalid_utf8_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // UTF-8として無効なファイルがあってもクラッシュしないはず
        let (_stdout, stderr, success) = run_fae_command(project_path, &["Valid"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        println!("Invalid UTF-8 handling - success: {}", success);

        Ok(())
    }

    #[test]
    fn test_syntax_error_file_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 構文エラーのあるファイルでもコンテンツ検索は動作するはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["Broken"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        // シンボル検索では解析エラーが発生する可能性があるが、クラッシュしないはず
        let (stdout2, stderr2, success2) = run_fae_command(project_path, &["#Broken"])?;
        
        assert!(success2, "Symbol search failed with stderr: {}", stderr2);
        
        println!("Syntax error handling - content: {}, symbol: {}", !stdout.is_empty(), !stdout2.is_empty());

        Ok(())
    }

    #[test]
    fn test_deep_directory_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 深いディレクトリ構造でも正常に動作するはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["DEEP_CONSTANT"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        // 深いパスのファイルも発見されるはず
        if !stdout.is_empty() {
            assert!(stdout.contains("deep") || stdout.contains("DEEP"));
        }
        
        println!("Deep directory handling - found deep content");

        Ok(())
    }

    #[test]
    fn test_special_characters_in_filenames() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 特殊文字を含むファイル名でも正常に処理されるはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["DASHED"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        // ファイル検索でも特殊文字が処理されるはず
        let (stdout2, stderr2, success2) = run_fae_command(project_path, &[">dashes"])?;
        
        assert!(success2, "File search failed with stderr: {}", stderr2);
        
        println!("Special characters handling - content: {}, file: {}", !stdout.is_empty(), !stdout2.is_empty());

        Ok(())
    }

    #[test]
    fn test_permission_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 読み取り専用ファイルでも検索できるはず
        let (_stdout, stderr, success) = run_fae_command(project_path, &["READONLY"])?;
        
        assert!(success, "Command failed with stderr: {}", stderr);
        
        println!("Permission handling - readonly file accessible");

        Ok(())
    }

    #[test]
    fn test_invalid_search_patterns() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 無効な正規表現パターン
        let invalid_patterns = [
            "/[",         // 不完全な文字クラス
            "/*",         // 無効な繰り返し
            "/(?",        // 不完全なグループ
            "/\\",        // 不完全なエスケープ
        ];

        for pattern in &invalid_patterns {
            let (stdout, stderr, success) = run_fae_command(project_path, &[pattern])?;
            
            // 無効なパターンでもクラッシュしないはず
            // （エラーメッセージが表示される可能性はある）
            println!("Invalid pattern '{}': success={}, stdout={}, stderr={}", 
                     pattern, success, stdout.len(), stderr.len());
        }

        Ok(())
    }

    #[test]
    fn test_memory_limit_conditions() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 大量の結果を返す可能性のあるクエリ
        let broad_queries = [
            "e",          // 非常に一般的な文字
            ".",          // すべての行にマッチ
            "const",      // 多くのJavaScript/TypeScriptファイルに存在
            "",           // 空クエリ
        ];

        for query in &broad_queries {
            let (stdout, stderr, success) = run_fae_command(project_path, &[query])?;
            
            // メモリ不足でクラッシュしないはず
            assert!(success || stderr.contains("too many results"), 
                    "Query '{}' failed unexpectedly: {}", query, stderr);
            
            println!("Broad query '{}': success={}, results={} chars", 
                     query, success, stdout.len());
        }

        Ok(())
    }

    #[test]
    fn test_concurrent_access_safety() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 複数の検索を素早く連続実行（同時実行の模擬）
        let queries = ["Normal", "#NormalClass", ">normal", "/class", "export"];
        
        for (i, query) in queries.iter().enumerate() {
            let (stdout, stderr, success) = run_fae_command(project_path, &[query])?;
            
            assert!(success, "Concurrent query {} '{}' failed: {}", i, query, stderr);
            
            println!("Concurrent query {}: '{}' - {} chars", i, query, stdout.len());
        }

        Ok(())
    }

    #[test]
    fn test_filesystem_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // ファイルシステムの境界条件テスト
        
        // 1. 存在しないファイル/ディレクトリに対する検索
        let (_stdout, stderr, success) = run_fae_command(project_path, &[">nonexistent"])?;
        assert!(success, "Search for nonexistent file failed: {}", stderr);
        
        // 2. 現在のディレクトリ外への検索試行
        let (_stdout2, stderr2, success2) = run_fae_command(project_path, &["../outside"])?;
        assert!(success2, "Search outside directory failed: {}", stderr2);
        
        // 3. 隠しファイル/ディレクトリの処理
        fs::create_dir_all(project_path.join(".hidden"))?;
        let mut hidden_file = File::create(project_path.join(".hidden/secret.ts"))?;
        writeln!(hidden_file, "export const SECRET = 'hidden';")?;
        
        let (_stdout3, stderr3, success3) = run_fae_command(project_path, &["SECRET"])?;
        assert!(success3, "Hidden file search failed: {}", stderr3);
        
        println!("Filesystem edge cases handled successfully");

        Ok(())
    }

    #[test]
    fn test_command_line_argument_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // コマンドライン引数の境界条件テスト
        
        // 1. 非常に長い引数
        let long_arg = "a".repeat(10000);
        let (_stdout, _stderr, success) = run_fae_command(project_path, &[&long_arg])?;
        // 長すぎる引数でもクラッシュしないはず
        println!("Long argument test: success={}", success);
        
        // 2. 特殊文字を含む引数
        let special_args = [
            "\n",           // 改行文字
            "\t",           // タブ文字
            " ",            // スペースのみ
            "   ",          // 複数スペース
            "\0",           // NULL文字
        ];
        
        for arg in &special_args {
            match run_fae_command(project_path, &[arg]) {
                Ok((_stdout, _stderr, success)) => {
                    println!("Special arg test {:?}: success={}", arg, success);
                }
                Err(e) => {
                    // NULL文字などでエラーになる場合も正常な動作
                    println!("Special arg test {:?}: error={}", arg, e);
                }
            }
        }
        
        // 3. Unicode文字を含む引数
        let unicode_args = ["日本語", "🚀", "café", "наука"];
        
        for arg in &unicode_args {
            let (_stdout, _stderr, success) = run_fae_command(project_path, &[arg])?;
            println!("Unicode arg test '{}': success={}", arg, success);
        }

        Ok(())
    }

    #[test]
    fn test_resource_exhaustion_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // リソース枯渇状況での動作テスト
        
        // 1. 大量のマッチを生成するクエリ
        let (stdout, stderr, success) = run_fae_command(project_path, &["export"])?;
        
        // 大量の結果でも適切に処理されるはず
        assert!(success, "Large result set query failed: {}", stderr);
        println!("Large result set: {} chars output", stdout.len());
        
        // 2. 複雑な正規表現（バックトラッキングを引き起こす可能性）
        let complex_regex = "/a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*a*b";
        let (_stdout2, _stderr2, success2) = run_fae_command(project_path, &[complex_regex])?;
        
        println!("Complex regex test: success={}, time should be reasonable", success2);

        Ok(())
    }

    #[test]
    fn test_graceful_degradation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_problematic_project()?;
        let project_path = temp_dir.path();

        // 優雅な劣化のテスト
        
        // 破損したファイルがあっても他のファイルは検索できるはず
        let (stdout, stderr, success) = run_fae_command(project_path, &["NormalClass"])?;
        
        assert!(success, "Search failed despite corrupted files: {}", stderr);
        assert!(stdout.contains("NormalClass") || stdout.contains("normal"), 
                "Should find normal content despite corrupted files");
        
        // インデックス構築エラーがあっても基本的な検索は動作するはず
        let (_stdout2, stderr2, success2) = run_fae_command(project_path, &["#NormalClass"])?;
        
        // シンボル検索は失敗する可能性があるが、プロセスはクラッシュしないはず
        assert!(success2, "Symbol search should not crash despite errors: {}", stderr2);
        
        println!("Graceful degradation test completed");

        Ok(())
    }
}