use anyhow::Result;
use fae::SymbolType;
use tempfile::NamedTempFile;
use std::io::Write;

// NOTE: これらのテストはまだ実装されていない機能に対するREDテストです
// Tree-sitter統合実装後にパスするようになります

#[test]
fn test_extract_typescript_symbols() -> Result<()> {
    // TypeScriptファイルのサンプル
    let ts_content = r#"
interface User {
    id: number;
    name: string;
}

class UserService {
    private users: User[] = [];
    
    public addUser(user: User): void {
        this.users.push(user);
    }
    
    public getUser(id: number): User | undefined {
        return this.users.find(u => u.id === id);
    }
}

const API_URL = 'https://api.example.com';

function handleResponse(response: Response): Promise<any> {
    return response.json();
}
"#;

    // 一時ファイル作成
    let mut temp_file = NamedTempFile::with_suffix(".ts")?;
    writeln!(temp_file, "{}", ts_content)?;
    
    // Tree-sitter統合でシンボル抽出（まだ実装されていない）
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    
    // 期待されるシンボル（統合クエリでソースコード順）
    let expected_symbols = [
        ("User", SymbolType::Interface, 2),
        ("UserService", SymbolType::Class, 7),
        ("addUser", SymbolType::Function, 10),
        ("getUser", SymbolType::Function, 14),
        ("API_URL", SymbolType::Constant, 19),
        ("handleResponse", SymbolType::Function, 21),
    ];
    
    // 抽出されたシンボルの検証
    assert_eq!(symbols.len(), expected_symbols.len());
    
    for (i, (name, symbol_type, line)) in expected_symbols.iter().enumerate() {
        assert_eq!(symbols[i].name, *name);
        assert_eq!(symbols[i].symbol_type, *symbol_type);
        assert_eq!(symbols[i].line, *line);
        assert_eq!(symbols[i].file_path, temp_file.path());
    }
    
    Ok(())
}

#[test]
fn test_extract_javascript_symbols() -> Result<()> {
    let js_content = r#"
class Calculator {
    constructor() {
        this.result = 0;
    }
    
    add(value) {
        this.result += value;
        return this;
    }
    
    multiply(value) {
        this.result *= value;
        return this;
    }
}

const PI = 3.14159;

function calculateArea(radius) {
    return PI * radius * radius;
}

const arrowFunction = (x, y) => x + y;
"#;

    let mut temp_file = NamedTempFile::with_suffix(".js")?;
    writeln!(temp_file, "{}", js_content)?;
    
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    
    let expected_symbols = [
        ("Calculator", SymbolType::Class, 2),
        ("constructor", SymbolType::Function, 3),
        ("add", SymbolType::Function, 7),
        ("multiply", SymbolType::Function, 12),
        ("PI", SymbolType::Constant, 18),
        ("calculateArea", SymbolType::Function, 20),
        ("arrowFunction", SymbolType::Constant, 24),
    ];
    
    assert_eq!(symbols.len(), expected_symbols.len());
    
    for (i, (name, symbol_type, line)) in expected_symbols.iter().enumerate() {
        assert_eq!(symbols[i].name, *name);
        assert_eq!(symbols[i].symbol_type, *symbol_type);
        assert_eq!(symbols[i].line, *line);
    }
    
    Ok(())
}

#[test]
fn test_extract_python_symbols() -> Result<()> {
    let py_content = r#"
class DataProcessor:
    def __init__(self, data):
        self.data = data
        self.processed = False
    
    def process(self):
        self.processed = True
        return self._transform_data()
    
    def _transform_data(self):
        return [item.upper() for item in self.data]

MAX_ITEMS = 1000

def validate_input(data):
    return len(data) <= MAX_ITEMS

def main():
    processor = DataProcessor(['hello', 'world'])
    result = processor.process()
    print(result)
"#;

    let mut temp_file = NamedTempFile::with_suffix(".py")?;
    writeln!(temp_file, "{}", py_content)?;
    
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    
    let expected_symbols = [
        ("DataProcessor", SymbolType::Class, 2),
        ("__init__", SymbolType::Function, 3),
        ("process", SymbolType::Function, 7),
        ("_transform_data", SymbolType::Function, 11),
        ("MAX_ITEMS", SymbolType::Constant, 14),
        ("validate_input", SymbolType::Function, 16),
        ("main", SymbolType::Function, 19),
        ("processor", SymbolType::Constant, 20),
        ("result", SymbolType::Constant, 21),
    ];
    
    assert_eq!(symbols.len(), expected_symbols.len());
    
    for (i, (name, symbol_type, line)) in expected_symbols.iter().enumerate() {
        assert_eq!(symbols[i].name, *name);
        assert_eq!(symbols[i].symbol_type, *symbol_type);
        assert_eq!(symbols[i].line, *line);
    }
    
    Ok(())
}

