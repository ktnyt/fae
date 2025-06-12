use fae::searchers::ContentSearcher;
use anyhow::Result;
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;

/// テスト用プロジェクトの作成
fn create_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // src ディレクトリ作成
    let src_dir = root.join("src");
    fs::create_dir(&src_dir)?;

    // components ディレクトリ作成
    let components_dir = src_dir.join("components");
    fs::create_dir(&components_dir)?;

    // main.ts - メインファイル
    let mut main_file = File::create(src_dir.join("main.ts"))?;
    writeln!(main_file, "import {{ Component }} from './components/Button';")?;
    writeln!(main_file, "")?;
    writeln!(main_file, "function handleClick() {{")?;
    writeln!(main_file, "    console.log('Button clicked!');")?;
    writeln!(main_file, "    processUserAction('click');")?;
    writeln!(main_file, "}}")?;
    writeln!(main_file, "")?;
    writeln!(main_file, "export default class App {{")?;
    writeln!(main_file, "    constructor() {{")?;
    writeln!(main_file, "        this.handleError = this.handleError.bind(this);")?;
    writeln!(main_file, "    }}")?;
    writeln!(main_file, "")?;
    writeln!(main_file, "    handleError(error: Error) {{")?;
    writeln!(main_file, "        console.error('An error occurred:', error.message);")?;
    writeln!(main_file, "    }}")?;
    writeln!(main_file, "}}")?;

    // Button.tsx - コンポーネントファイル
    let mut button_file = File::create(components_dir.join("Button.tsx"))?;
    writeln!(button_file, "import React from 'react';")?;
    writeln!(button_file, "")?;
    writeln!(button_file, "interface ButtonProps {{")?;
    writeln!(button_file, "    onClick: () => void;")?;
    writeln!(button_file, "    label: string;")?;
    writeln!(button_file, "}}")?;
    writeln!(button_file, "")?;
    writeln!(button_file, "export function Button({{ onClick, label }}: ButtonProps) {{")?;
    writeln!(button_file, "    const handleClick = () => {{")?;
    writeln!(button_file, "        console.log('Button action triggered');")?;
    writeln!(button_file, "        onClick();")?;
    writeln!(button_file, "    }};")?;
    writeln!(button_file, "")?;
    writeln!(button_file, "    return (")?;
    writeln!(button_file, "        <button onClick={{handleClick}}>")?;
    writeln!(button_file, "            {{label}}")?;
    writeln!(button_file, "        </button>")?;
    writeln!(button_file, "    );")?;
    writeln!(button_file, "}}")?;

    // utils.py - Python ユーティリティファイル
    let mut utils_file = File::create(src_dir.join("utils.py"))?;
    writeln!(utils_file, "def process_data(data):")?;
    writeln!(utils_file, "    \"\"\"Process the input data and return results\"\"\"")?;
    writeln!(utils_file, "    if not data:")?;
    writeln!(utils_file, "        raise ValueError('Data cannot be empty')")?;
    writeln!(utils_file, "    ")?;
    writeln!(utils_file, "    # データ処理ロジック")?;
    writeln!(utils_file, "    processed = data.strip().lower()")?;
    writeln!(utils_file, "    return processed")?;
    writeln!(utils_file, "")?;
    writeln!(utils_file, "def handle_error(error):")?;
    writeln!(utils_file, "    \"\"\"Handle errors gracefully\"\"\"")?;
    writeln!(utils_file, "    print(f'Error occurred: {{error}}')")?;
    writeln!(utils_file, "    return None")?;

    // config.rs - Rust 設定ファイル
    let mut config_file = File::create(src_dir.join("config.rs"))?;
    writeln!(config_file, "use std::collections::HashMap;")?;
    writeln!(config_file, "")?;
    writeln!(config_file, "pub struct Config {{")?;
    writeln!(config_file, "    pub host: String,")?;
    writeln!(config_file, "    pub port: u16,")?;
    writeln!(config_file, "}}")?;
    writeln!(config_file, "")?;
    writeln!(config_file, "impl Config {{")?;
    writeln!(config_file, "    pub fn new() -> Self {{")?;
    writeln!(config_file, "        Self {{")?;
    writeln!(config_file, "            host: String::from(\"localhost\"),")?;
    writeln!(config_file, "            port: 8080,")?;
    writeln!(config_file, "        }}")?;
    writeln!(config_file, "    }}")?;
    writeln!(config_file, "")?;
    writeln!(config_file, "    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {{")?;
    writeln!(config_file, "        // 環境変数から設定を読み込み")?;
    writeln!(config_file, "        todo!(\"Implement configuration loading\")")?;
    writeln!(config_file, "    }}")?;
    writeln!(config_file, "}}")?;

    Ok(temp_dir)
}

