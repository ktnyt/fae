use anyhow::Result;
use fae::extract_symbols_from_file;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn debug_typescript_symbols() -> Result<()> {
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

    let mut temp_file = NamedTempFile::with_suffix(".ts")?;
    writeln!(temp_file, "{}", ts_content)?;
    
    let symbols = extract_symbols_from_file(temp_file.path())?;
    
    println!("抽出されたTypeScriptシンボル:");
    for (i, symbol) in symbols.iter().enumerate() {
        println!("  {}: {} ({:?}) at line {}", i, symbol.name, symbol.symbol_type, symbol.line);
    }
    
    Ok(())
}

#[test]
fn debug_javascript_symbols() -> Result<()> {
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
    
    let symbols = extract_symbols_from_file(temp_file.path())?;
    
    println!("抽出されたJavaScriptシンボル:");
    for (i, symbol) in symbols.iter().enumerate() {
        println!("  {}: {} ({:?}) at line {}", i, symbol.name, symbol.symbol_type, symbol.line);
    }
    
    Ok(())
}

#[test]
fn debug_python_symbols() -> Result<()> {
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
    
    let symbols = extract_symbols_from_file(temp_file.path())?;
    
    println!("抽出されたPythonシンボル:");
    for (i, symbol) in symbols.iter().enumerate() {
        println!("  {}: {} ({:?}) at line {}", i, symbol.name, symbol.symbol_type, symbol.line);
    }
    
    Ok(())
}

#[test]
fn debug_rust_symbols() -> Result<()> {
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
    
    let symbols = extract_symbols_from_file(temp_file.path())?;
    
    println!("抽出されたRustシンボル:");
    for (i, symbol) in symbols.iter().enumerate() {
        println!("  {}: {} ({:?}) at line {}", i, symbol.name, symbol.symbol_type, symbol.line);
    }
    
    Ok(())
}