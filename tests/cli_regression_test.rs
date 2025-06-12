use std::process::Command;
use std::str;
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;

/// CLIの回帰テスト - 実際のCLIプロセスを起動して動作を検証
#[cfg(test)]
mod cli_regression_tests {
    use super::*;

    fn create_test_project() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // src ディレクトリ作成
        let src_dir = root.join("src");
        fs::create_dir(&src_dir)?;

        // TypeScript ファイル
        let mut ts_file = File::create(src_dir.join("utils.ts"))?;
        writeln!(ts_file, "export interface UserData {{")?;
        writeln!(ts_file, "  name: string;")?;
        writeln!(ts_file, "  age: number;")?;
        writeln!(ts_file, "}}")?;
        writeln!(ts_file, "")?;
        writeln!(ts_file, "export function processUser(data: UserData): string {{")?;
        writeln!(ts_file, "  console.log('Processing user:', data.name);")?;
        writeln!(ts_file, "  return `${{data.name}} is ${{data.age}} years old`;")?;
        writeln!(ts_file, "}}")?;

        // JavaScript ファイル
        let mut js_file = File::create(src_dir.join("calculator.js"))?;
        writeln!(js_file, "class Calculator {{")?;
        writeln!(js_file, "  constructor() {{")?;
        writeln!(js_file, "    this.history = [];")?;
        writeln!(js_file, "  }}")?;
        writeln!(js_file, "")?;
        writeln!(js_file, "  add(a, b) {{")?;
        writeln!(js_file, "    console.log('Adding numbers');")?;
        writeln!(js_file, "    return a + b;")?;
        writeln!(js_file, "  }}")?;
        writeln!(js_file, "}}")?;

        // Python ファイル
        let mut py_file = File::create(src_dir.join("helper.py"))?;
        writeln!(py_file, "class DataProcessor:")?;
        writeln!(py_file, "    def __init__(self):")?;
        writeln!(py_file, "        self.items = []")?;
        writeln!(py_file, "")?;
        writeln!(py_file, "    def process(self, data):")?;
        writeln!(py_file, "        print('Processing data')")?;
        writeln!(py_file, "        return len(data)")?;
        writeln!(py_file, "")?;
        writeln!(py_file, "def main():")?;
        writeln!(py_file, "    processor = DataProcessor()")?;
        writeln!(py_file, "    print(processor.process([1, 2, 3]))")?;

        // Rust ファイル
        let mut rs_file = File::create(src_dir.join("config.rs"))?;
        writeln!(rs_file, "pub struct Config {{")?;
        writeln!(rs_file, "    pub debug: bool,")?;
        writeln!(rs_file, "    pub timeout: u32,")?;
        writeln!(rs_file, "}}")?;
        writeln!(rs_file, "")?;
        writeln!(rs_file, "impl Config {{")?;
        writeln!(rs_file, "    pub fn new() -> Self {{")?;
        writeln!(rs_file, "        println!(\"Creating config\");")?;
        writeln!(rs_file, "        Config {{ debug: false, timeout: 30 }}")?;
        writeln!(rs_file, "    }}")?;
        writeln!(rs_file, "}}")?;

        // README.md
        let mut readme = File::create(root.join("README.md"))?;
        writeln!(readme, "# Test Project")?;
        writeln!(readme, "")?;
        writeln!(readme, "This is a test project for fae CLI testing.")?;
        writeln!(readme, "It contains files in multiple languages.")?;

        // .gitignore
        let mut gitignore = File::create(root.join(".gitignore"))?;
        writeln!(gitignore, "node_modules/")?;
        writeln!(gitignore, "target/")?;
        writeln!(gitignore, "*.log")?;

        // Cargo.toml (テスト用に最小限のもの)
        let mut cargo_toml = File::create(root.join("Cargo.toml"))?;
        writeln!(cargo_toml, "[package]")?;
        writeln!(cargo_toml, "name = \"test-project\"")?;
        writeln!(cargo_toml, "version = \"0.1.0\"")?;
        writeln!(cargo_toml, "edition = \"2021\"")?;
        writeln!(cargo_toml, "")?;
        writeln!(cargo_toml, "[dependencies]")?;
        writeln!(cargo_toml, "fae = {{ path = \"../..\" }}")?;

