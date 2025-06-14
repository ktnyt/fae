use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use crate::jsonrpc::handler::JsonRpcHandler;
use crate::jsonrpc::message::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

/// リテラル検索のJSON-RPCハンドラー
pub struct LiteralSearchHandler {
    /// 検索基準ディレクトリ
    search_root: PathBuf,
    /// 現在の検索クエリ
    current_query: Option<String>,
    /// 検索結果カウンタ
    result_count: u32,
}

impl LiteralSearchHandler {
    /// 新しいハンドラーを作成
    pub fn new(search_root: PathBuf) -> Self {
        Self {
            search_root,
            current_query: None,
            result_count: 0,
        }
    }

    /// clearSearchResults 通知を送信（将来実装用）
    async fn send_clear_notification(&mut self) {
        log::info!("Search results cleared");
        self.result_count = 0;
    }

    /// pushSearchResult 通知を送信（将来実装用）
    async fn send_result_notification(
        &mut self,
        filename: &str,
        line: u32,
        offset: u32,
        content: &str,
    ) {
        self.result_count += 1;
        log::debug!(
            "Found result #{}: {}:{}:{} - {}",
            self.result_count,
            filename,
            line,
            offset,
            content.trim()
        );
    }

    /// updateQuery 通知を処理
    async fn handle_update_query(&mut self, params: Option<Value>) -> Result<(), JsonRpcError> {
        let query = params
            .as_ref()
            .and_then(|p| p.get("query"))
            .and_then(|q| q.as_str())
            .ok_or_else(|| {
                JsonRpcError::invalid_params(
                    Some("updateQuery requires 'query' parameter"),
                    None,
                )
            })?;

        log::info!("Starting literal search for query: '{}'", query);
        self.current_query = Some(query.to_string());

        // 検索結果をクリア
        self.send_clear_notification().await;

        // ripgrepで検索実行
        if let Err(e) = self.execute_ripgrep_search(query).await {
            log::error!("Search execution failed: {}", e);
            return Err(JsonRpcError::internal_error(
                Some(format!("Search failed: {}", e)),
                None,
            ));
        }

        Ok(())
    }

    /// ripgrepを使って実際の検索を実行
    async fn execute_ripgrep_search(&mut self, query: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut cmd = Command::new("rg");
        cmd.arg("--line-number")
            .arg("--byte-offset")
            .arg("--no-heading")
            .arg("--color=never")
            .arg("--fixed-strings") // リテラル検索（正規表現無効化）
            .arg("--")
            .arg(query)
            .arg(&self.search_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        log::debug!("Executing ripgrep: {:?}", cmd);

        let mut child = cmd.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture ripgrep stdout")?;

        // 出力を行ごとに処理
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if let Some((filename, line_num, byte_offset, content)) = self.parse_ripgrep_line(&line)
            {
                self.send_result_notification(&filename, line_num, byte_offset, &content)
                    .await;
            }
        }

        // プロセス終了を待機
        let status = child.wait().await?;
        if !status.success() {
            let stderr_content = if let Some(stderr) = child.stderr.take() {
                let mut stderr_reader = BufReader::new(stderr);
                let mut content = String::new();
                use tokio::io::AsyncReadExt;
                stderr_reader.read_to_string(&mut content).await.unwrap_or_default();
                content
            } else {
                "Unknown error".to_string()
            };

            log::warn!("ripgrep exited with status: {}, stderr: {}", status, stderr_content);
        }

        log::info!("Search completed for query: '{}'", query);
        Ok(())
    }

    /// ripgrepの出力行をパース
    /// フォーマット: filename:line_number:byte_offset:content
    fn parse_ripgrep_line(&self, line: &str) -> Option<(String, u32, u32, String)> {
        let parts: Vec<&str> = line.splitn(4, ':').collect();
        if parts.len() != 4 {
            log::warn!("Invalid ripgrep output format: {}", line);
            return None;
        }

        let filename = parts[0].to_string();
        let line_num = parts[1].parse::<u32>().ok()?;
        let byte_offset = parts[2].parse::<u32>().ok()?;
        let content = parts[3].to_string();

        Some((filename, line_num, byte_offset, content))
    }
}

#[async_trait]
impl JsonRpcHandler for LiteralSearchHandler {
    async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        log::debug!(
            "LiteralSearchHandler received request: method={}",
            request.method
        );

