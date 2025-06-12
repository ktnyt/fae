use crate::types::{SearchResult, DisplayInfo};
use crate::index_manager::{IndexManager, FileInfo};
use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::{OnceLock, mpsc};
use std::thread;

/// コンテンツ検索エンジン（grep風）
#[derive(Clone)]
pub struct ContentSearcher {
    /// ファイル発見エンジン
    index_manager: IndexManager,
    /// プロジェクトルート
    project_root: PathBuf,
}

/// ストリーミング検索結果のイテレーター
pub struct ContentSearchStream {
    receiver: mpsc::Receiver<SearchResult>,
    _handle: thread::JoinHandle<()>,
}

impl Iterator for ContentSearchStream {
    type Item = SearchResult;
    
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

/// 大文字小文字を無視するための正規表現キャッシュ
static CASE_INSENSITIVE_REGEX_CACHE: OnceLock<std::sync::Mutex<lru::LruCache<String, Regex>>> = OnceLock::new();

impl ContentSearcher {
    /// 新しいContentSearcherを作成
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let index_manager = IndexManager::new(project_root.clone());
        
        Ok(Self {
            index_manager,
            project_root,
        })
    }

    /// ストリーミングコンテンツ検索を実行
    pub fn search_stream(&self, query: &str) -> Result<ContentSearchStream> {
        // 空のクエリは空のストリームを返す
        if query.trim().is_empty() {
            let (sender, receiver) = mpsc::channel();
            drop(sender); // すぐにチャンネルを閉じる
            let handle = thread::spawn(|| {}); // 空のスレッド
            return Ok(ContentSearchStream { receiver, _handle: handle });
        }

        let query = query.to_string();
        let searcher = self.clone();
        let (sender, receiver) = mpsc::channel();

        let handle = thread::spawn(move || {
            // ファイル一覧を取得
            let files = match searcher.index_manager.discover_files() {
                Ok(files) => files,
                Err(err) => {
                    eprintln!("Warning: Failed to discover files: {}", err);
                    return;
                }
            };

            // ファイルごとに順次検索（行番号順で結果出力のため）
            for file_info in files {
                match searcher.search_in_file(&file_info, &query) {
                    Ok(mut results) => {
                        // ファイル内では行番号順にソート
                        results.sort_by_key(|r| r.line);
                        
                        for result in results {
                            if sender.send(result).is_err() {
                                // receiverが閉じられた場合は終了
                                return;
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Warning: Failed to search in file {}: {}", 
                                  file_info.relative_path.display(), err);
                    }
                }
            }
        });

        Ok(ContentSearchStream { receiver, _handle: handle })
    }

    /// コンテンツ検索を実行
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        // 空のクエリは結果なし
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        // ファイル一覧を取得
        let files = self.index_manager.discover_files()
            .context("Failed to discover files for content search")?;

        // 並列でファイル内容を検索
        let mut results: Vec<SearchResult> = files
            .par_iter()
            .map(|file_info| {
                self.search_in_file(file_info, query).unwrap_or_else(|err| {
                    eprintln!("Warning: Failed to search in file {}: {}", 
                              file_info.relative_path.display(), err);
                    Vec::new()
                })
            })
            .flatten()
            .collect();

        // スコア順でソート（高い順）
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 制限数まで切り詰め
        results.truncate(limit);

        Ok(results)
    }

    /// プロジェクトルートを取得
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// 単一ファイル内でのコンテンツ検索
    fn search_in_file(&self, file_info: &FileInfo, query: &str) -> Result<Vec<SearchResult>> {
        // ファイル内容を読み込み
        let content = fs::read_to_string(&file_info.path)
            .with_context(|| format!("Failed to read file: {}", file_info.path.display()))?;

        let mut results = Vec::new();
        let regex = self.create_case_insensitive_regex(query)?;

        // 行ごとに検索
        for (line_number, line) in content.lines().enumerate() {
            if let Some(captures) = regex.find(line) {
                let match_start = captures.start();
                let match_end = captures.end();
                
                // スコア計算（完全一致 > 部分一致 > 大文字小文字違い）
                let score = self.calculate_score(query, line, match_start, match_end);

                let result = SearchResult {
                    file_path: file_info.path.clone(),
                    line: (line_number + 1) as u32,  // 1ベース
                    column: (match_start + 1) as u32, // 1ベース
                    display_info: DisplayInfo::Content {
                        line_content: line.to_string(),
                        match_start,
                        match_end,
                    },
                    score,
                };
                
                results.push(result);
            }
        }

        Ok(results)
    }

