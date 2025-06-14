//! ripgrep バックエンド実装
//! 
//! ripgrepは最も高速なテキスト検索ツールの一つで、最優先の選択肢です。

use super::{BackendType, SearchBackend, SearchMatch};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

/// ripgrepバックエンド
#[derive(Debug)]
pub struct RipgrepBackend {
    /// ripgrepバイナリのパス（Noneの場合はPATHから"rg"を検索）
    binary_path: Option<PathBuf>,
}

impl RipgrepBackend {
    /// 新しいripgrepバックエンドを作成
    pub fn new() -> Self {
        Self {
            binary_path: None,
        }
    }
    
    /// カスタムripgrepバイナリパスを指定
    pub fn with_binary_path(binary_path: PathBuf) -> Self {
        Self {
            binary_path: Some(binary_path),
        }
    }
    
    /// ripgrepコマンド名を取得
    fn get_command_name(&self) -> String {
        self.binary_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "rg".to_string())
    }
    
    /// ripgrepの出力行をパース
    /// フォーマット: filename:line_number:byte_offset:content
    fn parse_ripgrep_line(&self, line: &str) -> Option<SearchMatch> {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() != 4 {
            log::warn!("Invalid ripgrep output format: {}", line);
            return None;
        }

        let filename = parts[0].to_string();
        let line_number = parts[1].parse::<u32>().ok()?;
        let byte_offset = parts[2].parse::<u32>().ok()?;
        let content = parts[3].to_string();

        Some(SearchMatch {
            filename,
            line_number,
            byte_offset,
            content,
        })
    }
}