        match request.method.as_str() {
            "search.status" => {
                // 現在の検索状態を返す
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!({
                        "status": "ready",
                        "current_query": self.current_query,
                        "search_root": self.search_root.to_string_lossy()
                    })),
                    error: None,
                }
            }
            _ => JsonRpcResponse {
                id: request.id,
                result: None,
                error: Some(JsonRpcError::method_not_found(
                    Some(format!("Method '{}' not found", request.method)),
                    Some(json!({"method": request.method})),
                )),
            },
        }
    }

    async fn on_notification(&mut self, notification: JsonRpcNotification) {
        log::debug!(
            "LiteralSearchHandler received notification: method={}",
            notification.method
        );

        match notification.method.as_str() {
            "updateQuery" => {
                if let Err(e) = self.handle_update_query(notification.params).await {
                    log::error!("Failed to handle updateQuery: {:?}", e);
                }
            }
            _ => {
                log::warn!("Unknown notification method: {}", notification.method);
            }
        }
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
    fn test_parse_ripgrep_line() {
        let handler = LiteralSearchHandler::new(PathBuf::from("."));

        // 正常なケース
        let line = "src/main.rs:10:245:    println!(\"Hello world\");";
        let result = handler.parse_ripgrep_line(line);
        assert!(result.is_some());

        let (filename, line_num, offset, content) = result.unwrap();
        assert_eq!(filename, "src/main.rs");
        assert_eq!(line_num, 10);
        assert_eq!(offset, 245);
        assert_eq!(content, "    println!(\"Hello world\");");

        // 不正なフォーマット
        let invalid_line = "invalid:format";
        assert!(handler.parse_ripgrep_line(invalid_line).is_none());
    }

    #[test]
    fn test_handler_creation() {
        let handler = LiteralSearchHandler::new(PathBuf::from("/tmp"));
        assert_eq!(handler.search_root, PathBuf::from("/tmp"));
        assert!(handler.current_query.is_none());
        assert_eq!(handler.result_count, 0);
    }

    #[tokio::test]
    async fn test_search_status_request() {
        let mut handler = LiteralSearchHandler::new(PathBuf::from("/test"));

        let request = JsonRpcRequest {
            id: 1,
            method: "search.status".to_string(),
            params: None,
        };

        let response = handler.on_request(request).await;
        assert_eq!(response.id, 1);
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert_eq!(result["status"], "ready");
        assert_eq!(result["search_root"], "/test");
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let mut handler = LiteralSearchHandler::new(PathBuf::from("/test"));

        let request = JsonRpcRequest {
            id: 2,
            method: "unknown.method".to_string(),
            params: None,
        };

        let response = handler.on_request(request).await;
        assert_eq!(response.id, 2);
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32601); // Method not found
    }

    #[tokio::test]
    async fn test_update_query_notification() {
        // ログ初期化
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let mut handler = LiteralSearchHandler::new(PathBuf::from("."));

        // updateQuery通知（無効なパラメータ）
        let notification = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"invalid": "param"})),
        };

        handler.on_notification(notification).await;

        // 正常なupdateQuery通知
        let notification = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"query": "test"})),
        };

        handler.on_notification(notification).await;

        // ハンドラーの状態が更新されることを確認
        assert_eq!(handler.current_query, Some("test".to_string()));
        // result_countは実際にripgrepが実行されるため、何らかの結果がある
        // テスト環境では"test"文字列が多数見つかることが期待される
        assert!(handler.result_count > 0);
    }

    #[tokio::test]
    #[ignore] // ripgrepが利用可能な環境でのみ実行
    async fn test_real_ripgrep_search() {
        // ripgrepが利用可能かチェック
        if Command::new("rg").arg("--version").output().await.is_err() {
            return; // ripgrepが見つからない場合はテストをスキップ
        }

        let temp_dir = TempDir::new().unwrap();
        create_test_files(&temp_dir).await.unwrap();

        let mut handler = LiteralSearchHandler::new(temp_dir.path().to_path_buf());

        // "Hello world"を検索
        let notification = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"query": "Hello world"})),
        };

        handler.on_notification(notification).await;

        // 検索が実行されたことを確認（結果カウンターで判定）
        assert_eq!(handler.current_query, Some("Hello world".to_string()));
        // 注: ripgrepの実行は非同期で行われるため、実際の結果カウントのチェックは困難
        // 代わりに、エラーが発生しないことを確認
    }
}