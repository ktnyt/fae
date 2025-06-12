use std::process::Command;
use tempfile::TempDir;
use std::fs::{self, File};
use std::io::Write;
use std::time::{Instant, Duration};

/// パフォーマンス回帰テスト - インデックス構築と検索の性能測定
#[cfg(test)]
mod performance_tests {
    use super::*;

    /// 大量ファイルを含むプロジェクトを作成
    fn create_large_project() -> Result<TempDir, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // 複数のディレクトリ構造を作成
        let dirs = [
            "src/components", "src/utils", "src/services", "src/types", "src/hooks",
            "lib/core", "lib/helpers", "lib/validators", "lib/formatters",
            "tests/unit", "tests/integration", "tests/e2e",
            "scripts/build", "scripts/deploy", "scripts/utils",
            "docs/api", "docs/guides", "docs/examples"
        ];

        for dir in &dirs {
            fs::create_dir_all(root.join(dir))?;
        }

        // 各ディレクトリに複数のファイルを作成（合計100+ファイル）
        create_typescript_files(&root.join("src/components"), 15)?;
        create_typescript_files(&root.join("src/utils"), 10)?;
        create_typescript_files(&root.join("src/services"), 8)?;
        create_typescript_files(&root.join("src/types"), 12)?;
        create_typescript_files(&root.join("src/hooks"), 6)?;
        
        create_javascript_files(&root.join("lib/core"), 10)?;
        create_javascript_files(&root.join("lib/helpers"), 8)?;
        create_javascript_files(&root.join("lib/validators"), 5)?;
        create_javascript_files(&root.join("lib/formatters"), 4)?;
        
        create_test_files(&root.join("tests/unit"), 20)?;
        create_test_files(&root.join("tests/integration"), 8)?;
        create_test_files(&root.join("tests/e2e"), 5)?;
        
        create_python_files(&root.join("scripts/build"), 3)?;
        create_python_files(&root.join("scripts/deploy"), 4)?;
        create_python_files(&root.join("scripts/utils"), 6)?;

