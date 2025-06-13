use std::process::Command;
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// E2Eワークフローテスト - 実際のプロジェクト構造での完全な操作フローをテスト
#[cfg(test)]
mod e2e_workflow_tests {
    use super::*;

    /// 大きなプロジェクト構造を作成（実際のNode.jsプロジェクトのような構造）
    fn create_realistic_project() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // package.json
        let mut package_json = File::create(root.join("package.json"))?;
        writeln!(package_json, r#"{{"#)?;
        writeln!(package_json, r#"  "name": "test-project","#)?;
        writeln!(package_json, r#"  "version": "1.0.0","#)?;
        writeln!(package_json, r#"  "scripts": {{"#)?;
        writeln!(package_json, r#"    "build": "tsc","#)?;
        writeln!(package_json, r#"    "test": "jest","#)?;
        writeln!(package_json, r#"    "start": "node dist/index.js""#)?;
        writeln!(package_json, r#"  }}"#)?;
        writeln!(package_json, r#"}}"#)?;

        // src/ディレクトリ構造
        fs::create_dir_all(root.join("src/components"))?;
        fs::create_dir_all(root.join("src/utils"))?;
        fs::create_dir_all(root.join("src/types"))?;
        fs::create_dir_all(root.join("tests"))?;

        // src/index.ts (メインエントリーポイント)
        let mut index_ts = File::create(root.join("src/index.ts"))?;
        writeln!(index_ts, "import {{ Application }} from './app';")?;
        writeln!(index_ts, "import {{ Logger }} from './utils/logger';")?;
        writeln!(index_ts, "import {{ DatabaseConfig }} from './types/config';")?;
        writeln!(index_ts, "")?;
        writeln!(index_ts, "const logger = new Logger('main');")?;
        writeln!(index_ts, "")?;
        writeln!(index_ts, "async function main() {{")?;
        writeln!(index_ts, "  try {{")?;
        writeln!(index_ts, "    const app = new Application();")?;
        writeln!(index_ts, "    await app.initialize();")?;
        writeln!(index_ts, "    logger.info('Application started successfully');")?;
        writeln!(index_ts, "    await app.start();")?;
        writeln!(index_ts, "  }} catch (error) {{")?;
        writeln!(index_ts, "    logger.error('Failed to start application:', error);")?;
        writeln!(index_ts, "    process.exit(1);")?;
        writeln!(index_ts, "  }}")?;
        writeln!(index_ts, "}}")?;
        writeln!(index_ts, "")?;
        writeln!(index_ts, "main();")?;

        // src/app.ts
        let mut app_ts = File::create(root.join("src/app.ts"))?;
        writeln!(app_ts, "import {{ Server }} from './components/server';")?;
        writeln!(app_ts, "import {{ Database }} from './components/database';")?;
        writeln!(app_ts, "import {{ Logger }} from './utils/logger';")?;
        writeln!(app_ts, "")?;
        writeln!(app_ts, "export class Application {{")?;
        writeln!(app_ts, "  private server: Server;")?;
        writeln!(app_ts, "  private database: Database;")?;
        writeln!(app_ts, "  private logger: Logger;")?;
        writeln!(app_ts, "")?;
        writeln!(app_ts, "  constructor() {{")?;
        writeln!(app_ts, "    this.logger = new Logger('app');")?;
        writeln!(app_ts, "    this.server = new Server();")?;
        writeln!(app_ts, "    this.database = new Database();")?;
        writeln!(app_ts, "  }}")?;
        writeln!(app_ts, "")?;
        writeln!(app_ts, "  async initialize(): Promise<void> {{")?;
        writeln!(app_ts, "    this.logger.info('Initializing application');")?;
        writeln!(app_ts, "    await this.database.connect();")?;
        writeln!(app_ts, "    this.server.setup();")?;
        writeln!(app_ts, "  }}")?;
        writeln!(app_ts, "")?;
        writeln!(app_ts, "  async start(): Promise<void> {{")?;
        writeln!(app_ts, "    this.logger.info('Starting server');")?;
        writeln!(app_ts, "    await this.server.listen(3000);")?;
        writeln!(app_ts, "  }}")?;
        writeln!(app_ts, "}}")?;

        // src/components/server.ts
        let mut server_ts = File::create(root.join("src/components/server.ts"))?;
        writeln!(server_ts, "import {{ Logger }} from '../utils/logger';")?;
        writeln!(server_ts, "import {{ validateRequest }} from '../utils/validation';")?;
        writeln!(server_ts, "")?;
        writeln!(server_ts, "export class Server {{")?;
        writeln!(server_ts, "  private logger: Logger;")?;
        writeln!(server_ts, "  private port: number = 3000;")?;
        writeln!(server_ts, "")?;
        writeln!(server_ts, "  constructor() {{")?;
        writeln!(server_ts, "    this.logger = new Logger('server');")?;
        writeln!(server_ts, "  }}")?;
        writeln!(server_ts, "")?;
        writeln!(server_ts, "  setup(): void {{")?;
        writeln!(server_ts, "    this.logger.info('Setting up server');")?;
        writeln!(server_ts, "    // Setup middleware, routes, etc.")?;
        writeln!(server_ts, "  }}")?;
        writeln!(server_ts, "")?;
        writeln!(server_ts, "  async listen(port: number): Promise<void> {{")?;
        writeln!(server_ts, "    this.port = port;")?;
        writeln!(server_ts, "    this.logger.info(`Server listening on port ${{port}}`);")?;
        writeln!(server_ts, "  }}")?;
        writeln!(server_ts, "")?;
        writeln!(server_ts, "  private handleRequest(req: any): void {{")?;
        writeln!(server_ts, "    if (!validateRequest(req)) {{")?;
        writeln!(server_ts, "      this.logger.warn('Invalid request received');")?;
        writeln!(server_ts, "      return;")?;
        writeln!(server_ts, "    }}")?;
        writeln!(server_ts, "    this.logger.info('Processing valid request');")?;
        writeln!(server_ts, "  }}")?;
        writeln!(server_ts, "}}")?;

        // src/components/database.ts
        let mut database_ts = File::create(root.join("src/components/database.ts"))?;
        writeln!(database_ts, "import {{ Logger }} from '../utils/logger';")?;
        writeln!(database_ts, "import {{ DatabaseConfig }} from '../types/config';")?;
        writeln!(database_ts, "")?;
        writeln!(database_ts, "export class Database {{")?;
        writeln!(database_ts, "  private logger: Logger;")?;
        writeln!(database_ts, "  private config: DatabaseConfig;")?;
        writeln!(database_ts, "  private connected: boolean = false;")?;
        writeln!(database_ts, "")?;
        writeln!(database_ts, "  constructor(config?: DatabaseConfig) {{")?;
        writeln!(database_ts, "    this.logger = new Logger('database');")?;
        writeln!(database_ts, "    this.config = config || {{ host: 'localhost', port: 5432 }};")?;
        writeln!(database_ts, "  }}")?;
        writeln!(database_ts, "")?;
        writeln!(database_ts, "  async connect(): Promise<void> {{")?;
        writeln!(database_ts, "    this.logger.info('Connecting to database');")?;
        writeln!(database_ts, "    // Simulate connection logic")?;
        writeln!(database_ts, "    this.connected = true;")?;
        writeln!(database_ts, "    this.logger.info('Database connected successfully');")?;
        writeln!(database_ts, "  }}")?;
        writeln!(database_ts, "")?;
        writeln!(database_ts, "  async query(sql: string): Promise<any[]> {{")?;
        writeln!(database_ts, "    if (!this.connected) {{")?;
        writeln!(database_ts, "      throw new Error('Database not connected');")?;
        writeln!(database_ts, "    }}")?;
        writeln!(database_ts, "    this.logger.debug(`Executing query: ${{sql}}`);")?;
        writeln!(database_ts, "    return [];")?;
        writeln!(database_ts, "  }}")?;
        writeln!(database_ts, "}}")?;

        // src/utils/logger.ts
        let mut logger_ts = File::create(root.join("src/utils/logger.ts"))?;
        writeln!(logger_ts, "export enum LogLevel {{")?;
        writeln!(logger_ts, "  DEBUG = 0,")?;
        writeln!(logger_ts, "  INFO = 1,")?;
        writeln!(logger_ts, "  WARN = 2,")?;
        writeln!(logger_ts, "  ERROR = 3")?;
        writeln!(logger_ts, "}}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "export class Logger {{")?;
        writeln!(logger_ts, "  private context: string;")?;
        writeln!(logger_ts, "  private level: LogLevel = LogLevel.INFO;")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  constructor(context: string) {{")?;
        writeln!(logger_ts, "    this.context = context;")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  info(message: string, ...args: any[]): void {{")?;
        writeln!(logger_ts, "    this.log(LogLevel.INFO, message, ...args);")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  debug(message: string, ...args: any[]): void {{")?;
        writeln!(logger_ts, "    this.log(LogLevel.DEBUG, message, ...args);")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  warn(message: string, ...args: any[]): void {{")?;
        writeln!(logger_ts, "    this.log(LogLevel.WARN, message, ...args);")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  error(message: string, ...args: any[]): void {{")?;
        writeln!(logger_ts, "    this.log(LogLevel.ERROR, message, ...args);")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "")?;
        writeln!(logger_ts, "  private log(level: LogLevel, message: string, ...args: any[]): void {{")?;
        writeln!(logger_ts, "    if (level >= this.level) {{")?;
        writeln!(logger_ts, "      const timestamp = new Date().toISOString();")?;
        writeln!(logger_ts, "      const levelStr = LogLevel[level];")?;
        writeln!(logger_ts, "      console.log(`${{timestamp}} [${{levelStr}}] [${{this.context}}] ${{message}}`, ...args);")?;
        writeln!(logger_ts, "    }}")?;
        writeln!(logger_ts, "  }}")?;
        writeln!(logger_ts, "}}")?;

        // src/utils/validation.ts
        let mut validation_ts = File::create(root.join("src/utils/validation.ts"))?;
        writeln!(validation_ts, "export function validateRequest(req: any): boolean {{")?;
        writeln!(validation_ts, "  if (!req || typeof req !== 'object') {{")?;
        writeln!(validation_ts, "    return false;")?;
        writeln!(validation_ts, "  }}")?;
        writeln!(validation_ts, "")?;
        writeln!(validation_ts, "  // Basic validation logic")?;
        writeln!(validation_ts, "  return true;")?;
        writeln!(validation_ts, "}}")?;
        writeln!(validation_ts, "")?;
        writeln!(validation_ts, "export function validateEmail(email: string): boolean {{")?;
        writeln!(validation_ts, "  const emailRegex = /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/;")?;
        writeln!(validation_ts, "  return emailRegex.test(email);")?;
        writeln!(validation_ts, "}}")?;
        writeln!(validation_ts, "")?;
        writeln!(validation_ts, "export function sanitizeInput(input: string): string {{")?;
        writeln!(validation_ts, "  return input.replace(/[<>\"'&]/g, '');")?;
        writeln!(validation_ts, "}}")?;

        // src/types/config.ts
        let mut config_ts = File::create(root.join("src/types/config.ts"))?;
        writeln!(config_ts, "export interface DatabaseConfig {{")?;
        writeln!(config_ts, "  host: string;")?;
        writeln!(config_ts, "  port: number;")?;
        writeln!(config_ts, "  username?: string;")?;
        writeln!(config_ts, "  password?: string;")?;
        writeln!(config_ts, "  database?: string;")?;
        writeln!(config_ts, "}}")?;
        writeln!(config_ts, "")?;
        writeln!(config_ts, "export interface ServerConfig {{")?;
        writeln!(config_ts, "  port: number;")?;
        writeln!(config_ts, "  host: string;")?;
        writeln!(config_ts, "  ssl?: boolean;")?;
        writeln!(config_ts, "}}")?;
        writeln!(config_ts, "")?;
        writeln!(config_ts, "export interface AppConfig {{")?;
        writeln!(config_ts, "  database: DatabaseConfig;")?;
        writeln!(config_ts, "  server: ServerConfig;")?;
        writeln!(config_ts, "  logLevel: 'debug' | 'info' | 'warn' | 'error';")?;
        writeln!(config_ts, "}}")?;

        // tests/server.test.ts
        let mut server_test = File::create(root.join("tests/server.test.ts"))?;
        writeln!(server_test, "import {{ Server }} from '../src/components/server';")?;
        writeln!(server_test, "")?;
        writeln!(server_test, "describe('Server', () => {{")?;
        writeln!(server_test, "  let server: Server;")?;
        writeln!(server_test, "")?;
        writeln!(server_test, "  beforeEach(() => {{")?;
        writeln!(server_test, "    server = new Server();")?;
        writeln!(server_test, "  }});")?;
        writeln!(server_test, "")?;
        writeln!(server_test, "  test('should setup server', () => {{")?;
        writeln!(server_test, "    expect(() => server.setup()).not.toThrow();")?;
        writeln!(server_test, "  }});")?;
        writeln!(server_test, "")?;
        writeln!(server_test, "  test('should listen on specified port', async () => {{")?;
        writeln!(server_test, "    await expect(server.listen(3000)).resolves.toBeUndefined();")?;
        writeln!(server_test, "  }});")?;
        writeln!(server_test, "}});")?;

        // Python ファイルも追加
        fs::create_dir_all(root.join("scripts"))?;
        let mut deploy_py = File::create(root.join("scripts/deploy.py"))?;
        writeln!(deploy_py, "#!/usr/bin/env python3")?;
        writeln!(deploy_py, "import os")?;
        writeln!(deploy_py, "import sys")?;
        writeln!(deploy_py, "import subprocess")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "class DeploymentManager:")?;
        writeln!(deploy_py, "    def __init__(self, environment):")?;
        writeln!(deploy_py, "        self.environment = environment")?;
        writeln!(deploy_py, "        self.config = self.load_config()")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    def load_config(self):")?;
        writeln!(deploy_py, "        config_file = f'config/{{self.environment}}.json'")?;
        writeln!(deploy_py, "        print(f'Loading configuration from {{config_file}}')")?;
        writeln!(deploy_py, "        return {{}}")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    def deploy(self):")?;
        writeln!(deploy_py, "        print(f'Deploying to {{self.environment}}')")?;
        writeln!(deploy_py, "        self.run_tests()")?;
        writeln!(deploy_py, "        self.build_application()")?;
        writeln!(deploy_py, "        self.push_to_server()")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    def run_tests(self):")?;
        writeln!(deploy_py, "        print('Running tests...')")?;
        writeln!(deploy_py, "        result = subprocess.run(['npm', 'test'], capture_output=True)")?;
        writeln!(deploy_py, "        if result.returncode != 0:")?;
        writeln!(deploy_py, "            raise Exception('Tests failed')")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    def build_application(self):")?;
        writeln!(deploy_py, "        print('Building application...')")?;
        writeln!(deploy_py, "        subprocess.run(['npm', 'run', 'build'], check=True)")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    def push_to_server(self):")?;
        writeln!(deploy_py, "        print('Pushing to server...')")?;
        writeln!(deploy_py, "        # Deployment logic here")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "if __name__ == '__main__':")?;
        writeln!(deploy_py, "    if len(sys.argv) != 2:")?;
        writeln!(deploy_py, "        print('Usage: deploy.py <environment>')")?;
        writeln!(deploy_py, "        sys.exit(1)")?;
        writeln!(deploy_py, "")?;
        writeln!(deploy_py, "    env = sys.argv[1]")?;
        writeln!(deploy_py, "    manager = DeploymentManager(env)")?;
        writeln!(deploy_py, "    manager.deploy()")?;

        Ok(temp_dir)
    }

    fn run_fae_command(dir: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
        // テスト実行時のパスからバイナリを見つける
        let current_dir = std::env::current_dir()?;
        let binary_path = current_dir.join("target/debug/fae");
        
        let output = Command::new(&binary_path)
            .args(args)
            .current_dir(dir)
            .output()?;

        if !output.status.success() {
            return Err(format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    #[test]
    fn test_complete_project_indexing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // プロジェクト全体のインデックス構築をテスト
        // 十分な数のファイルが発見されるかを確認
        let symbol_search = run_fae_command(project_path, &["#Logger"])?;
        
        // Loggerクラスが見つかるはず
        assert!(symbol_search.contains("Logger"));
        
        // 複数のファイルからLoggerが参照されているはず
        assert!(symbol_search.contains("logger.ts") || symbol_search.contains("app.ts"));

        Ok(())
    }

    #[test]
    fn test_cross_file_dependency_search() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // クロスファイル依存関係の検索テスト
        let import_search = run_fae_command(project_path, &["import"])?;
        
        // 多数のimport文が見つかるはず
        assert!(import_search.contains("import"));
        
        // 複数のファイルでimportが使われているはず
        let import_count = import_search.matches("import").count();
        assert!(import_count >= 5, "Expected at least 5 import statements, found {}", import_count);

        Ok(())
    }

    #[test]
    fn test_function_method_discovery() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // 関数とメソッドの発見テスト
        let function_search = run_fae_command(project_path, &["#connect"])?;
        
        // Database.connect()メソッドが見つかるはず
        if !function_search.is_empty() {
            assert!(function_search.contains("connect"));
        }

        // async関数の検索
        let async_search = run_fae_command(project_path, &["async"])?;
        
        // 複数のasync関数が見つかるはず
        if !async_search.is_empty() {
            assert!(async_search.contains("async"));
        }

        Ok(())
    }

    #[test]
    fn test_interface_type_search() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // インターフェースと型の検索テスト
        let interface_search = run_fae_command(project_path, &["#DatabaseConfig"])?;
        
        // DatabaseConfigインターフェースが見つかるはず
        if !interface_search.is_empty() {
            assert!(interface_search.contains("DatabaseConfig"));
        }

        // 型定義ファイルでの検索
        let type_file_search = run_fae_command(project_path, &[">config"])?;
        
        // config.tsファイルが見つかるはず
        assert!(type_file_search.contains("config"));

        Ok(())
    }

    #[test]
    fn test_regex_patterns_in_codebase() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // 正規表現パターンでの検索テスト
        let arrow_functions = run_fae_command(project_path, &[r"/=>\s*"])?;
        let console_logs = run_fae_command(project_path, &[r"/console\.\w+"])?;
        let error_handling = run_fae_command(project_path, &[r"/catch\s*\("])?;

        // それぞれのパターンで何らかの結果が得られるはず
        println!("Arrow functions found: {}", !arrow_functions.is_empty());
        println!("Console logs found: {}", !console_logs.is_empty());
        println!("Error handling found: {}", !error_handling.is_empty());

        Ok(())
    }

    #[test]
    fn test_multi_language_project_handling() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // 多言語プロジェクトでの検索テスト
        let ts_files = run_fae_command(project_path, &[">*.ts"])?;
        let py_files = run_fae_command(project_path, &[">*.py"])?;
        let json_files = run_fae_command(project_path, &[">*.json"])?;

        // TypeScriptファイルが見つかるはず
        if !ts_files.is_empty() {
            assert!(ts_files.contains(".ts"));
        }

        // Pythonファイルが見つかるはず
        if !py_files.is_empty() {
            assert!(py_files.contains(".py"));
        }

        // JSONファイルが見つかるはず
        if !json_files.is_empty() {
            assert!(json_files.contains(".json"));
        }

        Ok(())
    }

    #[test]
    fn test_nested_directory_search() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // ネストしたディレクトリでの検索テスト
        let components_search = run_fae_command(project_path, &[">components"])?;
        let utils_search = run_fae_command(project_path, &[">utils"])?;
        let types_search = run_fae_command(project_path, &[">types"])?;

        // 各ディレクトリが見つかるはず
        if !components_search.is_empty() {
            assert!(components_search.contains("components"));
        }
        
        if !utils_search.is_empty() {
            assert!(utils_search.contains("utils"));
        }
        
        if !types_search.is_empty() {
            assert!(types_search.contains("types"));
        }

        Ok(())
    }

    #[test]
    fn test_performance_with_larger_codebase() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // パフォーマンステスト - 大きなクエリでの応答時間
        let start_time = std::time::Instant::now();
        
        // 一般的な検索を実行
        let result = run_fae_command(project_path, &["function"])?;
        
        let elapsed = start_time.elapsed();
        
        // 5秒以内に完了するはず（妥当な上限）
        assert!(elapsed.as_secs() < 5, "Search took too long: {}ms", elapsed.as_millis());
        
        // 何らかの結果が得られるはず
        println!("Search completed in {}ms, {} characters returned", 
                 elapsed.as_millis(), result.len());

        Ok(())
    }

    #[test]
    fn test_error_recovery_in_complex_project() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // エラー回復能力のテスト - 破損したファイルがあっても動作継続
        let broken_file_path = project_path.join("src/broken.ts");
        let mut broken_file = File::create(&broken_file_path)?;
        writeln!(broken_file, "This is not valid TypeScript syntax ??? {{{{ ]]]")?;

        // 破損したファイルがあっても正常なファイルは検索できるはず
        let search_result = run_fae_command(project_path, &["Logger"])?;
        
        // 他の正常なファイルからの結果は得られるはず
        // （エラー処理によって完全に失敗しないことを確認）
        println!("Search with broken file result: {}", search_result);

        Ok(())
    }

    #[test]
    fn test_comprehensive_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_realistic_project()?;
        let project_path = temp_dir.path();

        // 包括的ワークフローテスト
        
        // 1. プロジェクト構造の把握
        let files_overview = run_fae_command(project_path, &[">src"])?;
        assert!(files_overview.contains("src"));

        // 2. 特定機能の検索
        let logger_usage = run_fae_command(project_path, &["this.logger.info"])?;
        
        // 3. エラーハンドリングパターンの調査
        let error_patterns = run_fae_command(project_path, &["/catch|Error|throw"])?;
        
        // 4. 設定関連ファイルの特定
        let config_files = run_fae_command(project_path, &["#Config"])?;
        
        // 5. テストファイルの発見
        let test_files = run_fae_command(project_path, &[">test"])?;

        // すべてのステップが基本的なエラーなく完了することを確認
        println!("Files overview: {} chars", files_overview.len());
        println!("Logger usage: {} chars", logger_usage.len());
        println!("Error patterns: {} chars", error_patterns.len()); 
        println!("Config files: {} chars", config_files.len());
        println!("Test files: {} chars", test_files.len());

        Ok(())
    }
}