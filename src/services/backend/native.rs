//! Native Rust バックエンド実装
//! 
//! 外部ツールに依存しない純粋なRust実装のフォールバックバックエンドです。
//! ripgrepやagが利用できない環境でも確実に動作します。

use super::{BackendType, SearchBackend, SearchMatch};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;
use walkdir::WalkDir;

/// Native Rust バックエンド
#[derive(Debug)]
pub struct NativeBackend {
    /// 除外するファイル拡張子
    excluded_extensions: Vec<String>,
    /// 除外するディレクトリ名
    excluded_dirs: Vec<String>,
    /// 最大ファイルサイズ（バイト）
    max_file_size: u64,
}

impl NativeBackend {
    /// 新しいNativeバックエンドを作成
    pub fn new() -> Self {
        Self {
            excluded_extensions: vec![
                // バイナリファイル
                "exe".to_string(), "dll".to_string(), "so".to_string(), "dylib".to_string(),
                "bin".to_string(), "obj".to_string(), "o".to_string(), "a".to_string(),
                // 画像・メディア
                "png".to_string(), "jpg".to_string(), "jpeg".to_string(), "gif".to_string(),
                "mp4".to_string(), "avi".to_string(), "mp3".to_string(), "wav".to_string(),
                // アーカイブ
                "zip".to_string(), "tar".to_string(), "gz".to_string(), "bz2".to_string(),
                "rar".to_string(), "7z".to_string(),
                // その他
                "pdf".to_string(), "doc".to_string(), "docx".to_string(),
            ],
            excluded_dirs: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".svn".to_string(),
                ".hg".to_string(),
                "__pycache__".to_string(),
                ".pytest_cache".to_string(),
                "build".to_string(),
                "dist".to_string(),
            ],
            max_file_size: 10 * 1024 * 1024, // 10MB
        }
    }
    
    /// カスタム設定でNativeバックエンドを作成
    pub fn with_config(
        excluded_extensions: Vec<String>,
        excluded_dirs: Vec<String>,
        max_file_size: u64,
    ) -> Self {
        Self {
            excluded_extensions,
            excluded_dirs,
            max_file_size,
        }
    }
    
    /// ファイルが検索対象かどうかを判定
    fn should_search_file(&self, path: &std::path::Path) -> bool {
        // ディレクトリチェック
        if let Some(parent) = path.parent() {
            for excluded_dir in &self.excluded_dirs {
                if parent.components().any(|c| c.as_os_str() == excluded_dir.as_str()) {
                    return false;
                }
            }
        }
        
        // ファイル拡張子チェック
        if let Some(extension) = path.extension() {
            if let Some(ext_str) = extension.to_str() {
                if self.excluded_extensions.contains(&ext_str.to_lowercase()) {
                    return false;
                }
            }
        }
        
        // ファイルサイズチェック（メタデータが取得できる場合のみ）
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > self.max_file_size {
                return false;
            }
        }
        
        true
    }
    
    /// ファイル内でリテラル検索を実行
    async fn search_in_file<F>(
        &self,
        file_path: &std::path::Path,
        query: &str,
        result_callback: &F,
        cancellation_token: &CancellationToken,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(SearchMatch) + Send + Sync,
    {
        let file = fs::File::open(file_path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut line_number = 1;
        let mut byte_offset = 0u32;
        let mut matches_found = 0;
        
        let filename = file_path.to_string_lossy().to_string();
        
        while let Some(line_result) = lines.next_line().await.transpose() {
            // キャンセルチェック
            if cancellation_token.is_cancelled() {
                break;
            }
            
            match line_result {
                Ok(line) => {
                    // リテラル検索（大文字小文字を区別）
                    if line.contains(query) {
                        // クエリが見つかった位置を検索
                        let mut search_start = 0;
                        while let Some(match_pos) = line[search_start..].find(query) {
                            let absolute_pos = search_start + match_pos;
                            matches_found += 1;
                            
                            let search_match = SearchMatch {
                                filename: filename.clone(),
                                line_number,
                                byte_offset: byte_offset + absolute_pos as u32,
                                content: line.clone(),
                            };
                            
                            result_callback(search_match);
                            
                            // 次の検索位置を設定（同じ行内の複数マッチに対応）
                            search_start = absolute_pos + query.len();
                            if search_start >= line.len() {
                                break;
                            }
                        }
                    }
                    
                    // 次の行のバイトオフセットを計算（\n を含む）
                    byte_offset += line.len() as u32 + 1;
                    line_number += 1;
                }
                Err(e) => {
                    log::warn!("Error reading line from {}: {}", filename, e);
                    break;
                }
            }
        }
        
        Ok(matches_found)
    }
}

impl Default for NativeBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchBackend for NativeBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Native
    }
    
    async fn is_available(&self) -> bool {
        // Native実装は常に利用可能
        true
    }
    
    async fn search_literal<F>(
        &self,
        query: &str,
        search_root: &PathBuf,
        cancellation_token: CancellationToken,
        result_callback: F,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(SearchMatch) + Send + Sync,
    {
        log::debug!("Starting native search for query '{}' in {:?}", query, search_root);
        
        let mut total_matches = 0;
        let mut files_processed = 0;
        
        // walkdirを使ってファイルを再帰的に探索
        for entry in WalkDir::new(search_root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            // キャンセルチェック
            if cancellation_token.is_cancelled() {
                log::info!("Native search cancelled, stopping file enumeration");
                break;
            }
            
            let path = entry.path();
            
            // ファイルのみを対象とし、検索対象かどうかをチェック
            if path.is_file() && self.should_search_file(path) {
                match self.search_in_file(path, query, &result_callback, &cancellation_token).await {
                    Ok(matches) => {
                        total_matches += matches;
                        files_processed += 1;
                        
                        // 適度にyieldして他のタスクに譲る
                        if files_processed % 10 == 0 {
                            tokio::task::yield_now().await;
                        }
                    }
                    Err(e) => {
                        log::debug!("Failed to search in file {:?}: {}", path, e);
                        // ファイル読み取りエラーは無視して続行
                    }
                }
            }
        }
        
        log::debug!(
            "Native search completed: {} matches found in {} files",
            total_matches,
            files_processed
        );
        
        Ok(total_matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_files(dir: &TempDir) -> Result<(), std::io::Error> {
        let test_content = [
            ("test1.txt", "Hello world\nThis is a test\nAnother line with Hello"),
            ("test2.rs", "fn main() {\n    println!(\"Hello world\");\n}\n// Hello comment"),
            ("test3.md", "# Test\nHello world example\n## End\nHello again"),
            ("binary.exe", "\x00\x01\x02\x03\x04"), // バイナリファイル（除外される）
        ];

        for (filename, content) in test_content.iter() {
            let file_path = dir.path().join(filename);
            fs::write(&file_path, content).await?;
        }

        // 除外ディレクトリのテスト
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).await?;
        let git_file = git_dir.join("config");
        fs::write(&git_file, "Hello in git directory").await?;

        Ok(())
    }

    #[test]
    fn test_backend_type() {
        let backend = NativeBackend::new();
        assert_eq!(backend.backend_type(), BackendType::Native);
    }

    #[test]
    fn test_should_search_file() {
        let backend = NativeBackend::new();
        
        // 検索対象のファイル
        assert!(backend.should_search_file(std::path::Path::new("test.txt")));
        assert!(backend.should_search_file(std::path::Path::new("src/main.rs")));
        assert!(backend.should_search_file(std::path::Path::new("README.md")));
        
        // 除外されるファイル
        assert!(!backend.should_search_file(std::path::Path::new("test.exe")));
        assert!(!backend.should_search_file(std::path::Path::new("image.png")));
        assert!(!backend.should_search_file(std::path::Path::new("archive.zip")));
        
        // 除外ディレクトリ内のファイル
        assert!(!backend.should_search_file(std::path::Path::new(".git/config")));
        assert!(!backend.should_search_file(std::path::Path::new("node_modules/package/index.js")));
        assert!(!backend.should_search_file(std::path::Path::new("target/debug/main")));
    }

    #[test]
    fn test_custom_config() {
        let backend = NativeBackend::with_config(
            vec!["custom".to_string()],
            vec!["custom_dir".to_string()],
            1024,
        );
        
        assert!(!backend.should_search_file(std::path::Path::new("test.custom")));
        assert!(!backend.should_search_file(std::path::Path::new("custom_dir/file.txt")));
        assert_eq!(backend.max_file_size, 1024);
    }

    #[tokio::test]
    async fn test_availability() {
        let backend = NativeBackend::new();
        assert!(backend.is_available().await);
    }

    #[tokio::test]
    async fn test_search_in_file() {
        let backend = NativeBackend::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        
        // テストファイルを作成
        fs::write(&test_file, "line 1: Hello world\nline 2: Hello again\nline 3: no match").await.unwrap();
        
        let cancellation_token = CancellationToken::new();
        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        
        let matches = backend.search_in_file(
            &test_file,
            "Hello",
            &move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
            &cancellation_token,
        ).await.unwrap();
        
        assert_eq!(matches, 2); // "Hello"が2回見つかる
        let results_vec = results.lock().unwrap();
        assert_eq!(results_vec.len(), 2);
        
        // 結果の検証
        assert_eq!(results_vec[0].line_number, 1);
        assert_eq!(results_vec[0].content, "line 1: Hello world");
        assert_eq!(results_vec[1].line_number, 2);
        assert_eq!(results_vec[1].content, "line 2: Hello again");
    }

    #[tokio::test]
    async fn test_real_search() {
        let backend = NativeBackend::new();
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).await.unwrap();

        let cancellation_token = CancellationToken::new();
        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        
        let result_count = backend.search_literal(
            "Hello",
            &temp_dir.path().to_path_buf(),
            cancellation_token,
            move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
        ).await.unwrap();

        // "Hello"は複数のファイルに複数回存在するはず
        assert!(result_count > 0);
        let results_vec = results.lock().unwrap();
        assert_eq!(results_vec.len() as u32, result_count);
        
        // バイナリファイルは除外されることを確認
        assert!(!results_vec.iter().any(|m| m.filename.contains("binary.exe")));
        
        // .gitディレクトリは除外されることを確認
        assert!(!results_vec.iter().any(|m| m.filename.contains(".git")));
        
        // 想定されるファイルに結果があることを確認
        assert!(results_vec.iter().any(|m| m.filename.contains("test1.txt")));
        assert!(results_vec.iter().any(|m| m.filename.contains("test2.rs")));
        assert!(results_vec.iter().any(|m| m.filename.contains("test3.md")));
    }

    #[tokio::test]
    async fn test_search_cancellation() {
        let backend = NativeBackend::new();
        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).await.unwrap();

        let cancellation_token = CancellationToken::new();
        let token_clone = cancellation_token.clone();
        
        // 検索開始後すぐにキャンセル
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            token_clone.cancel();
        });

        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        let result_count = backend.search_literal(
            "Hello",
            &temp_dir.path().to_path_buf(),
            cancellation_token,
            move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
        ).await.unwrap();

        log::info!("Native search cancelled, found {} results", result_count);
        // キャンセルされたため、結果数は制限される可能性がある
        let results_vec = results.lock().unwrap();
        assert_eq!(results_vec.len() as u32, result_count);
    }

    #[tokio::test]
    async fn test_multiple_matches_per_line() {
        let backend = NativeBackend::new();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_match.txt");
        
        // 同じ行に複数のマッチがあるテストファイル
        fs::write(&test_file, "test test test\nanother line\ntest again test").await.unwrap();
        
        let cancellation_token = CancellationToken::new();
        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        
        let matches = backend.search_in_file(
            &test_file,
            "test",
            &move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
            &cancellation_token,
        ).await.unwrap();
        
        // "test"が5回見つかる（1行目に3回、3行目に2回）
        assert_eq!(matches, 5);
        let results_vec = results.lock().unwrap();
        assert_eq!(results_vec.len(), 5);
        
        // 1行目の結果を確認
        let line1_results: Vec<_> = results_vec.iter().filter(|r| r.line_number == 1).collect();
        assert_eq!(line1_results.len(), 3);
        
        // 3行目の結果を確認
        let line3_results: Vec<_> = results_vec.iter().filter(|r| r.line_number == 3).collect();
        assert_eq!(line3_results.len(), 2);
    }
}