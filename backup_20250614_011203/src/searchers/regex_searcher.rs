use crate::types::SearchResult;
use crate::searchers::backend::BackendDetector;
use crate::searchers::ContentSearcher;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

/// 正規表現検索エンジン
/// 
/// ripgrepバックエンドを優先して正規表現検索を実行し、
/// フォールバック時は内蔵regex crateを使用する。
pub struct RegexSearcher {
    project_root: PathBuf,
    backend_detector: Arc<RegexBackendDetector>,
    fallback_searcher: ContentSearcher,
}

/// 正規表現検索のストリーミングイテレーター
pub struct RegexSearchStream {
    receiver: mpsc::Receiver<SearchResult>,
    _handle: thread::JoinHandle<()>,
}

impl Iterator for RegexSearchStream {
    type Item = SearchResult;
    
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

/// 正規表現検索用のバックエンド検出器
struct RegexBackendDetector {
    backend_detector: BackendDetector,
}

impl RegexBackendDetector {
    fn new(project_root: &Path) -> Result<Self> {
        let backend_detector = BackendDetector::new(project_root)?;
        Ok(Self { backend_detector })
    }
    
    /// 正規表現検索を実行
    fn search_regex(&self, project_root: &Path, pattern: &str) -> Result<Vec<SearchResult>> {
        // BackendDetectorの専用正規表現検索メソッドを使用
        self.backend_detector.search_regex(project_root, pattern)
    }
    
    /// 利用中のバックエンド情報を取得
    fn backend_info(&self) -> (String, Vec<String>) {
        let primary = self.backend_detector.primary_backend().to_string();
        let available: Vec<String> = self.backend_detector.available_backends()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        
        (primary, available)
    }
}

impl RegexSearcher {
    /// 新しいRegexSearcherを作成
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let backend_detector = Arc::new(RegexBackendDetector::new(&project_root)?);
        let fallback_searcher = ContentSearcher::new(project_root.clone())?;
        
        Ok(Self {
            project_root,
            backend_detector,
            fallback_searcher,
        })
    }
    
    /// ストリーミング正規表現検索を実行
    pub fn search_stream(&self, pattern: &str) -> Result<RegexSearchStream> {
        // 空のパターンは空のストリームを返す
        if pattern.trim().is_empty() {
            let (sender, receiver) = mpsc::channel();
            drop(sender);
            let handle = thread::spawn(|| {});
            return Ok(RegexSearchStream { receiver, _handle: handle });
        }
        
        let pattern = pattern.to_string();
        let project_root = self.project_root.clone();
        let backend_detector = Arc::clone(&self.backend_detector);
        let fallback_searcher = self.fallback_searcher.clone();
        
        let (sender, receiver) = mpsc::channel();
        
        let handle = thread::spawn(move || {
            // まず外部バックエンドで正規表現検索を試行
            match backend_detector.search_regex(&project_root, &pattern) {
                Ok(results) => {
                    // 各バックエンドの自然な順序を保持してそのまま送信
                    for result in results {
                        if sender.send(result).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => {
                    // 外部バックエンドが失敗した場合はフォールバック
                    // TODO: 内蔵regex crateを使った正規表現検索を実装
                    if let Ok(stream) = fallback_searcher.search_stream(&pattern) {
                        for result in stream {
                            if sender.send(result).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        });
        
        Ok(RegexSearchStream { receiver, _handle: handle })
    }
    
    /// バッチ正規表現検索を実行
    pub fn search(&self, pattern: &str, limit: usize) -> Result<Vec<SearchResult>> {
        if pattern.trim().is_empty() {
            return Ok(Vec::new());
        }
        
        // まず外部バックエンドを試行
        match self.backend_detector.search_regex(&self.project_root, pattern) {
            Ok(mut results) => {
                // 各バックエンドの自然な順序を保持
                results.truncate(limit);
                Ok(results)
            }
            Err(_) => {
                // フォールバックを使用
                self.fallback_searcher.search(pattern, limit)
            }
        }
    }
    
    /// 利用中のバックエンド情報を取得
    pub fn backend_info(&self) -> (String, Vec<String>) {
        self.backend_detector.backend_info()
    }
    
    /// プロジェクトルートを取得
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_test_file(temp_dir: &TempDir, filename: &str, content: &str) -> Result<PathBuf> {
        let file_path = temp_dir.path().join(filename);
        let mut file = File::create(&file_path)?;
        write!(file, "{}", content)?;
        Ok(file_path)
    }

    #[test]
    fn test_regex_searcher_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = RegexSearcher::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(searcher.project_root(), temp_dir.path());
        
        let (primary, available) = searcher.backend_info();
        assert!(!available.is_empty());
        assert!(!primary.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_regex_search_with_fallback() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_test_file(&temp_dir, "test.rs", "fn main() { println!(\"hello world\"); }")?;
        
        let searcher = RegexSearcher::new(temp_dir.path().to_path_buf())?;
        let results = searcher.search("hello", 10)?;
        
        // フォールバックが動作して結果が得られることを確認
        assert!(results.len() > 0);
        
        Ok(())
    }

    #[test]
    fn test_regex_streaming_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_test_file(&temp_dir, "test1.rs", "fn hello() { println!(\"world\"); }")?;
        create_test_file(&temp_dir, "test2.js", "function hello() { console.log('world'); }")?;
        
        let searcher = RegexSearcher::new(temp_dir.path().to_path_buf())?;
        let stream = searcher.search_stream("hello")?;
        
        let results: Vec<_> = stream.collect();
        assert!(results.len() > 0);
        
        Ok(())
    }

    #[test]
    fn test_regex_search_empty_pattern() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = RegexSearcher::new(temp_dir.path().to_path_buf())?;
        
        let results = searcher.search("", 10)?;
        assert_eq!(results.len(), 0);
        
        let stream = searcher.search_stream("")?;
        let stream_results: Vec<_> = stream.collect();
        assert_eq!(stream_results.len(), 0);
        
        Ok(())
    }
}