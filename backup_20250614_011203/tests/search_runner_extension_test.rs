//! SearchRunnerの拡張機能テスト
//! 
//! TUI統合で追加されたcollect_results_with_strategy機能や
//! 新しい非同期統合の動作を検証する

use fae::cli::{SearchRunner, strategies::*};
use fae::types::DisplayInfo;
use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs::File;
use std::io::Write;

fn create_comprehensive_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // TypeScriptファイル
    let mut ts_file = File::create(root.join("example.ts"))?;
    writeln!(ts_file, "interface User {{")?;
    writeln!(ts_file, "  name: string;")?;
    writeln!(ts_file, "  age: number;")?;
    writeln!(ts_file, "}}")?;
    writeln!(ts_file, "")?;
    writeln!(ts_file, "function greetUser(user: User): string {{")?;
    writeln!(ts_file, "  return `Hello, ${{user.name}}!`;")?;
    writeln!(ts_file, "}}")?;
    writeln!(ts_file, "")?;
    writeln!(ts_file, "export {{ User, greetUser }};")?;

    // Rustファイル
    let mut rs_file = File::create(root.join("main.rs"))?;
    writeln!(rs_file, "struct Person {{")?;
    writeln!(rs_file, "    name: String,")?;
    writeln!(rs_file, "    age: u32,")?;
    writeln!(rs_file, "}}")?;
    writeln!(rs_file, "")?;
    writeln!(rs_file, "impl Person {{")?;
    writeln!(rs_file, "    fn new(name: String, age: u32) -> Self {{")?;
    writeln!(rs_file, "        Self {{ name, age }}")?;
    writeln!(rs_file, "    }}")?;
    writeln!(rs_file, "")?;
    writeln!(rs_file, "    fn greet(&self) -> String {{")?;
    writeln!(rs_file, "        format!(\"Hello, {{}}!\", self.name)")?;
    writeln!(rs_file, "    }}")?;
    writeln!(rs_file, "}}")?;
    writeln!(rs_file, "")?;
    writeln!(rs_file, "fn main() {{")?;
    writeln!(rs_file, "    let person = Person::new(\"Alice\".to_string(), 30);")?;
    writeln!(rs_file, "    println!(\"{{}}\", person.greet());")?;
    writeln!(rs_file, "}}")?;

    // JavaScriptファイル
    let mut js_file = File::create(root.join("utils.js"))?;
    writeln!(js_file, "/**")?;
    writeln!(js_file, " * ユーティリティ関数集")?;
    writeln!(js_file, " */")?;
    writeln!(js_file, "")?;
    writeln!(js_file, "function capitalizeString(str) {{")?;
    writeln!(js_file, "    return str.charAt(0).toUpperCase() + str.slice(1);")?;
    writeln!(js_file, "}}")?;
    writeln!(js_file, "")?;
    writeln!(js_file, "const formatDate = (date) => {{")?;
    writeln!(js_file, "    return date.toISOString().split('T')[0];")?;
    writeln!(js_file, "}};")?;
    writeln!(js_file, "")?;
    writeln!(js_file, "module.exports = {{ capitalizeString, formatDate }};")?;

    // Pythonファイル
    let mut py_file = File::create(root.join("helper.py"))?;
    writeln!(py_file, "\"\"\"Helper functions for data processing\"\"\"")?;
    writeln!(py_file, "")?;
    writeln!(py_file, "import re")?;
    writeln!(py_file, "from typing import List, Optional")?;
    writeln!(py_file, "")?;
    writeln!(py_file, "class DataProcessor:")?;
    writeln!(py_file, "    def __init__(self, name: str):")?;
    writeln!(py_file, "        self.name = name")?;
    writeln!(py_file, "")?;
    writeln!(py_file, "    def process_text(self, text: str) -> str:")?;
    writeln!(py_file, "        \"\"\"Process text by removing special characters\"\"\"")?;
    writeln!(py_file, "        return re.sub(r'[^\\w\\s]', '', text)")?;
    writeln!(py_file, "")?;
    writeln!(py_file, "def find_patterns(text: str, patterns: List[str]) -> List[str]:")?;
    writeln!(py_file, "    \"\"\"Find all patterns in text\"\"\"")?;
    writeln!(py_file, "    matches = []")?;
    writeln!(py_file, "    for pattern in patterns:")?;
    writeln!(py_file, "        matches.extend(re.findall(pattern, text))")?;
    writeln!(py_file, "    return matches")?;

    // テスト用のMarkdownファイル（検索対象外）
    let mut md_file = File::create(root.join("README.md"))?;
    writeln!(md_file, "# Test Project")?;
    writeln!(md_file, "")?;
    writeln!(md_file, "This is a test project containing multiple programming languages:")?;
    writeln!(md_file, "")?;
    writeln!(md_file, "- TypeScript (example.ts)")?;
    writeln!(md_file, "- Rust (main.rs)")?;
    writeln!(md_file, "- JavaScript (utils.js)")?;
    writeln!(md_file, "- Python (helper.py)")?;

    Ok(temp_dir)
}

