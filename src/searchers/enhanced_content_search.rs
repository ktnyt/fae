use crate::types::SearchResult;
use crate::searchers::backend::BackendDetector;
use crate::searchers::ContentSearcher;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

/// 外部バックエンド対応のコンテンツ検索エンジン
pub struct EnhancedContentSearcher {
    project_root: PathBuf,
    backend_detector: Arc<BackendDetector>,
    fallback_searcher: ContentSearcher,
}

/// 拡張コンテンツ検索のストリーミングイテレーター
pub struct EnhancedContentSearchStream {
    receiver: mpsc::Receiver<SearchResult>,
    _handle: thread::JoinHandle<()>,
}

impl Iterator for EnhancedContentSearchStream {
    type Item = SearchResult;
    
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

impl EnhancedContentSearcher {
    /// 新しいEnhancedContentSearcherを作成
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let backend_detector = Arc::new(BackendDetector::new(&project_root)?);
        let fallback_searcher = ContentSearcher::new(project_root.clone())?;
        
        Ok(Self {
            project_root,
            backend_detector,
            fallback_searcher,
        })
    }
    
    /// ストリーミング検索を実行（外部バックエンド優先）
    pub fn search_stream(&self, query: &str) -> Result<EnhancedContentSearchStream> {
        // 空のクエリは空のストリームを返す
        if query.trim().is_empty() {
            let (sender, receiver) = mpsc::channel();
            drop(sender);
            let handle = thread::spawn(|| {});
            return Ok(EnhancedContentSearchStream { receiver, _handle: handle });
        }
        
        let query = query.to_string();
        let project_root = self.project_root.clone();
        let backend_detector = Arc::clone(&self.backend_detector);
        let fallback_searcher = self.fallback_searcher.clone();
        
        let (sender, receiver) = mpsc::channel();
        
        let handle = thread::spawn(move || {
            // まず外部バックエンドを試行
            match backend_detector.search_content(&project_root, &query) {
                Ok(results) => {
                    // 各バックエンドの自然な順序を保持してそのまま送信
                    for result in results {
                        if sender.send(result).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => {
                    // 外部バックエンドが失敗した場合はフォールバックを使用
                    if let Ok(stream) = fallback_searcher.search_stream(&query) {
                        for result in stream {
                            if sender.send(result).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
        });
        
        Ok(EnhancedContentSearchStream { receiver, _handle: handle })
    }
    
    /// バッチ検索を実行（外部バックエンド優先）
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        
        // まず外部バックエンドを試行
        match self.backend_detector.search_content(&self.project_root, query) {
            Ok(mut results) => {
                // 各バックエンドの自然な順序を保持
                results.truncate(limit);
                Ok(results)
            }
            Err(_) => {
                // フォールバックを使用
                self.fallback_searcher.search(query, limit)
            }
        }
    }
    
    /// 利用中のバックエンド情報を取得
    pub fn backend_info(&self) -> (String, Vec<String>) {
        let primary = self.backend_detector.primary_backend().to_string();
        let available: Vec<String> = self.backend_detector.available_backends()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        
        (primary, available)
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
    fn test_enhanced_searcher_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = EnhancedContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(searcher.project_root(), temp_dir.path());
        
        let (primary, available) = searcher.backend_info();
        assert!(!available.is_empty());
        assert!(!primary.is_empty());
        
        Ok(())
    }

    #[test]
    fn test_enhanced_search_with_fallback() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_test_file(&temp_dir, "test.ts", "function hello() { console.log('world'); }")?;
        
        let searcher = EnhancedContentSearcher::new(temp_dir.path().to_path_buf())?;
        let results = searcher.search("hello", 10)?;
        
        // フォールバックが動作して結果が得られることを確認
        assert!(results.len() > 0);
        
        Ok(())
    }

    #[test]
    fn test_enhanced_streaming_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_test_file(&temp_dir, "test1.ts", "function hello() { console.log('world'); }")?;
        create_test_file(&temp_dir, "test2.rs", "fn hello() { println!('world'); }")?;
        
        let searcher = EnhancedContentSearcher::new(temp_dir.path().to_path_buf())?;
        let stream = searcher.search_stream("hello")?;
        
        let results: Vec<_> = stream.collect();
        assert!(results.len() > 0);
        
        Ok(())
    }

    #[test]
    fn test_enhanced_search_empty_query() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = EnhancedContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        let results = searcher.search("", 10)?;
        assert_eq!(results.len(), 0);
        
        let stream = searcher.search_stream("")?;
        let stream_results: Vec<_> = stream.collect();
        assert_eq!(stream_results.len(), 0);
        
        Ok(())
    }
}