        Ok(temp_dir)
    }

    fn create_typescript_files(dir: &std::path::Path, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..count {
            let mut file = File::create(dir.join(format!("module{}.ts", i)))?;
            writeln!(file, "import {{ Logger }} from '../utils/logger';")?;
            writeln!(file, "import {{ Config }} from '../types/config';")?;
            writeln!(file, "")?;
            writeln!(file, "export interface Module{}Props {{", i)?;
            writeln!(file, "  id: string;")?;
            writeln!(file, "  name: string;")?;
            writeln!(file, "  active: boolean;")?;
            writeln!(file, "  metadata?: Record<string, any>;")?;
            writeln!(file, "}}")?;
            writeln!(file, "")?;
            writeln!(file, "export class Module{} {{", i)?;
            writeln!(file, "  private logger: Logger;")?;
            writeln!(file, "  private config: Config;")?;
            writeln!(file, "  private props: Module{}Props;", i)?;
            writeln!(file, "")?;
            writeln!(file, "  constructor(props: Module{}Props) {{", i)?;
            writeln!(file, "    this.logger = new Logger('module{}');", i)?;
            writeln!(file, "    this.props = props;")?;
            writeln!(file, "    this.config = new Config();")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  async initialize(): Promise<void> {{")?;
            writeln!(file, "    this.logger.info('Initializing module{}');", i)?;
            writeln!(file, "    await this.loadConfiguration();")?;
            writeln!(file, "    await this.validateProps();")?;
            writeln!(file, "    this.logger.info('Module{} initialized successfully');", i)?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  private async loadConfiguration(): Promise<void> {{")?;
            writeln!(file, "    try {{")?;
            writeln!(file, "      const configData = await this.config.load();")?;
            writeln!(file, "      this.logger.debug('Configuration loaded:', configData);")?;
            writeln!(file, "    }} catch (error) {{")?;
            writeln!(file, "      this.logger.error('Failed to load configuration:', error);")?;
            writeln!(file, "      throw error;")?;
            writeln!(file, "    }}")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  private async validateProps(): Promise<void> {{")?;
            writeln!(file, "    if (!this.props.id || !this.props.name) {{")?;
            writeln!(file, "      throw new Error('Invalid props: id and name are required');")?;
            writeln!(file, "    }}")?;
            writeln!(file, "    this.logger.debug('Props validation passed');")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  public async process(data: any[]): Promise<any[]> {{")?;
            writeln!(file, "    this.logger.info(`Processing ${{data.length}} items`);")?;
            writeln!(file, "    const results = [];")?;
            writeln!(file, "    for (const item of data) {{")?;
            writeln!(file, "      try {{")?;
            writeln!(file, "        const processed = await this.processItem(item);")?;
            writeln!(file, "        results.push(processed);")?;
            writeln!(file, "      }} catch (error) {{")?;
            writeln!(file, "        this.logger.error('Failed to process item:', error);")?;
            writeln!(file, "      }}")?;
            writeln!(file, "    }}")?;
            writeln!(file, "    this.logger.info(`Processed ${{results.length}} items successfully`);")?;
            writeln!(file, "    return results;")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  private async processItem(item: any): Promise<any> {{")?;
            writeln!(file, "    // Complex processing logic simulation")?;
            writeln!(file, "    await new Promise(resolve => setTimeout(resolve, 1));")?;
            writeln!(file, "    return {{ ...item, processed: true, timestamp: Date.now() }};")?;
            writeln!(file, "  }}")?;
            writeln!(file, "}}")?;
        }
        Ok(())
    }

    fn create_javascript_files(dir: &std::path::Path, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..count {
            let mut file = File::create(dir.join(format!("helper{}.js", i)))?;
            writeln!(file, "const {{ EventEmitter }} = require('events');")?;
            writeln!(file, "const {{ promisify }} = require('util');")?;
            writeln!(file, "")?;
            writeln!(file, "class Helper{} extends EventEmitter {{", i)?;
            writeln!(file, "  constructor(options = {{}}) {{")?;
            writeln!(file, "    super();")?;
            writeln!(file, "    this.options = options;")?;
            writeln!(file, "    this.cache = new Map();")?;
            writeln!(file, "    this.isInitialized = false;")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  async initialize() {{")?;
            writeln!(file, "    console.log('Initializing helper{}');", i)?;
            writeln!(file, "    await this.loadDefaults();")?;
            writeln!(file, "    this.setupEventHandlers();")?;
            writeln!(file, "    this.isInitialized = true;")?;
            writeln!(file, "    this.emit('initialized');")?;
            writeln!(file, "    console.log('Helper{} initialized');", i)?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  async loadDefaults() {{")?;
            writeln!(file, "    const defaults = {{")?;
            writeln!(file, "      timeout: 5000,")?;
            writeln!(file, "      retries: 3,")?;
            writeln!(file, "      debug: false")?;
            writeln!(file, "    }};")?;
            writeln!(file, "    this.options = {{ ...defaults, ...this.options }};")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  setupEventHandlers() {{")?;
            writeln!(file, "    this.on('error', (error) => {{")?;
            writeln!(file, "      console.error(`Helper{} error:`, error);", i)?;
            writeln!(file, "    }});")?;
            writeln!(file, "    this.on('data', (data) => {{")?;
            writeln!(file, "      if (this.options.debug) {{")?;
            writeln!(file, "        console.log(`Helper{} received data:`, data);", i)?;
            writeln!(file, "      }}")?;
            writeln!(file, "    }});")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  async processData(input) {{")?;
            writeln!(file, "    if (!this.isInitialized) {{")?;
            writeln!(file, "      throw new Error('Helper{} not initialized');", i)?;
            writeln!(file, "    }}")?;
            writeln!(file, "")?;
            writeln!(file, "    const cacheKey = JSON.stringify(input);")?;
            writeln!(file, "    if (this.cache.has(cacheKey)) {{")?;
            writeln!(file, "      return this.cache.get(cacheKey);")?;
            writeln!(file, "    }}")?;
            writeln!(file, "")?;
            writeln!(file, "    try {{")?;
            writeln!(file, "      const result = await this.transform(input);")?;
            writeln!(file, "      this.cache.set(cacheKey, result);")?;
            writeln!(file, "      this.emit('data', result);")?;
            writeln!(file, "      return result;")?;
            writeln!(file, "    }} catch (error) {{")?;
            writeln!(file, "      this.emit('error', error);")?;
            writeln!(file, "      throw error;")?;
            writeln!(file, "    }}")?;
            writeln!(file, "  }}")?;
            writeln!(file, "")?;
            writeln!(file, "  async transform(data) {{")?;
            writeln!(file, "    // Simulate complex transformation")?;
            writeln!(file, "    await new Promise(resolve => setTimeout(resolve, 10));")?;
            writeln!(file, "    return {{")?;
            writeln!(file, "      original: data,")?;
            writeln!(file, "      transformed: true,")?;
            writeln!(file, "      helper: 'Helper{}',", i)?;
            writeln!(file, "      timestamp: new Date().toISOString()")?;
            writeln!(file, "    }};")?;
            writeln!(file, "  }}")?;
            writeln!(file, "}}")?;
            writeln!(file, "")?;
            writeln!(file, "module.exports = Helper{};", i)?;
        }
        Ok(())
    }

    fn create_test_files(dir: &std::path::Path, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..count {
            let mut file = File::create(dir.join(format!("test{}.test.ts", i)))?;
            writeln!(file, "import {{ describe, test, expect, beforeEach, afterEach }} from '@jest/globals';")?;
            writeln!(file, "import {{ Module{} }} from '../src/components/module{}';", i, i)?;
            writeln!(file, "")?;
            writeln!(file, "describe('Module{} Tests', () => {{", i)?;
            writeln!(file, "  let module: Module{};\n", i)?;
            writeln!(file, "  beforeEach(() => {{")?;
            writeln!(file, "    module = new Module{}({{", i)?;
            writeln!(file, "      id: 'test-{}',", i)?;
            writeln!(file, "      name: 'Test Module {}',", i)?;
            writeln!(file, "      active: true")?;
            writeln!(file, "    }});")?;
            writeln!(file, "  }});")?;
            writeln!(file, "")?;
            writeln!(file, "  afterEach(() => {{")?;
            writeln!(file, "    // Cleanup")?;
            writeln!(file, "  }});")?;
            writeln!(file, "")?;
            writeln!(file, "  test('should initialize properly', async () => {{")?;
            writeln!(file, "    await expect(module.initialize()).resolves.toBeUndefined();")?;
            writeln!(file, "  }});")?;
            writeln!(file, "")?;
            writeln!(file, "  test('should process data correctly', async () => {{")?;
            writeln!(file, "    await module.initialize();")?;
            writeln!(file, "    const testData = [{{ id: 1, value: 'test' }}];")?;
            writeln!(file, "    const result = await module.process(testData);")?;
            writeln!(file, "    expect(result).toHaveLength(1);")?;
            writeln!(file, "    expect(result[0]).toHaveProperty('processed', true);")?;
            writeln!(file, "  }});")?;
            writeln!(file, "")?;
            writeln!(file, "  test('should handle empty data', async () => {{")?;
            writeln!(file, "    await module.initialize();")?;
            writeln!(file, "    const result = await module.process([]);")?;
            writeln!(file, "    expect(result).toHaveLength(0);")?;
            writeln!(file, "  }});")?;
            writeln!(file, "")?;
            writeln!(file, "  test('should validate required props', async () => {{")?;
            writeln!(file, "    const invalidModule = new Module{}({{", i)?;
            writeln!(file, "      id: '',")?;
            writeln!(file, "      name: '',")?;
            writeln!(file, "      active: false")?;
            writeln!(file, "    }});")?;
            writeln!(file, "    await expect(invalidModule.initialize()).rejects.toThrow();")?;
            writeln!(file, "  }});")?;
            writeln!(file, "}});")?;
        }
        Ok(())
    }

    fn create_python_files(dir: &std::path::Path, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for i in 0..count {
            let mut file = File::create(dir.join(format!("tool{}.py", i)))?;
            writeln!(file, "#!/usr/bin/env python3")?;
            writeln!(file, "import os")?;
            writeln!(file, "import sys")?;
            writeln!(file, "import json")?;
            writeln!(file, "import asyncio")?;
            writeln!(file, "import logging")?;
            writeln!(file, "from typing import List, Dict, Any, Optional")?;
            writeln!(file, "from dataclasses import dataclass")?;
            writeln!(file, "")?;
            writeln!(file, "@dataclass")?;
            writeln!(file, "class Tool{}Config:", i)?;
            writeln!(file, "    name: str")?;
            writeln!(file, "    version: str")?;
            writeln!(file, "    debug: bool = False")?;
            writeln!(file, "    max_workers: int = 4")?;
            writeln!(file, "    timeout: float = 30.0")?;
            writeln!(file, "")?;
            writeln!(file, "class Tool{}:", i)?;
            writeln!(file, "    def __init__(self, config: Tool{}Config):", i)?;
            writeln!(file, "        self.config = config")?;
            writeln!(file, "        self.logger = self._setup_logging()")?;
            writeln!(file, "        self.tasks = []")?;
            writeln!(file, "        self.results = {{}}")?;
            writeln!(file, "")?;
            writeln!(file, "    def _setup_logging(self) -> logging.Logger:")?;
            writeln!(file, "        logger = logging.getLogger(f'tool{}_main')", i)?;
            writeln!(file, "        level = logging.DEBUG if self.config.debug else logging.INFO")?;
            writeln!(file, "        logger.setLevel(level)")?;
            writeln!(file, "        if not logger.handlers:")?;
            writeln!(file, "            handler = logging.StreamHandler()")?;
            writeln!(file, "            formatter = logging.Formatter(")?;
            writeln!(file, "                '%(asctime)s - %(name)s - %(levelname)s - %(message)s'")?;
            writeln!(file, "            )")?;
            writeln!(file, "            handler.setFormatter(formatter)")?;
            writeln!(file, "            logger.addHandler(handler)")?;
            writeln!(file, "        return logger")?;
            writeln!(file, "")?;
            writeln!(file, "    async def initialize(self) -> None:")?;
            writeln!(file, "        self.logger.info(f'Initializing Tool{} v{{self.config.version}}')", i)?;
            writeln!(file, "        await self._load_configuration()")?;
            writeln!(file, "        await self._validate_environment()")?;
            writeln!(file, "        self.logger.info('Tool{} initialization complete')", i)?;
            writeln!(file, "")?;
            writeln!(file, "    async def _load_configuration(self) -> None:")?;
            writeln!(file, "        config_file = f'config/tool{}.json'", i)?;
            writeln!(file, "        if os.path.exists(config_file):")?;
            writeln!(file, "            try:")?;
            writeln!(file, "                with open(config_file, 'r') as f:")?;
            writeln!(file, "                    config_data = json.load(f)")?;
            writeln!(file, "                self.logger.debug(f'Loaded configuration: {{config_data}}')")?;
            writeln!(file, "            except Exception as e:")?;
            writeln!(file, "                self.logger.error(f'Failed to load configuration: {{e}}')")?;
            writeln!(file, "                raise")?;
            writeln!(file, "")?;
            writeln!(file, "    async def _validate_environment(self) -> None:")?;
            writeln!(file, "        required_vars = ['TOOL{}_KEY', 'TOOL{}_SECRET']", i.to_string().to_uppercase(), i.to_string().to_uppercase())?;
            writeln!(file, "        missing = [var for var in required_vars if not os.getenv(var)]")?;
            writeln!(file, "        if missing:")?;
            writeln!(file, "            raise EnvironmentError(f'Missing required environment variables: {{missing}}')")?;
            writeln!(file, "")?;
            writeln!(file, "    async def process_batch(self, items: List[Dict[str, Any]]) -> List[Dict[str, Any]]:")?;
            writeln!(file, "        self.logger.info(f'Processing batch of {{len(items)}} items')")?;
            writeln!(file, "        semaphore = asyncio.Semaphore(self.config.max_workers)")?;
            writeln!(file, "        tasks = [self._process_item_with_semaphore(semaphore, item) for item in items]")?;
            writeln!(file, "        results = await asyncio.gather(*tasks, return_exceptions=True)")?;
            writeln!(file, "        ")?;
            writeln!(file, "        # Filter out exceptions and log them")?;
            writeln!(file, "        successful_results = []")?;
            writeln!(file, "        for i, result in enumerate(results):")?;
            writeln!(file, "            if isinstance(result, Exception):")?;
            writeln!(file, "                self.logger.error(f'Failed to process item {{i}}: {{result}}')")?;
            writeln!(file, "            else:")?;
            writeln!(file, "                successful_results.append(result)")?;
            writeln!(file, "        ")?;
            writeln!(file, "        self.logger.info(f'Successfully processed {{len(successful_results)}}/{{len(items)}} items')")?;
            writeln!(file, "        return successful_results")?;
            writeln!(file, "")?;
            writeln!(file, "    async def _process_item_with_semaphore(self, semaphore: asyncio.Semaphore, item: Dict[str, Any]) -> Dict[str, Any]:")?;
            writeln!(file, "        async with semaphore:")?;
            writeln!(file, "            return await self._process_item(item)")?;
            writeln!(file, "")?;
            writeln!(file, "    async def _process_item(self, item: Dict[str, Any]) -> Dict[str, Any]:")?;
            writeln!(file, "        try:")?;
            writeln!(file, "            # Simulate processing time")?;
            writeln!(file, "            await asyncio.sleep(0.1)")?;
            writeln!(file, "            processed_item = {{")?;
            writeln!(file, "                **item,")?;
            writeln!(file, "                'processed_by': f'tool{}',", i)?;
            writeln!(file, "                'processed_at': asyncio.get_event_loop().time(),")?;
            writeln!(file, "                'status': 'completed'")?;
            writeln!(file, "            }}")?;
            writeln!(file, "            self.logger.debug(f'Processed item: {{item.get(\"id\", \"unknown\")}}')")?;
            writeln!(file, "            return processed_item")?;
            writeln!(file, "        except Exception as e:")?;
            writeln!(file, "            self.logger.error(f'Error processing item {{item}}: {{e}}')")?;
            writeln!(file, "            raise")?;
            writeln!(file, "")?;
            writeln!(file, "async def main():")?;
            writeln!(file, "    config = Tool{}Config(", i)?;
            writeln!(file, "        name=f'tool{}',", i)?;
            writeln!(file, "        version='1.0.0',")?;
            writeln!(file, "        debug=True")?;
            writeln!(file, "    )")?;
            writeln!(file, "    ")?;
            writeln!(file, "    tool = Tool{}(config)", i)?;
            writeln!(file, "    await tool.initialize()")?;
            writeln!(file, "    ")?;
            writeln!(file, "    # Example usage")?;
            writeln!(file, "    test_items = [")?;
            writeln!(file, "        {{'id': i, 'data': f'test_data_{{i}}'}} for i in range(10)")?;
            writeln!(file, "    ]")?;
            writeln!(file, "    ")?;
            writeln!(file, "    results = await tool.process_batch(test_items)")?;
            writeln!(file, "    print(f'Processed {{len(results)}} items successfully')")?;
            writeln!(file, "")?;
            writeln!(file, "if __name__ == '__main__':")?;
            writeln!(file, "    asyncio.run(main())")?;
        }
        Ok(())
    }

    fn run_fae_command_with_timing(dir: &std::path::Path, args: &[&str]) -> Result<(String, Duration), Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // テスト実行時のパスからバイナリを見つける
        let current_dir = std::env::current_dir()?;
        let binary_path = current_dir.join("target/debug/fae");
        
        let output = Command::new(&binary_path)
            .args(args)
            .current_dir(dir)
            .output()?;

        let elapsed = start_time.elapsed();

        if !output.status.success() {
            return Err(format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }

        Ok((String::from_utf8_lossy(&output.stdout).to_string(), elapsed))
    }

    #[test]
    fn test_index_build_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // インデックス構築性能テスト
        let start_time = Instant::now();
        
        // 最初の検索でインデックス構築を実行
        let (result, search_time) = run_fae_command_with_timing(project_path, &["Logger"])?;
        
        let total_time = start_time.elapsed();

        // パフォーマンス基準
        // - 総実行時間: 10秒以内
        // - 結果が得られること
        assert!(total_time.as_secs() < 10, "Index build took too long: {}ms", total_time.as_millis());
        assert!(!result.is_empty(), "No results found");

        println!("Index build performance:");
        println!("  Total time: {}ms", total_time.as_millis());
        println!("  Search time: {}ms", search_time.as_millis());
        println!("  Results length: {} characters", result.len());

        Ok(())
    }

    #[test]
    fn test_symbol_search_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // ウォームアップ（インデックス構築）
        let _ = run_fae_command_with_timing(project_path, &["warmup"])?;

        // シンボル検索性能テスト
        let test_queries = ["#Module", "#Helper", "#Config", "#Logger", "#initialize"];
        
        for query in &test_queries {
            let (result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            // 各検索は3秒以内で完了するはず
            assert!(elapsed.as_secs() < 3, "Symbol search '{}' took too long: {}ms", query, elapsed.as_millis());
            
            println!("Symbol search '{}': {}ms, {} chars", query, elapsed.as_millis(), result.len());
        }

        Ok(())
    }

    #[test]
    fn test_content_search_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // ウォームアップ
        let _ = run_fae_command_with_timing(project_path, &["warmup"])?;

        // コンテンツ検索性能テスト
        let test_queries = ["async", "console.log", "import", "class", "function"];
        
        for query in &test_queries {
            let (result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            // 各検索は5秒以内で完了するはず
            assert!(elapsed.as_secs() < 5, "Content search '{}' took too long: {}ms", query, elapsed.as_millis());
            
            println!("Content search '{}': {}ms, {} chars", query, elapsed.as_millis(), result.len());
        }

        Ok(())
    }

    #[test]
    fn test_file_search_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // ファイル検索性能テスト
        let test_queries = [">module", ">helper", ">test", ">tool", ">config"];
        
        for query in &test_queries {
            let (result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            // ファイル検索は特に高速であるべき（1秒以内）
            assert!(elapsed.as_secs() < 1, "File search '{}' took too long: {}ms", query, elapsed.as_millis());
            
            println!("File search '{}': {}ms, {} chars", query, elapsed.as_millis(), result.len());
        }

        Ok(())
    }

    #[test]
    fn test_regex_search_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // ウォームアップ
        let _ = run_fae_command_with_timing(project_path, &["warmup"])?;

        // 正規表現検索性能テスト
        let test_queries = [
            r"/async\s+function",
            r"/console\.\w+",
            r"/class\s+\w+",
            r"/import\s+\{",
            r"/\.then\("
        ];
        
        for query in &test_queries {
            let (result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            // 正規表現検索は複雑だが、それでも5秒以内
            assert!(elapsed.as_secs() < 5, "Regex search '{}' took too long: {}ms", query, elapsed.as_millis());
            
            println!("Regex search '{}': {}ms, {} chars", query, elapsed.as_millis(), result.len());
        }

        Ok(())
    }

    #[test]
    fn test_concurrent_search_performance() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // ウォームアップ
        let _ = run_fae_command_with_timing(project_path, &["warmup"])?;

        // 連続検索性能テスト（実際の使用パターンをシミュレート）
        let search_sequence = [
            "Logger",           // 一般的な検索
            "#initialize",      // シンボル検索
            ">module",          // ファイル検索
            "/async.*function", // 正規表現検索
            "config",           // 再び一般的な検索
        ];

        let total_start = Instant::now();
        
        for (i, query) in search_sequence.iter().enumerate() {
            let (result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            println!("Search {}: '{}' - {}ms, {} chars", i+1, query, elapsed.as_millis(), result.len());
            
            // 個別の検索は5秒以内
            assert!(elapsed.as_secs() < 5, "Search '{}' took too long: {}ms", query, elapsed.as_millis());
        }

        let total_elapsed = total_start.elapsed();
        
        // 連続検索全体は15秒以内
        assert!(total_elapsed.as_secs() < 15, "Total search sequence took too long: {}ms", total_elapsed.as_millis());
        
        println!("Total sequence time: {}ms", total_elapsed.as_millis());

        Ok(())
    }

    #[test]
    fn test_memory_efficiency() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // メモリ効率テスト（間接的）
        // 大量の検索を実行してもクラッシュしないことを確認
        let queries = vec![
            "test", "class", "function", "import", "export",
            "#Module", "#Helper", "#Config", "#Logger", "#Tool",
            ">src", ">lib", ">tests", ">scripts", ">docs",
            "/\\w+Error", "/async.*", "/console\\.", "/new\\s+", "/require\\("
        ];

        for (i, query) in queries.iter().enumerate() {
            let (_result, elapsed) = run_fae_command_with_timing(project_path, &[query])?;
            
            // メモリ不足によるクラッシュがないことを確認
            assert!(elapsed.as_secs() < 10, "Search {} '{}' may indicate memory issues: {}ms", i, query, elapsed.as_millis());
            
            if i % 5 == 0 {
                println!("Memory test progress: {}/{} searches completed", i+1, queries.len());
            }
        }

        println!("Memory efficiency test completed successfully");

        Ok(())
    }

    #[test] 
    fn test_scalability_limits() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = create_large_project()?;
        let project_path = temp_dir.path();

        // スケーラビリティ制限テスト
        
        // 1. 非常に長いクエリ
        let long_query = "a".repeat(1000);
        let (_result, elapsed) = run_fae_command_with_timing(project_path, &[&long_query])?;
        assert!(elapsed.as_secs() < 10, "Long query took too long: {}ms", elapsed.as_millis());
        println!("Long query test: {}ms", elapsed.as_millis());

        // 2. 特殊文字を含むクエリ
        let special_chars = ["{{", "}}", "[", "]", "(", ")", "*", "+", "?", "^", "$"];
        for special in &special_chars {
            let (_, elapsed) = run_fae_command_with_timing(project_path, &[special])?;
            assert!(elapsed.as_secs() < 5, "Special char '{}' took too long: {}ms", special, elapsed.as_millis());
        }

        // 3. 空文字列クエリ
        let (_, elapsed) = run_fae_command_with_timing(project_path, &[""])?;
        assert!(elapsed.as_secs() < 3, "Empty query took too long: {}ms", elapsed.as_millis());

        println!("Scalability limits test completed");

        Ok(())
    }
}