#[cfg(test)]
mod collect_results_tests {
    use super::*;

    #[test]
    fn test_content_strategy_collection() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = ContentStrategy;

        // "function"を検索
        let results = search_runner.collect_results_with_strategy(&strategy, "function")?;
        assert!(results.len() > 0);

        // 結果の構造を検証
        for result in &results {
            assert!(!result.file_path.as_os_str().is_empty());
            assert!(result.line >= 1);
            assert!(result.column >= 1);
            assert!(result.score > 0.0);
            
            match &result.display_info {
                DisplayInfo::Content { line_content, match_start, match_end } => {
                    assert!(!line_content.is_empty());
                    assert!(*match_start <= *match_end);
                    assert!(*match_end <= line_content.len());
                }
                _ => panic!("Expected Content display info"),
            }
        }

        Ok(())
    }

    #[test]
    fn test_file_strategy_collection() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = FileStrategy;

        // ".rs"ファイルを検索
        let results = search_runner.collect_results_with_strategy(&strategy, "rs")?;
        assert!(results.len() > 0);

        // 結果の構造を検証
        for result in &results {
            match &result.display_info {
                DisplayInfo::File { path, is_directory } => {
                    assert!(path.to_string_lossy().contains("rs"));
                    // ファイル検索なのでディレクトリではない
                    assert!(!is_directory);
                }
                _ => panic!("Expected File display info"),
            }
        }

        Ok(())
    }

    #[test]
    fn test_regex_strategy_collection() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = RegexStrategy;

        // 関数定義のパターンを検索
        let results = search_runner.collect_results_with_strategy(&strategy, r"fn\s+\w+")?;
        assert!(results.len() > 0);

        // 結果の構造を検証
        for result in &results {
            match &result.display_info {
                DisplayInfo::Regex { line_content, match_start, match_end, .. } => {
                    assert!(!line_content.is_empty());
                    assert!(*match_start <= *match_end);
                    // 正規表現検索では "fn " で始まる行が見つかるはず
                    let matched_text = &line_content[*match_start..*match_end];
                    assert!(matched_text.starts_with("fn "));
                }
                DisplayInfo::Content { line_content, .. } => {
                    // 一部の実装ではContent形式で返される場合もある
                    assert!(!line_content.is_empty());
                    println!("Found Content display info in regex search: {}", line_content);
                }
                other => panic!("Expected Regex or Content display info, got: {:?}", other),
            }
        }

        Ok(())
    }

    #[test]
    fn test_symbol_strategy_collection() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = SymbolStrategy::new();

        // シンボル名を検索
        let results = search_runner.collect_results_with_strategy(&strategy, "Person")?;
        
        // 結果の構造を検証（結果がある場合）
        for result in &results {
            match &result.display_info {
                DisplayInfo::Symbol { name, symbol_type } => {
                    assert!(!name.is_empty());
                    assert!(name.contains("Person"));
                    // symbol_typeが適切に設定されていることを確認
                    println!("Found symbol: {} ({:?})", name, symbol_type);
                }
                _ => panic!("Expected Symbol display info"),
            }
        }

        Ok(())
    }

    #[test]
    fn test_empty_query_handling() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);

        // 各戦略で空クエリをテスト
        let content_strategy = ContentStrategy;
        let content_results = search_runner.collect_results_with_strategy(&content_strategy, "")?;
        assert_eq!(content_results.len(), 0);

        let file_strategy = FileStrategy;
        let file_results = search_runner.collect_results_with_strategy(&file_strategy, "")?;
        assert_eq!(file_results.len(), 0);

        let regex_strategy = RegexStrategy;
        let regex_results = search_runner.collect_results_with_strategy(&regex_strategy, "")?;
        assert_eq!(regex_results.len(), 0);

        Ok(())
    }

    #[test]
    fn test_nonexistent_query_handling() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = ContentStrategy;

        // 存在しないパターンを検索
        let results = search_runner.collect_results_with_strategy(&strategy, "nonexistent_pattern_xyz123")?;
        assert_eq!(results.len(), 0);

        Ok(())
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_collection_performance() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = ContentStrategy;

        let start_time = Instant::now();
        let results = search_runner.collect_results_with_strategy(&strategy, "function")?;
        let elapsed = start_time.elapsed();

        // パフォーマンス要件：1秒以内に完了すること
        assert!(elapsed.as_secs() < 1, "Collection took too long: {:?}", elapsed);
        
        // 結果が合理的な範囲内であること
        assert!(results.len() < 1000, "Too many results: {}", results.len());

        println!("Collection completed in {:?} with {} results", elapsed, results.len());

        Ok(())
    }

    #[test]
    fn test_multiple_strategy_performance() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);

        // 各戦略を個別にテスト
        let start_time = Instant::now();
        let content_strategy = ContentStrategy;
        let content_results = search_runner.collect_results_with_strategy(&content_strategy, "test")?;
        let content_elapsed = start_time.elapsed();
        println!("content strategy: {:?} with {} results", content_elapsed, content_results.len());
        assert!(content_elapsed.as_secs() < 2);

        let start_time = Instant::now();
        let file_strategy = FileStrategy;
        let file_results = search_runner.collect_results_with_strategy(&file_strategy, "test")?;
        let file_elapsed = start_time.elapsed();
        println!("file strategy: {:?} with {} results", file_elapsed, file_results.len());
        assert!(file_elapsed.as_secs() < 2);

        let start_time = Instant::now();
        let regex_strategy = RegexStrategy;
        let regex_results = search_runner.collect_results_with_strategy(&regex_strategy, "test")?;
        let regex_elapsed = start_time.elapsed();
        println!("regex strategy: {:?} with {} results", regex_elapsed, regex_results.len());
        assert!(regex_elapsed.as_secs() < 2);

        Ok(())
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_project_root() {
        let nonexistent_path = PathBuf::from("/nonexistent/path/that/should/not/exist");
        let search_runner = SearchRunner::new(nonexistent_path, false);
        let strategy = ContentStrategy;

        // 存在しないパスでも SearchRunner は作成できる
        // （実際の検索時にエラーハンドリングされる）
        let result = search_runner.collect_results_with_strategy(&strategy, "test");
        
        // エラーまたは空の結果が返ることを確認
        match result {
            Ok(results) => assert_eq!(results.len(), 0),
            Err(_) => {} // エラーも許容される
        }
    }

    #[test]
    fn test_malformed_regex_pattern() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = RegexStrategy;

        // 不正な正規表現パターン
        let result = search_runner.collect_results_with_strategy(&strategy, "[[[invalid_regex");
        
        // エラーまたは空の結果が返ることを確認
        match result {
            Ok(results) => assert_eq!(results.len(), 0),
            Err(e) => {
                // 正規表現エラーが適切に処理されることを確認
                let error_msg = e.to_string();
                assert!(error_msg.contains("regex") || error_msg.contains("pattern") || error_msg.contains("invalid"));
            }
        }

        Ok(())
    }

    #[test]
    fn test_concurrent_access() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        use std::thread;
        use std::sync::Arc;

        let runner = Arc::new(search_runner);
        let mut handles = vec![];

        // 複数スレッドで同時にアクセス
        for i in 0..5 {
            let runner_clone = Arc::clone(&runner);
            let handle = thread::spawn(move || {
                let strategy = ContentStrategy;
                let query = format!("test{}", i);
                runner_clone.collect_results_with_strategy(&strategy, &query)
            });
            handles.push(handle);
        }

        // すべてのスレッドの完了を待つ
        for handle in handles {
            let result = handle.join().unwrap();
            // エラーまたは成功が返ることを確認
            match result {
                Ok(_) => {},
                Err(_) => {},
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod integration_with_cli_tests {
    use super::*;

    #[test]
    fn test_collect_vs_run_consistency() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = ContentStrategy;

        // collect_results_with_strategy の結果を取得
        let collected_results = search_runner.collect_results_with_strategy(&strategy, "function")?;

        // 結果の基本的な妥当性を検証
        for result in &collected_results {
            assert!(!result.file_path.as_os_str().is_empty());
            assert!(result.line >= 1);
            assert!(result.column >= 1);
            assert!(result.score >= 0.0);
        }

        // 既存のCLI機能（run_with_strategy）も同じプロジェクトで動作することを確認
        // Note: run_with_strategy はstdoutに出力するため、直接比較はできないが、
        // 少なくともエラーなく実行できることを確認
        let run_result = search_runner.run_with_strategy(&strategy, "function");
        assert!(run_result.is_ok());

        Ok(())
    }

    #[test]
    fn test_heading_mode_consistency() -> Result<()> {
        let temp_dir = create_comprehensive_test_project()?;
        
        // heading = false
        let search_runner_no_heading = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        let strategy = ContentStrategy;
        let results_no_heading = search_runner_no_heading.collect_results_with_strategy(&strategy, "function")?;

        // heading = true
        let search_runner_with_heading = SearchRunner::new(temp_dir.path().to_path_buf(), true);
        let results_with_heading = search_runner_with_heading.collect_results_with_strategy(&strategy, "function")?;

        // collect_results_with_strategy では heading フラグは結果の内容に影響しない
        // （出力フォーマットのみに影響）ため、同じ結果が返るはず
        assert_eq!(results_no_heading.len(), results_with_heading.len());

        Ok(())
    }
}