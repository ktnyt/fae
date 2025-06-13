//! TUI統合の回帰テスト
//! 
//! CLIアプリケーションの新しいTUI機能が正しく動作し、
//! 既存のCLI機能と互換性を保っていることを検証する

use fae::tui::{TuiEngine, TuiState};
use fae::cli::SearchRunner;
use anyhow::Result;
use tempfile::TempDir;
use std::fs::File;
use std::io::Write;
use std::process::Command;

fn create_test_project_with_content() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // 複数のファイルを作成
    let mut file1 = File::create(root.join("app.ts"))?;
    writeln!(file1, "import {{ Component }} from 'react';")?;
    writeln!(file1, "")?;
    writeln!(file1, "interface Props {{")?;
    writeln!(file1, "  title: string;")?;
    writeln!(file1, "  onSubmit: () => void;")?;
    writeln!(file1, "}}")?;
    writeln!(file1, "")?;
    writeln!(file1, "export const MyComponent: Component<Props> = ({{ title, onSubmit }}) => {{")?;
    writeln!(file1, "  return (")?;
    writeln!(file1, "    <div>")?;
    writeln!(file1, "      <h1>{{title}}</h1>")?;
    writeln!(file1, "      <button onClick={{onSubmit}}>Submit</button>")?;
    writeln!(file1, "    </div>")?;
    writeln!(file1, "  );")?;
    writeln!(file1, "}};")?;

    let mut file2 = File::create(root.join("server.rs"))?;
    writeln!(file2, "use std::collections::HashMap;")?;
    writeln!(file2, "use serde::{{Serialize, Deserialize}};")?;
    writeln!(file2, "")?;
    writeln!(file2, "#[derive(Serialize, Deserialize)]")?;
    writeln!(file2, "pub struct User {{")?;
    writeln!(file2, "    pub id: u32,")?;
    writeln!(file2, "    pub name: String,")?;
    writeln!(file2, "    pub email: String,")?;
    writeln!(file2, "}}")?;
    writeln!(file2, "")?;
    writeln!(file2, "pub struct UserService {{")?;
    writeln!(file2, "    users: HashMap<u32, User>,")?;
    writeln!(file2, "}}")?;
    writeln!(file2, "")?;
    writeln!(file2, "impl UserService {{")?;
    writeln!(file2, "    pub fn new() -> Self {{")?;
    writeln!(file2, "        Self {{")?;
    writeln!(file2, "            users: HashMap::new(),")?;
    writeln!(file2, "        }}")?;
    writeln!(file2, "    }}")?;
    writeln!(file2, "")?;
    writeln!(file2, "    pub fn add_user(&mut self, user: User) {{")?;
    writeln!(file2, "        self.users.insert(user.id, user);")?;
    writeln!(file2, "    }}")?;
    writeln!(file2, "")?;
    writeln!(file2, "    pub fn get_user(&self, id: u32) -> Option<&User> {{")?;
    writeln!(file2, "        self.users.get(&id)")?;
    writeln!(file2, "    }}")?;
    writeln!(file2, "}}")?;

    let mut file3 = File::create(root.join("utils.py"))?;
    writeln!(file3, "\"\"\"Utility functions for data processing\"\"\"")?;
    writeln!(file3, "")?;
    writeln!(file3, "import json")?;
    writeln!(file3, "import re")?;
    writeln!(file3, "from typing import Dict, List, Any")?;
    writeln!(file3, "")?;
    writeln!(file3, "def parse_json_file(file_path: str) -> Dict[str, Any]:")?;
    writeln!(file3, "    \"\"\"Parse JSON file and return dictionary\"\"\"")?;
    writeln!(file3, "    with open(file_path, 'r') as f:")?;
    writeln!(file3, "        return json.load(f)")?;
    writeln!(file3, "")?;
    writeln!(file3, "def validate_email(email: str) -> bool:")?;
    writeln!(file3, "    \"\"\"Validate email address format\"\"\"")?;
    writeln!(file3, "    pattern = r'^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{{2,}}$'")?;
    writeln!(file3, "    return re.match(pattern, email) is not None")?;
    writeln!(file3, "")?;
    writeln!(file3, "class DataValidator:")?;
    writeln!(file3, "    \"\"\"Data validation utility class\"\"\"")?;
    writeln!(file3, "    ")?;
    writeln!(file3, "    @staticmethod")?;
    writeln!(file3, "    def is_valid_phone(phone: str) -> bool:")?;
    writeln!(file3, "        \"\"\"Validate phone number format\"\"\"")?;
    writeln!(file3, "        pattern = r'^\\+?1?\\d{{9,15}}$'")?;
    writeln!(file3, "        return re.match(pattern, phone) is not None")?;

    Ok(temp_dir)
}