/// ContentSearcher のテスト
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_search_creation() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(searcher.project_root(), temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_simple_content_search() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // "handleClick" を検索
        let results = searcher.search("handleClick", 10)?;
        
        // main.ts と Button.tsx の両方でヒットするはず
        assert!(results.len() >= 2);
        
        // ファイル名を確認
        let file_names: Vec<String> = results.iter()
            .map(|r| r.file_path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        
        assert!(file_names.contains(&"main.ts".to_string()));
        assert!(file_names.contains(&"Button.tsx".to_string()));
        
        Ok(())
    }

    #[test]
    fn test_case_insensitive_search() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // 大文字小文字を変えて検索
        let results_lower = searcher.search("handleclick", 10)?;
        let results_upper = searcher.search("HANDLECLICK", 10)?;
        let results_mixed = searcher.search("HandleClick", 10)?;
        
        // すべて同じ結果数になるはず（大文字小文字無視）
        assert_eq!(results_lower.len(), results_upper.len());
        assert_eq!(results_lower.len(), results_mixed.len());
        assert!(!results_lower.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_content_search_with_context() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // "console.log" を検索
        let results = searcher.search("console.log", 10)?;
        
        assert!(!results.is_empty());
        
        // 結果の詳細を確認
        for result in &results {
            assert!(result.line > 0); // 行番号が正しく設定されている
            
            // DisplayInfoがContentになっている
            if let fae::types::DisplayInfo::Content { line_content, match_start, match_end } = &result.display_info {
                assert!(line_content.contains("console.log"));
                assert!(*match_start < *match_end);
                assert!(*match_end <= line_content.len());
            } else {
                panic!("Expected Content DisplayInfo, got {:?}", result.display_info);
            }
        }
        
        Ok(())
    }

    #[test]
    fn test_multi_language_search() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // "error" を検索（複数言語でヒット）
        let results = searcher.search("error", 20)?;
        
        assert!(!results.is_empty());
        
        // TypeScript, Python, Rust ファイルでヒットするはず
        let extensions: Vec<String> = results.iter()
            .filter_map(|r| r.file_path.extension())
            .map(|ext| ext.to_string_lossy().to_string())
            .collect();
        
        // 少なくとも2つの異なる拡張子があるはず
        let unique_extensions: std::collections::HashSet<_> = extensions.into_iter().collect();
        assert!(unique_extensions.len() >= 2);
        
        Ok(())
    }

    #[test]
    fn test_search_scoring() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // より具体的な語句で検索
        let results = searcher.search("Button action triggered", 10)?;
        
        if !results.is_empty() {
            // スコアが設定されていることを確認
            for result in &results {
                assert!(result.score > 0.0);
            }
            
            // スコア順でソートされていることを確認
            for i in 1..results.len() {
                assert!(results[i-1].score >= results[i].score);
            }
        }
        
        Ok(())
    }

    #[test]
    fn test_empty_query() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // 空のクエリは結果なし
        let results = searcher.search("", 10)?;
        assert!(results.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_no_matches() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // 存在しない語句を検索
        let results = searcher.search("this_definitely_does_not_exist", 10)?;
        assert!(results.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_limit_functionality() -> Result<()> {
        let temp_dir = create_test_project()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        // 限定数を指定して検索
        let results_limited = searcher.search("the", 2)?;
        let results_unlimited = searcher.search("the", 100)?;
        
        // 制限が効いていることを確認
        assert!(results_limited.len() <= 2);
        assert!(results_unlimited.len() >= results_limited.len());
        
        Ok(())
    }
}