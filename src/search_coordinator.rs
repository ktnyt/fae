use crate::index_manager::{IndexManager, FileInfo};
use crate::cache_manager::CacheManager;
use crate::symbol_index::{SymbolIndex, SymbolMetadata, MetadataStorage, SearchHit};
use crate::tree_sitter::extract_symbols_from_file;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

/// インデックス構築とマルチモード検索を調整するコーディネーター
pub struct SearchCoordinator {
    /// ファイル発見エンジン
    index_manager: IndexManager,
    /// キャッシュマネージャー
    cache_manager: CacheManager,
    /// メタデータストレージ
    metadata_storage: Option<MetadataStorage>,
    /// プロジェクトルート
    project_root: PathBuf,
    /// 構築済みシンボルインデックス
    symbol_index: Option<SymbolIndex>,
}

/// インデックス構築の進捗情報
#[derive(Debug, Clone)]
pub struct IndexProgress {
    /// 処理済みファイル数
    pub processed_files: usize,
    /// 総ファイル数
    pub total_files: usize,
    /// 抽出されたシンボル数
    pub total_symbols: usize,
    /// 経過時間（ミリ秒）
    pub elapsed_ms: u64,
    /// 現在処理中のファイル
    pub current_file: Option<PathBuf>,
}

/// インデックス構築結果
#[derive(Debug)]
pub struct IndexResult {
    /// 構築したシンボルインデックス
    pub symbol_index: SymbolIndex,
    /// 処理したファイル数
    pub processed_files: usize,
    /// 抽出したシンボル数
    pub total_symbols: usize,
    /// 構築時間（ミリ秒）
    pub build_time_ms: u64,
    /// エラーが発生したファイル数
    pub error_files: usize,
}

impl SearchCoordinator {
    /// 新しいSearchCoordinatorを作成
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let index_manager = IndexManager::new(project_root.clone());
        let cache_manager = CacheManager::new();
        let metadata_storage = MetadataStorage::new(&project_root).ok();