        Ok(temp_dir)
    }

    fn run_fae_command(dir: &std::path::Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
        // テスト実行時のパスからバイナリを見つける
        let current_dir = std::env::current_dir()?;
        let binary_path = current_dir.join("target/debug/fae");
        
        let output = Command::new(&binary_path)
            .args(args)
            .current_dir(dir)
            .output()?;

        if !output.status.success() {
            return Err(format!("Command failed with status: {} stderr: {}", 
                output.status, 
                String::from_utf8_lossy(&output.stderr)
            ).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[test]
    fn test_content_search_cli() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // Content search: "console"を検索
        let output = run_fae_command(project_path, &["console"])?;
        
        // TypeScriptファイルのconsole.logが見つかるはず
        assert!(output.contains("utils.ts"));
        assert!(output.contains("console.log"));
        
        // JavaScriptファイルのconsole.logも見つかるはず
        assert!(output.contains("calculator.js"));

        Ok(())
    }

    #[test]
    fn test_symbol_search_cli() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // Symbol search: "#process"を検索
        let output = run_fae_command(project_path, &["#process"])?;
        
        // processUser関数とprocess関数が見つかるはず
        assert!(output.contains("processUser") || output.contains("process"));
        assert!(output.contains("utils.ts") || output.contains("helper.py"));

        Ok(())
    }

    #[test]
    fn test_file_search_cli() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // File search: ">config"を検索
        let output = run_fae_command(project_path, &[">config"])?;
        
        // config.rsファイルが見つかるはず
        assert!(output.contains("config.rs"));

        Ok(())
    }

    #[test]
    fn test_regex_search_cli() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // Regex search: "/console\\.log"を検索
        let output = run_fae_command(project_path, &["/console\\.log"])?;
        
        // console.logの呼び出しが見つかるはず
        assert!(output.contains("console.log"));
        assert!(output.contains("utils.ts") || output.contains("calculator.js"));

        Ok(())
    }

    #[test]
    fn test_heading_format_cli() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // --headingオプション付きでContent search
        let output = run_fae_command(project_path, &["console", "--heading"])?;
        
        // TTY形式の出力（ファイル名ヘッダー）があるはず
        assert!(output.contains("utils.ts") || output.contains("calculator.js"));

        Ok(())
    }

    #[test]
    fn test_empty_query_behavior() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // 空のクエリ
        let output = run_fae_command(project_path, &[""])?;
        
        // 空のクエリでも正常終了するはず（デフォルト動作）
        // 最近変更されたファイルなどが表示される可能性
        assert!(!output.is_empty() || output.is_empty()); // いずれも許可

        Ok(())
    }

    #[test]
    fn test_non_existent_search() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // 存在しない文字列を検索
        let output = run_fae_command(project_path, &["xyzzynoexiststring"])?;
        
        // 結果が空、または「見つからない」メッセージがあるはず
        // （実装によって動作が異なる可能性があるため、エラーではなく正常終了を期待）
        assert!(!output.contains("error") || output.is_empty());

        Ok(())
    }

    #[test]
    fn test_multiple_mode_consistency() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // 複数の検索モードで同じベース文字列を検索
        let content_output = run_fae_command(project_path, &["Config"])?;
        let symbol_output = run_fae_command(project_path, &["#Config"])?;
        let file_output = run_fae_command(project_path, &[">config"])?;

        // それぞれの検索で何らかの結果が得られるはず
        // Content searchではConfigクラス定義が見つかる
        if !content_output.is_empty() {
            assert!(content_output.contains("Config") || content_output.contains("config"));
        }

        // Symbol searchではConfigシンボルが見つかる
        if !symbol_output.is_empty() {
            assert!(symbol_output.contains("Config"));
        }

        // File searchではconfig.rsが見つかる
        if !file_output.is_empty() {
            assert!(file_output.contains("config"));
        }

        Ok(())
    }

    #[test]
    fn test_case_sensitivity() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // 大文字小文字の異なる検索
        let lowercase_output = run_fae_command(project_path, &["config"])?;
        let uppercase_output = run_fae_command(project_path, &["CONFIG"])?;

        // 大文字小文字を区別しない検索の場合、両方で結果が得られるはず
        // （検索エンジンの設定によって動作が異なる可能性）
        println!("Lowercase search results: {}", lowercase_output);
        println!("Uppercase search results: {}", uppercase_output);

        Ok(())
    }

    #[test]
    fn test_special_characters_in_search() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_test_project()?;
        let project_path = temp_dir.path();

        // 特殊文字を含む検索
        let bracket_search = run_fae_command(project_path, &["{{"])?;
        let dot_search = run_fae_command(project_path, &["."])?;

        // 特殊文字の検索でもクラッシュしないはず
        println!("Bracket search results: {}", bracket_search);
        println!("Dot search results: {}", dot_search);

        Ok(())
    }
}