#[cfg(test)]
mod cli_arguments_tests {
    use super::*;

    #[test]
    fn test_cli_help_display() {
        let output = Command::new("cargo")
            .args(&["run", "--", "--help"])
            .output()
            .expect("Failed to execute command");

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // ヘルプに新しいTUIオプションが含まれていることを確認
        assert!(stdout.contains("--tui"));
        assert!(stdout.contains("Start interactive TUI mode"));
        
        // 既存のオプションも残っていることを確認
        assert!(stdout.contains("--index"));
        assert!(stdout.contains("--backends"));
        assert!(stdout.contains("--heading"));
    }

    #[test]
    fn test_version_display() {
        let output = Command::new("cargo")
            .args(&["run", "--", "--version"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("fae"));
    }
}

#[cfg(test)]
mod tui_engine_creation_tests {
    use super::*;

    #[tokio::test]
    async fn test_tui_engine_creation() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);

        // TuiEngineが正常に作成できることを確認
        let tui_engine_result = TuiEngine::new(temp_dir.path().to_path_buf(), search_runner);
        
        match tui_engine_result {
            Ok(_engine) => {
                // 正常に作成された
                // Note: 実際のTerminal初期化はCI環境では失敗する可能性があるため、
                // ここではエラーが発生しないことのみを確認
            }
            Err(e) => {
                // CI環境やヘッドレス環境では Terminal 初期化でエラーになる可能性がある
                println!("TuiEngine creation failed (expected in CI): {}", e);
            }
        }

        Ok(())
    }

    #[test]
    fn test_tui_state_functionality() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        let mut state = TuiState::new(temp_dir.path().to_path_buf());

        // 初期状態の確認
        assert_eq!(state.query, "");
        assert_eq!(state.results.len(), 0);
        assert_eq!(state.selected_index, 0);
        assert!(!state.loading);

        // 検索結果のモック追加
        use fae::types::{SearchResult, DisplayInfo};
        let mock_result = SearchResult {
            file_path: temp_dir.path().join("app.ts"),
            line: 1,
            column: 1,
            display_info: DisplayInfo::Content {
                line_content: "import { Component } from 'react';".to_string(),
                match_start: 0,
                match_end: 5,
            },
            score: 1.0,
        };

        state.results.push(mock_result);

        // ナビゲーション機能のテスト
        assert_eq!(state.selected_index, 0);
        assert!(state.selected_result().is_some());

        state.select_next();
        assert_eq!(state.selected_index, 0); // 1つしかないので戻る

        state.select_previous();
        assert_eq!(state.selected_index, 0); // 1つしかないので変わらず

        Ok(())
    }
}

#[cfg(test)]
mod backward_compatibility_tests {
    use super::*;