        Ok(Self {
            index_manager,
            cache_manager,
            metadata_storage,
            project_root,
            symbol_index: None,
        })
    }

    /// プロジェクト全体のインデックスを構築
    pub fn build_index(&mut self) -> Result<IndexResult> {
        let start_time = Instant::now();
        
        // ファイル発見
        let files = self.index_manager.discover_files()
            .context("Failed to discover files")?;
        
        println!("Found {} files to index", files.len());

        // 並列シンボル抽出
        let symbols = self.extract_symbols_parallel(&files)?;
        
        // シンボルインデックス構築
        let symbol_index = SymbolIndex::from_symbols(symbols.clone());
        
        // 内部にシンボルインデックスを保存
        self.symbol_index = Some(symbol_index.clone());
        
        // メタデータストレージに保存
        if let Some(ref mut storage) = self.metadata_storage {
            storage.save_metadata(symbols.clone())
                .context("Failed to save metadata")?;
        }

        let build_time_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(IndexResult {
            symbol_index,
            processed_files: files.len(),
            total_symbols: symbols.len(),
            build_time_ms,
            error_files: 0, // TODO: エラー数を追跡
        })
    }

    /// プログレッシブインデックス構築（非ブロッキング）
    pub fn build_index_progressive(
        &mut self,
        progress_sender: mpsc::Sender<IndexProgress>
    ) -> Result<IndexResult> {
        let start_time = Instant::now();
        
        // ファイル発見
        let files = self.index_manager.discover_files()
            .context("Failed to discover files")?;
        
        let total_files = files.len();
        let processed_files = Arc::new(Mutex::new(0usize));
        let total_symbols = Arc::new(Mutex::new(0usize));
        let error_files = Arc::new(Mutex::new(0usize));
        
        // 初期進捗報告
        let _ = progress_sender.send(IndexProgress {
            processed_files: 0,
            total_files,
            total_symbols: 0,
            elapsed_ms: 0,
            current_file: None,
        });

        // 並列シンボル抽出（進捗報告付き）
        let symbols: Vec<SymbolMetadata> = files
            .par_iter()
            .filter_map(|file_info| {
                // 進捗報告
                let current_file = Some(file_info.relative_path.clone());
                
                let result = extract_symbols_from_file(&file_info.path);
                
                match result {
                    Ok(file_symbols) => {
                        // 進捗更新
                        {
                            let mut processed = processed_files.lock().unwrap();
                            *processed += 1;
                            
                            let mut symbols_count = total_symbols.lock().unwrap();
                            *symbols_count += file_symbols.len();
                            
                            // 進捗報告
                            let _ = progress_sender.send(IndexProgress {
                                processed_files: *processed,
                                total_files,
                                total_symbols: *symbols_count,
                                elapsed_ms: start_time.elapsed().as_millis() as u64,
                                current_file,
                            });
                        }
                        
                        Some(file_symbols)
                    }
                    Err(_err) => {
                        // エラーカウント更新
                        {
                            let mut processed = processed_files.lock().unwrap();
                            *processed += 1;
                            
                            let mut errors = error_files.lock().unwrap();
                            *errors += 1;
                            
                            // 進捗報告（エラーとして）
                            let _ = progress_sender.send(IndexProgress {
                                processed_files: *processed,
                                total_files,
                                total_symbols: *total_symbols.lock().unwrap(),
                                elapsed_ms: start_time.elapsed().as_millis() as u64,
                                current_file,
                            });
                        }
                        
                        None
                    }
                }
            })
            .flatten()
            .collect();

        // シンボルインデックス構築
        let symbol_index = SymbolIndex::from_symbols(symbols.clone());
        
        // 内部にシンボルインデックスを保存
        self.symbol_index = Some(symbol_index.clone());
        
        // メタデータストレージに保存
        if let Some(ref mut storage) = self.metadata_storage {
            storage.save_metadata(symbols.clone())
                .context("Failed to save metadata")?;
        }

        let build_time_ms = start_time.elapsed().as_millis() as u64;
        let final_processed = *processed_files.lock().unwrap();
        let final_errors = *error_files.lock().unwrap();
        
        Ok(IndexResult {
            symbol_index,
            processed_files: final_processed,
            total_symbols: symbols.len(),
            build_time_ms,
            error_files: final_errors,
        })
    }

    /// ファジーシンボル検索
    pub fn search_symbols(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        if let Some(ref symbol_index) = self.symbol_index {
            symbol_index.fuzzy_search(query, limit)
        } else {
            // フォールバックとしてキャッシュマネージャーを使用
            self.cache_manager.fuzzy_search_symbols(query, limit)
        }
    }

    /// シンボル詳細情報取得
    pub fn get_symbol_details(&self, symbol_name: &str) -> Vec<SymbolMetadata> {
        // まずキャッシュから取得を試行
        let cache_details = self.cache_manager.get_symbol_details(symbol_name);
        
        if !cache_details.is_empty() {
            return cache_details;
        }
        
        // メタデータストレージから取得
        if let Some(ref _storage) = self.metadata_storage {
            if let Ok(mut storage_mut) = MetadataStorage::new(&self.project_root) {
                if let Ok(metadata) = storage_mut.find_metadata(symbol_name) {
                    return metadata;
                }
            }
        }
        
        Vec::new()
    }

    /// プロジェクトルートを取得
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }

    /// IndexManagerの参照を取得
    pub fn index_manager(&self) -> &IndexManager {
        &self.index_manager
    }

    /// CacheManagerの可変参照を取得
    pub fn cache_manager_mut(&mut self) -> &mut CacheManager {
        &mut self.cache_manager
    }

    /// 並列シンボル抽出
    fn extract_symbols_parallel(&self, files: &[FileInfo]) -> Result<Vec<SymbolMetadata>> {
        let symbols: Vec<SymbolMetadata> = files
            .par_iter()
            .filter_map(|file_info| {
                match extract_symbols_from_file(&file_info.path) {
                    Ok(file_symbols) => Some(file_symbols),
                    Err(_) => {
                        // ログ出力してスキップ
                        eprintln!("Warning: Failed to extract symbols from: {}", 
                                  file_info.relative_path.display());
                        None
                    }
                }
            })
            .flatten()
            .collect();

        Ok(symbols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::{self, File};
    use std::io::Write;

    fn create_test_project() -> Result<TempDir> {
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
        writeln!(ts_file, "  return `${{data.name}} is ${{data.age}} years old`;")?;
        writeln!(ts_file, "}}")?;

        // Python ファイル
        let mut py_file = File::create(src_dir.join("calculator.py"))?;
        writeln!(py_file, "class Calculator:")?;
        writeln!(py_file, "    def add(self, a, b):")?;
        writeln!(py_file, "        return a + b")?;
        writeln!(py_file, "")?;
        writeln!(py_file, "def main():")?;
        writeln!(py_file, "    calc = Calculator()")?;
        writeln!(py_file, "    print(calc.add(2, 3))")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_search_coordinator_creation() -> Result<()> {
        let temp_dir = create_test_project()?;
        let _coordinator = SearchCoordinator::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(_coordinator.project_root(), temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_build_index() -> Result<()> {
        let temp_dir = create_test_project()?;
        let mut coordinator = SearchCoordinator::new(temp_dir.path().to_path_buf())?;
        
        let result = coordinator.build_index()?;
        
        // 2つのファイルが処理されたはず
        assert_eq!(result.processed_files, 2);
        
        // シンボルが抽出されたはず
        assert!(result.total_symbols > 0);
        
        // 構築時間が記録されたはず
        assert!(result.build_time_ms > 0);
        
        Ok(())
    }

    #[test]
    fn test_progressive_index_build() -> Result<()> {
        let temp_dir = create_test_project()?;
        let _coordinator = SearchCoordinator::new(temp_dir.path().to_path_buf())?;
        
        let (sender, receiver) = mpsc::channel();
        
        // 別スレッドでプログレッシブ構築
        let coordinator_clone = temp_dir.path().to_path_buf();
        let handle = std::thread::spawn(move || {
            let mut _coord = SearchCoordinator::new(coordinator_clone).unwrap();
            _coord.build_index_progressive(sender)
        });
        
        // 進捗メッセージを受信
        let mut progress_messages = Vec::new();
        while let Ok(progress) = receiver.recv() {
            progress_messages.push(progress);
            if progress_messages.last().unwrap().processed_files >= 2 {
                break; // 全ファイル処理完了
            }
        }
        
        let result = handle.join().unwrap()?;
        
        // 進捗メッセージが送信されたはず
        assert!(!progress_messages.is_empty());
        
        // 最終結果の確認
        assert_eq!(result.processed_files, 2);
        assert!(result.total_symbols > 0);
        
        Ok(())
    }

    #[test]
    fn test_symbol_search() -> Result<()> {
        let temp_dir = create_test_project()?;
        let mut coordinator = SearchCoordinator::new(temp_dir.path().to_path_buf())?;
        
        // インデックス構築
        coordinator.build_index()?;
        
        // シンボル検索
        let results = coordinator.search_symbols("User", 5);
        
        // UserData や processUser などがヒットするはず
        assert!(!results.is_empty());
        
        Ok(())
    }
}