#[test]
fn test_extract_rust_symbols() -> Result<()> {
    let rust_content = r#"
use std::collections::HashMap;

pub struct Config {
    pub name: String,
    pub version: String,
}

impl Config {
    pub fn new(name: String, version: String) -> Self {
        Self { name, version }
    }
    
    pub fn display(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }
}

pub enum Status {
    Active,
    Inactive,
    Pending,
}

const DEFAULT_TIMEOUT: u64 = 30;

pub fn create_default_config() -> Config {
    Config::new("fae".to_string(), "0.1.0".to_string())
}
"#;

    let mut temp_file = NamedTempFile::with_suffix(".rs")?;
    writeln!(temp_file, "{}", rust_content)?;
    
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    
    let expected_symbols = [
        ("Config", SymbolType::Class, 4),  // struct
        ("new", SymbolType::Function, 10),
        ("display", SymbolType::Function, 14),
        ("Status", SymbolType::Type, 19),  // enum
        ("DEFAULT_TIMEOUT", SymbolType::Constant, 25),
        ("create_default_config", SymbolType::Function, 27),
    ];
    
    assert_eq!(symbols.len(), expected_symbols.len());
    
    for (i, (name, symbol_type, line)) in expected_symbols.iter().enumerate() {
        assert_eq!(symbols[i].name, *name);
        assert_eq!(symbols[i].symbol_type, *symbol_type);
        assert_eq!(symbols[i].line, *line);
    }
    
    Ok(())
}

#[test]
fn test_unsupported_file_extension() -> Result<()> {
    // 未対応のファイル拡張子
    let mut temp_file = NamedTempFile::with_suffix(".unknown")?;
    writeln!(temp_file, "some content")?;
    
    // 未対応の場合は空のVecが返されることを期待
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    assert_eq!(symbols.len(), 0);
    
    Ok(())
}

#[test]
fn test_invalid_syntax_file() -> Result<()> {
    // 構文エラーのあるファイル
    let invalid_ts_content = r#"
class BrokenClass {
    // 意図的に構文エラー
    public method( {
        return "broken";
    }
"#;

    let mut temp_file = NamedTempFile::with_suffix(".ts")?;
    writeln!(temp_file, "{}", invalid_ts_content)?;
    
    // 構文エラーの場合もエラーにならず、部分的にでもシンボルを抽出することを期待
    let result = fae::extract_symbols_from_file(temp_file.path());
    
    // エラーにならず、何らかの結果が返されることを確認
    assert!(result.is_ok());
    // 壊れたファイルでも最低限のシンボルは抽出できることを期待
    let _symbols = result.unwrap();
    // パニックしないことが重要（len()は常に >= 0 なので条件は削除）
    
    Ok(())
}

#[test] 
fn test_empty_file() -> Result<()> {
    // 空ファイル
    let temp_file = NamedTempFile::with_suffix(".ts")?;
    // 何も書き込まない
    
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    assert_eq!(symbols.len(), 0);
    
    Ok(())
}

#[test]
fn test_file_with_only_comments() -> Result<()> {
    // コメントのみのファイル
    let comment_only_content = r#"
// This is a comment
/* 
 * Multi-line comment
 * No actual code here
 */
// Another comment
"#;

    let mut temp_file = NamedTempFile::with_suffix(".js")?;
    writeln!(temp_file, "{}", comment_only_content)?;
    
    let symbols = fae::extract_symbols_from_file(temp_file.path())?;
    assert_eq!(symbols.len(), 0);
    
    Ok(())
}