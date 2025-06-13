use crate::types::{SearchResult, DisplayInfo};
use crate::index_manager::IndexManager;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use log::debug;
use ignore::{WalkBuilder, DirEntry};

/// ファイル・ディレクトリ検索エンジン
#[derive(Clone)]
pub struct FileSearcher {
    /// ファイル発見エンジン
    _index_manager: IndexManager,
    /// プロジェクトルート
    project_root: PathBuf,
}

/// ファイル検索結果のイテレーター
pub struct FileSearchStream {
    receiver: mpsc::Receiver<SearchResult>,
    _handle: thread::JoinHandle<()>,
}

impl Iterator for FileSearchStream {
    type Item = SearchResult;
    
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

/// ファイル/ディレクトリ情報
#[derive(Debug, Clone)]
pub struct PathInfo {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub is_directory: bool,
}

impl FileSearcher {
    /// 新しいFileSearcherを作成
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let index_manager = IndexManager::new(project_root.clone());
        
        Ok(Self {
            _index_manager: index_manager,
            project_root,
        })
    }

    /// ストリーミングファイル検索を実行
    pub fn search_stream(&self, query: &str) -> Result<FileSearchStream> {
        // 空のクエリは空のストリームを返す
        if query.trim().is_empty() {
            let (sender, receiver) = mpsc::channel();
            drop(sender); // すぐにチャンネルを閉じる
            let handle = thread::spawn(|| {}); // 空のスレッド
            return Ok(FileSearchStream { receiver, _handle: handle });
        }

        let query = query.to_string();
        let searcher = self.clone();
        let (sender, receiver) = mpsc::channel();

        let handle = thread::spawn(move || {
            // ファイル・ディレクトリ一覧を取得
            let paths = match searcher.discover_all_paths() {
                Ok(paths) => {
                    debug!("File search discovered {} paths", paths.len());
                    paths
                }
                Err(err) => {
                    log::warn!("Failed to discover paths: {}", err);
                    return;
                }
            };

            // ファジー検索でマッチするパスを抽出
            let mut matches = searcher.fuzzy_match_paths(&paths, &query);
            
            // スコア順でソート（高い順）
            matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            // 結果を送信
            for result in matches {
                if sender.send(result).is_err() {
                    // receiverが閉じられた場合は終了
                    return;
                }
            }
        });

        Ok(FileSearchStream { receiver, _handle: handle })
    }

    /// プロジェクトルートを取得
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// ファイルとディレクトリの一覧を取得
    fn discover_all_paths(&self) -> Result<Vec<PathInfo>> {
        let mut paths = Vec::new();
        
        let walker = WalkBuilder::new(&self.project_root)
            .standard_filters(true)  // .gitignore, .ignore, hidden filesを自動処理
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if let Some(path_info) = self.process_path_entry(entry)? {
                        paths.push(path_info);
                    }
                }
                Err(err) => {
                    log::warn!("Failed to access path: {}", err);
                }
            }
        }

        Ok(paths)
    }

    /// DirEntryを処理してPathInfoに変換
    fn process_path_entry(&self, entry: DirEntry) -> Result<Option<PathInfo>> {
        let path = entry.path();
        
        // プロジェクトルート自体はスキップ
        if path == self.project_root {
            return Ok(None);
        }

        // 相対パスを計算
        let relative_path = path.strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_path_buf();

        let is_directory = path.is_dir();

        Ok(Some(PathInfo {
            path: path.to_path_buf(),
            relative_path,
            is_directory,
        }))
    }

    /// ファジーマッチングでパスを検索
    fn fuzzy_match_paths(&self, paths: &[PathInfo], query: &str) -> Vec<SearchResult> {
        use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
        
        let matcher = SkimMatcherV2::default();
        let mut results = Vec::new();

        for path_info in paths {
            // ファイル名またはディレクトリ名で検索
            let file_name = path_info.relative_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");

            // フルパスでも検索（より包括的）
            let full_path_str = path_info.relative_path.to_string_lossy();

            // ファイル名とフルパスの両方でマッチングを試行
            let score = matcher.fuzzy_match(file_name, query)
                .or_else(|| matcher.fuzzy_match(&full_path_str, query));

            if let Some(score) = score {
                let result = SearchResult {
                    file_path: path_info.path.clone(),
                    line: 1, // ファイル検索では行番号は意味がない
                    column: 1,
                    display_info: DisplayInfo::File {
                        path: path_info.relative_path.clone(),
                        is_directory: path_info.is_directory,
                    },
                    score: score as f64,
                };
                
                results.push(result);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_directory_structure() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path();

        // ディレクトリ構造を作成
        fs::create_dir(root.join("src"))?;
        fs::create_dir(root.join("tests"))?;
        fs::create_dir(root.join("docs"))?;
        fs::create_dir(root.join("src").join("utils"))?;

        // ファイルを作成
        fs::write(root.join("src").join("main.rs"), "fn main() {}")?;
        fs::write(root.join("src").join("lib.rs"), "// lib")?;
        fs::write(root.join("src").join("utils").join("helper.rs"), "// helper")?;
        fs::write(root.join("tests").join("test_main.rs"), "// test")?;
        fs::write(root.join("README.md"), "# README")?;

        Ok(temp_dir)
    }

    #[test]
    fn test_file_searcher_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(searcher.project_root(), temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_discover_all_paths() -> Result<()> {
        let temp_dir = create_test_directory_structure()?;
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf())?;
        
        let paths = searcher.discover_all_paths()?;
        
        // ファイルとディレクトリの両方が見つかるはず
        assert!(!paths.is_empty());
        
        // ディレクトリが含まれているかチェック
        let has_directory = paths.iter().any(|p| p.is_directory);
        assert!(has_directory);
        
        // ファイルが含まれているかチェック
        let has_file = paths.iter().any(|p| !p.is_directory);
        assert!(has_file);
        
        Ok(())
    }

    #[test]
    fn test_file_search_stream() -> Result<()> {
        let temp_dir = create_test_directory_structure()?;
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf())?;
        
        let stream = searcher.search_stream("main")?;
        let results: Vec<_> = stream.collect();
        
        // "main"を含むファイルが見つかるはず（main.rs, test_main.rs）
        assert!(!results.is_empty());
        
        // 結果がスコア順でソートされているかチェック
        for i in 1..results.len() {
            assert!(results[i-1].score >= results[i].score);
        }
        
        Ok(())
    }

    #[test]
    fn test_file_search_empty_query() -> Result<()> {
        let temp_dir = create_test_directory_structure()?;
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf())?;
        
        let stream = searcher.search_stream("")?;
        let results: Vec<_> = stream.collect();
        
        // 空クエリは結果なし
        assert_eq!(results.len(), 0);
        
        Ok(())
    }

    #[test]
    fn test_directory_search() -> Result<()> {
        let temp_dir = create_test_directory_structure()?;
        let searcher = FileSearcher::new(temp_dir.path().to_path_buf())?;
        
        let stream = searcher.search_stream("utils")?;
        let results: Vec<_> = stream.collect();
        
        // "utils"ディレクトリが見つかるはず
        assert!(!results.is_empty());
        
        // ディレクトリが含まれているかチェック
        let has_directory = results.iter().any(|r| {
            matches!(&r.display_info, DisplayInfo::File { is_directory: true, .. })
        });
        assert!(has_directory);
        
        Ok(())
    }
}