    /// 大文字小文字を無視する正規表現を作成・キャッシュ
    fn create_case_insensitive_regex(&self, query: &str) -> Result<Regex> {
        let cache = CASE_INSENSITIVE_REGEX_CACHE.get_or_init(|| {
            std::sync::Mutex::new(lru::LruCache::new(std::num::NonZeroUsize::new(100).unwrap()))
        });

        let mut cache_guard = cache.lock().unwrap();
        
        if let Some(regex) = cache_guard.get(query) {
            return Ok(regex.clone());
        }

        // 正規表現メタ文字をエスケープ
        let escaped_query = regex::escape(query);
        
        // 大文字小文字を無視する正規表現を作成
        let regex_pattern = format!("(?i){}", escaped_query);
        let regex = Regex::new(&regex_pattern)
            .with_context(|| format!("Failed to create regex for query: {}", query))?;

        cache_guard.put(query.to_string(), regex.clone());
        Ok(regex)
    }

    /// 検索スコアを計算
    fn calculate_score(&self, query: &str, line: &str, match_start: usize, match_end: usize) -> f64 {
        let matched_text = &line[match_start..match_end];
        
        // ベーススコア
        let mut score = 1.0;

        // 完全一致ボーナス
        if matched_text.eq_ignore_ascii_case(query) {
            score += 2.0;
        }

        // 大文字小文字完全一致ボーナス
        if matched_text == query {
            score += 1.0;
        }

        // 単語境界ボーナス（単語の開始位置）
        if match_start == 0 || !line.chars().nth(match_start - 1).unwrap_or(' ').is_alphanumeric() {
            score += 0.5;
        }

        // 短い行ほど高スコア（関連性が高い可能性）
        let line_length_factor = 100.0 / (line.len() as f64 + 1.0);
        score += line_length_factor * 0.1;

        // クエリ長に対するマッチ長の比率
        let match_ratio = (match_end - match_start) as f64 / query.len() as f64;
        score += match_ratio * 0.5;

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    fn create_simple_test_file(temp_dir: &TempDir, filename: &str, content: &str) -> Result<PathBuf> {
        let file_path = temp_dir.path().join(filename);
        let mut file = File::create(&file_path)?;
        write!(file, "{}", content)?;
        Ok(file_path)
    }

    #[test]
    fn test_content_searcher_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        assert_eq!(searcher.project_root(), temp_dir.path());
        Ok(())
    }

    #[test]
    fn test_simple_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_simple_test_file(&temp_dir, "test.ts", "function hello() { console.log('world'); }")?;
        
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        let results = searcher.search("hello", 10)?;
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line, 1);
        
        Ok(())
    }

    #[test]
    fn test_case_insensitive_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_simple_test_file(&temp_dir, "test.ts", "function HelloWorld() { return 'test'; }")?;
        
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        
        let results_lower = searcher.search("helloworld", 10)?;
        let results_upper = searcher.search("HELLOWORLD", 10)?;
        
        assert_eq!(results_lower.len(), 1);
        assert_eq!(results_upper.len(), 1);
        
        Ok(())
    }

    #[test]
    fn test_score_calculation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_simple_test_file(&temp_dir, "test.ts", "hello world\nthis has hello in the middle\nhello")?;
        
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        let results = searcher.search("hello", 10)?;
        
        assert_eq!(results.len(), 3);
        
        // スコア順でソートされていることを確認
        for i in 1..results.len() {
            assert!(results[i-1].score >= results[i].score);
        }
        
        Ok(())
    }

    #[test]
    fn test_streaming_search() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_simple_test_file(&temp_dir, "test1.ts", "function hello() { console.log('world'); }")?;
        create_simple_test_file(&temp_dir, "test2.rs", "fn hello() { println!('world'); }")?;
        
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        let stream = searcher.search_stream("hello")?;
        
        let results: Vec<_> = stream.collect();
        assert_eq!(results.len(), 2);
        
        // ファイル名順で結果を確認
        let mut file_names: Vec<_> = results.iter()
            .map(|r| r.file_path.file_name().unwrap().to_str().unwrap())
            .collect();
        file_names.sort();
        
        assert_eq!(file_names, vec!["test1.ts", "test2.rs"]);
        
        Ok(())
    }

    #[test]
    fn test_streaming_search_empty_query() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        let stream = searcher.search_stream("")?;
        
        let results: Vec<_> = stream.collect();
        assert_eq!(results.len(), 0);
        
        Ok(())
    }

    #[test]
    fn test_streaming_search_limit() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_simple_test_file(&temp_dir, "test1.ts", "hello\nhello\nhello")?;
        create_simple_test_file(&temp_dir, "test2.rs", "hello\nhello")?;
        
        let searcher = ContentSearcher::new(temp_dir.path().to_path_buf())?;
        let stream = searcher.search_stream("hello")?;
        
        // 最初の3件だけ取得
        let results: Vec<_> = stream.take(3).collect();
        assert_eq!(results.len(), 3);
        
        Ok(())
    }
}