#[async_trait]
impl SearchBackend for RipgrepBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Ripgrep
    }
    
    async fn is_available(&self) -> bool {
        let cmd_name = self.get_command_name();
        
        match Command::new(&cmd_name)
            .arg("--version")
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    log::debug!("ripgrep version: {}", version.trim());
                    true
                } else {
                    log::debug!("ripgrep command '{}' failed with status: {}", cmd_name, output.status);
                    false
                }
            }
            Err(e) => {
                log::debug!("ripgrep command '{}' not found: {}", cmd_name, e);
                false
            }
        }
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
        let cmd_name = self.get_command_name();
        let mut cmd = Command::new(&cmd_name);
        
        cmd.arg("--line-number")
            .arg("--byte-offset")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("--fixed-strings") // リテラル検索（正規表現無効化）
            .arg("--")
            .arg(query)
            .arg(search_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        log::debug!("Executing ripgrep: {:?}", cmd);

        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture ripgrep stdout")?;

        // リアルタイムで出力を行ごとに処理
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        // stderrもバックグラウンドで読み取り
        let stderr = child.stderr.take();
        let stderr_task = if let Some(stderr) = stderr {
            Some(tokio::spawn(async move {
                let mut stderr_reader = BufReader::new(stderr);
                let mut content = String::new();
                use tokio::io::AsyncReadExt;
                let _ = stderr_reader.read_to_string(&mut content).await;
                content
            }))
        } else {
            None
        };

        // 結果のストリーミング処理
        let mut lines_processed = 0;
        let mut was_cancelled = false;
        
        while let Some(line_result) = lines.next_line().await.transpose() {
            // キャンセルチェック
            if cancellation_token.is_cancelled() {
                log::info!("ripgrep search cancelled, stopping result processing");
                was_cancelled = true;
                break;
            }
            
            match line_result {
                Ok(line) => {
                    if let Some(search_match) = self.parse_ripgrep_line(&line) {
                        lines_processed += 1;
                        result_callback(search_match);
                        
                        // 大量の結果の場合は適度にyieldして他のタスクに譲る
                        if lines_processed % 100 == 0 {
                            tokio::task::yield_now().await;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Error reading ripgrep line: {}", e);
                    break;
                }
            }
        }

        // 中断された場合はripgrepプロセスを終了
        if was_cancelled {
            log::info!("Terminating ripgrep process due to cancellation");
            let _ = child.kill().await;
        }

        // プロセス終了をバックグラウンドで待機（ブロックしない）
        let query_copy = query.to_string();
        let wait_task = tokio::spawn(async move {
            let status = child.wait().await;
            match status {
                Ok(status) if status.success() => {
                    log::debug!("ripgrep process completed successfully for query: '{}'", query_copy);
                }
                Ok(status) => {
                    log::warn!("ripgrep exited with non-zero status: {} for query: '{}'", status, query_copy);
                }
                Err(e) => {
                    log::error!("Failed to wait for ripgrep process: {} for query: '{}'", e, query_copy);
                }
            }
            
            // stderrの内容をログ出力
            if let Some(stderr_task) = stderr_task {
                if let Ok(stderr_content) = stderr_task.await {
                    if !stderr_content.trim().is_empty() {
                        log::debug!("ripgrep stderr for query '{}': {}", query_copy, stderr_content.trim());
                    }
                }
            }
        });

        // バックグラウンドタスクを実行
        tokio::spawn(wait_task);
        
        Ok(lines_processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_files(dir: &TempDir) -> Result<(), std::io::Error> {
        let test_content = [
            ("test1.txt", "Hello world\nThis is a test\nAnother line"),
            ("test2.rs", "fn main() {\n    println!(\"Hello world\");\n}"),
            ("test3.md", "# Test\nHello world example\n## End"),
        ];

        for (filename, content) in test_content.iter() {
            let file_path = dir.path().join(filename);
            fs::write(&file_path, content).await?;
        }

        Ok(())
    }

    #[test]
    fn test_backend_type() {
        let backend = RipgrepBackend::new();
        assert_eq!(backend.backend_type(), BackendType::Ripgrep);
    }

    #[test]
    fn test_parse_ripgrep_line() {
        let backend = RipgrepBackend::new();

        // 正常なケース
        let line = "src/main.rs:10:245:    println!(\"Hello world\");";
        let result = backend.parse_ripgrep_line(line);
        assert!(result.is_some());

        let search_match = result.unwrap();
        assert_eq!(search_match.filename, "src/main.rs");
        assert_eq!(search_match.line_number, 10);
        assert_eq!(search_match.byte_offset, 245);
        assert_eq!(search_match.content, "    println!(\"Hello world\");");

        // 不正なフォーマット
        let invalid_line = "invalid:format";
        assert!(backend.parse_ripgrep_line(invalid_line).is_none());
    }

    #[test]
    fn test_with_binary_path() {
        let custom_path = PathBuf::from("/usr/local/bin/rg");
        let backend = RipgrepBackend::with_binary_path(custom_path.clone());
        assert_eq!(backend.binary_path, Some(custom_path));
        assert_eq!(backend.get_command_name(), "/usr/local/bin/rg");
    }

    #[test]
    fn test_default_command_name() {
        let backend = RipgrepBackend::new();
        assert_eq!(backend.get_command_name(), "rg");
    }

    #[tokio::test]
    async fn test_availability_check() {
        let backend = RipgrepBackend::new();
        
        // is_availableは実際のripgrepの存在に依存するため、
        // テスト環境では結果が変わる可能性がある
        let available = backend.is_available().await;
        log::info!("ripgrep available: {}", available);
        
        // 存在しないパスでのテスト
        let backend_invalid = RipgrepBackend::with_binary_path(PathBuf::from("/nonexistent/rg"));
        assert!(!backend_invalid.is_available().await);
    }

    #[tokio::test]
    #[ignore] // ripgrepが利用可能な環境でのみ実行
    async fn test_real_search() {
        // ripgrepが利用可能かチェック
        let backend = RipgrepBackend::new();
        if !backend.is_available().await {
            return; // ripgrepが見つからない場合はテストをスキップ
        }

        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).await.unwrap();

        let cancellation_token = CancellationToken::new();
        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        
        let result_count = backend.search_literal(
            "Hello world",
            &temp_dir.path().to_path_buf(),
            cancellation_token,
            move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
        ).await.unwrap();

        // "Hello world"は複数のファイルに存在するはず
        assert!(result_count > 0);
        let results_vec = results.lock().unwrap();
        assert_eq!(results_vec.len() as u32, result_count);
        
        // 結果の検証
        assert!(results_vec.iter().any(|m| m.filename.contains("test1.txt")));
        assert!(results_vec.iter().any(|m| m.filename.contains("test2.rs")));
        assert!(results_vec.iter().any(|m| m.filename.contains("test3.md")));
    }

    #[tokio::test]
    #[ignore] // ripgrepが利用可能な環境でのみ実行
    async fn test_search_cancellation() {
        let backend = RipgrepBackend::new();
        if !backend.is_available().await {
            return;
        }

        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).await.unwrap();

        let cancellation_token = CancellationToken::new();
        let token_clone = cancellation_token.clone();
        
        // 検索開始後すぐにキャンセル
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            token_clone.cancel();
        });

        let results = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let results_clone = results.clone();
        let result_count = backend.search_literal(
            "test", // 一般的な検索語
            &temp_dir.path().to_path_buf(),
            cancellation_token,
            move |search_match| {
                results_clone.lock().unwrap().push(search_match);
            },
        ).await.unwrap();

        log::info!("Search cancelled, found {} results", result_count);
        // キャンセルされたため、結果数は制限される可能性がある
    }
}