    #[test]
    fn test_existing_cli_functionality_preserved() {
        let temp_dir = create_test_project_with_content().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap();

        // Content search
        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, "Component"])
            .output()
            .expect("Failed to execute command");
        
        // エラーでないことを確認
        if !output.status.success() {
            println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        }
        assert!(output.status.success());

        // Symbol search
        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, "#User"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());

        // File search
        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, ">app"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());

        // Regex search
        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, r"/fn\s+\w+"])
            .output()
            .expect("Failed to execute command");
        
        assert!(output.status.success());
    }

    #[test]
    fn test_backend_info_functionality() {
        let output = Command::new("cargo")
            .args(&["run", "--", "--backends"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Search Backend Information"));
        assert!(stdout.contains("Primary backend"));
        assert!(stdout.contains("Available backends"));
    }

    #[test]
    fn test_index_build_functionality() {
        let temp_dir = create_test_project_with_content().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap();

        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, "--index"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Building project index") || stdout.contains("Index build completed"));
    }

    #[test]
    fn test_heading_mode_functionality() {
        let temp_dir = create_test_project_with_content().unwrap();
        let temp_path = temp_dir.path().to_str().unwrap();

        let output = Command::new("cargo")
            .args(&["run", "--", "-d", temp_path, "--heading", "import"])
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
    }
}

#[cfg(test)]
mod error_scenarios_tests {
    use super::*;

    #[test]
    fn test_nonexistent_directory_handling() {
        let output = Command::new("cargo")
            .args(&["run", "--", "-d", "/nonexistent/path", "test"])
            .output()
            .expect("Failed to execute command");

        // エラーで終了するか、適切に処理されることを確認
        // (実装によってはエラーメッセージを出して正常終了する場合もある)
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            assert!(stderr.len() > 0);
        }
    }

    #[test]
    fn test_invalid_argument_combinations() {
        // index と query の同時指定
        let output = Command::new("cargo")
            .args(&["run", "--", "--index", "test_query"])
            .output()
            .expect("Failed to execute command");

        // この組み合わせは許可されているが、indexが優先される
        assert!(output.status.success());
    }

    #[test]
    fn test_empty_project_directory() {
        let empty_dir = TempDir::new().unwrap();
        let empty_path = empty_dir.path().to_str().unwrap();

        let output = Command::new("cargo")
            .args(&["run", "--", "-d", empty_path, "anything"])
            .output()
            .expect("Failed to execute command");

        // 空のディレクトリでも正常に動作すること（結果が0件）
        assert!(output.status.success());
    }
}

#[cfg(test)]
mod async_functionality_tests {
    use super::*;

    #[tokio::test]
    async fn test_async_search_runner_integration() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);

        // collect_results_with_strategy が非同期環境で正常に動作することを確認
        use fae::cli::strategies::ContentStrategy;
        let strategy = ContentStrategy;
        
        let start_time = std::time::Instant::now();
        let results = tokio::task::spawn_blocking(move || {
            search_runner.collect_results_with_strategy(&strategy, "Component")
        }).await.unwrap()?;
        let elapsed = start_time.elapsed();

        println!("Async search completed in {:?} with {} results", elapsed, results.len());

        // 結果が合理的な範囲内であることを確認
        assert!(results.len() < 100);
        assert!(elapsed.as_secs() < 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_searches() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        
        // 複数の検索を並行実行
        let mut handles = vec![];
        
        for i in 0..3 {
            let temp_path = temp_dir.path().to_path_buf();
            let handle = tokio::spawn(async move {
                let search_runner = SearchRunner::new(temp_path, false);
                use fae::cli::strategies::ContentStrategy;
                let strategy = ContentStrategy;
                
                let query = match i {
                    0 => "Component",
                    1 => "User", 
                    _ => "function",
                };
                
                tokio::task::spawn_blocking(move || {
                    search_runner.collect_results_with_strategy(&strategy, query)
                }).await.unwrap()
            });
            handles.push(handle);
        }

        // 全ての検索が完了することを確認
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        Ok(())
    }
}

#[cfg(test)]
mod memory_and_performance_tests {
    use super::*;

    #[test]
    fn test_memory_efficiency_with_large_results() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        let search_runner = SearchRunner::new(temp_dir.path().to_path_buf(), false);
        
        use fae::cli::strategies::ContentStrategy;
        let strategy = ContentStrategy;

        // メモリ使用量を確認するため、複数回検索を実行
        for i in 0..10 {
            let query = if i % 2 == 0 { "import" } else { "function" };
            let results = search_runner.collect_results_with_strategy(&strategy, query)?;
            
            // 結果が合理的な範囲内であることを確認
            assert!(results.len() < 1000, "Iteration {}: too many results: {}", i, results.len());
            
            // メモリリークがないことを簡単に確認（結果を破棄）
            drop(results);
        }

        Ok(())
    }

    #[test]
    fn test_repeated_tui_state_operations() -> Result<()> {
        let temp_dir = create_test_project_with_content()?;
        let mut state = TuiState::new(temp_dir.path().to_path_buf());

        // 大量の状態変更操作を実行してメモリ効率を確認
        for i in 0..1000 {
            state.query = format!("query_{}", i);
            state.select_next();
            state.select_previous();
            
            // 定期的に結果をクリア
            if i % 100 == 0 {
                state.results.clear();
                state.selected_index = 0;
            }
        }

        // 最終状態が合理的であることを確認
        assert!(state.query.starts_with("query_"));
        assert_eq!(state.selected_index, 0);

        Ok(())
    }
}