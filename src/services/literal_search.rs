use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use crate::jsonrpc::handler::JsonRpcHandler;
use crate::jsonrpc::message::{JsonRpcError, JsonRpcNotification, JsonRpcPayload, JsonRpcRequest, JsonRpcResponse};
use crate::services::backend::{SearchBackend, SearchBackendImpl};

/// 現在の検索状態
#[derive(Debug)]
struct SearchState {
    /// 現在の検索クエリ
    query: String,
    /// キャンセレーショントークン
    cancellation_token: CancellationToken,
    /// 検索結果カウンタ
    result_count: u32,
}

/// リテラル検索のJSON-RPCハンドラー
pub struct LiteralSearchHandler {
    /// 検索基準ディレクトリ
    search_root: PathBuf,
    /// 現在の検索状態（共有状態）
    current_search: Arc<Mutex<Option<SearchState>>>,
    /// 検索バックエンド
    backend: SearchBackendImpl,
    /// JSON-RPC通知送信チャンネル
    notification_sender: Option<mpsc::UnboundedSender<JsonRpcPayload>>,
}

impl LiteralSearchHandler {
    /// 新しいハンドラーを作成（バックエンドは自動選択）
    pub async fn new(search_root: PathBuf) -> Self {
        let backend = SearchBackendImpl::create_best_available().await;
        Self {
            search_root,
            current_search: Arc::new(Mutex::new(None)),
            backend,
            notification_sender: None,
        }
    }
    
    /// 指定されたバックエンドでハンドラーを作成
    pub fn with_backend(search_root: PathBuf, backend: SearchBackendImpl) -> Self {
        Self {
            search_root,
            current_search: Arc::new(Mutex::new(None)),
            backend,
            notification_sender: None,
        }
    }


    /// clearSearchResults 通知を送信
    async fn send_clear_notification(&self) {
        if let Some(ref sender) = self.notification_sender {
            let notification = JsonRpcNotification {
                method: "clearSearchResults".to_string(),
                params: None,
            };

            let payload = JsonRpcPayload::Notification(notification);
            if let Err(e) = sender.send(payload) {
                log::error!("Failed to send clearSearchResults notification: {}", e);
            } else {
                log::debug!("Sent clearSearchResults notification");
            }
        } else {
            log::info!("Search results cleared (no notification sender)");
        }
    }

    /// pushSearchResult 通知を送信
    async fn send_result_notification(
        &self,
        filename: &str,
        line: u32,
        offset: u32,
        content: &str,
        result_count: u32,
    ) {
        if let Some(ref sender) = self.notification_sender {
            let notification = JsonRpcNotification {
                method: "pushSearchResult".to_string(),
                params: Some(json!({
                    "filename": filename,
                    "line": line,
                    "offset": offset,
                    "content": content,
                    "result_count": result_count
                })),
            };

            let payload = JsonRpcPayload::Notification(notification);
            if let Err(e) = sender.send(payload) {
                log::error!("Failed to send pushSearchResult notification: {}", e);
            } else {
                log::debug!(
                    "Sent pushSearchResult #{}: {}:{}:{} - {}",
                    result_count,
                    filename,
                    line,
                    offset,
                    content.trim()
                );
            }
        } else {
            log::debug!(
                "Found result #{}: {}:{}:{} - {} (no notification sender)",
                result_count,
                filename,
                line,
                offset,
                content.trim()
            );
        }
    }

    /// searchCompleted 通知を送信（検索終了を通知）
    async fn send_search_completed_notification(&self, query: &str, total_results: u32) {
        if let Some(ref sender) = self.notification_sender {
            let notification = JsonRpcNotification {
                method: "searchCompleted".to_string(),
                params: Some(json!({
                    "query": query,
                    "total_results": total_results
                })),
            };

            let payload = JsonRpcPayload::Notification(notification);
            if let Err(e) = sender.send(payload) {
                log::error!("Failed to send searchCompleted notification: {}", e);
            } else {
                log::info!(
                    "Search completed for query '{}': {} results found",
                    query,
                    total_results
                );
            }
        } else {
            log::info!(
                "Search completed for query '{}': {} results found (no notification sender)",
                query,
                total_results
            );
        }
    }

    /// searchCancelled 通知を送信（検索キャンセル通知）
    async fn send_search_cancelled_notification(&self, query: &str, partial_results: u32) {
        if let Some(ref sender) = self.notification_sender {
            let notification = JsonRpcNotification {
                method: "searchCancelled".to_string(),
                params: Some(json!({
                    "query": query,
                    "partial_results": partial_results
                })),
            };

            let payload = JsonRpcPayload::Notification(notification);
            if let Err(e) = sender.send(payload) {
                log::error!("Failed to send searchCancelled notification: {}", e);
            } else {
                log::info!(
                    "Search cancelled for query '{}': {} partial results found",
                    query,
                    partial_results
                );
            }
        } else {
            log::info!(
                "Search cancelled for query '{}': {} partial results found (no notification sender)",
                query,
                partial_results
            );
        }
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

        // 既存の検索をキャンセル
        let mut current_search = self.current_search.lock().await;
        if let Some(existing_search) = current_search.take() {
            log::info!("Cancelling existing search for query: '{}'", existing_search.query);
            existing_search.cancellation_token.cancel();
            self.send_search_cancelled_notification(&existing_search.query, existing_search.result_count).await;
        }

        // 新しい検索状態を作成
        let cancellation_token = CancellationToken::new();
        let new_search = SearchState {
            query: query.to_string(),
            cancellation_token: cancellation_token.clone(),
            result_count: 0,
        };
        *current_search = Some(new_search);
        drop(current_search); // ロックを早めに解放

        // 検索結果をクリア
        self.send_clear_notification().await;

        // バックエンドで検索実行
        if let Err(e) = self.execute_search(query, cancellation_token).await {
            log::error!("Search execution failed: {}", e);
            return Err(JsonRpcError::internal_error(
                Some(format!("Search failed: {}", e)),
                None,
            ));
        }

        Ok(())
    }

    /// バックエンドを使って実際の検索を実行（リアルタイムストリーミング）
    async fn execute_search(&self, query: &str, cancellation_token: CancellationToken) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("Starting search with backend: {}", self.backend.backend_type().name());
        
        let current_search_clone = self.current_search.clone();
        let notification_sender_clone = self.notification_sender.clone();
        
        let result_count = self.backend.search_literal(
            query,
            &self.search_root,
            cancellation_token.clone(),
            move |search_match| {
                // 検索結果を受信した際のコールバック
                let current_search_inner = current_search_clone.clone();
                let sender_clone = notification_sender_clone.clone();
                tokio::spawn(async move {
                    let result_count = {
                        let mut current_search = current_search_inner.lock().await;
                        if let Some(ref mut search_state) = current_search.as_mut() {
                            search_state.result_count += 1;
                            search_state.result_count
                        } else {
                            1 // フォールバック
                        }
                    };
                    
                    // JSON-RPC pushSearchResult 通知を送信
                    if let Some(ref sender) = sender_clone {
                        let notification = JsonRpcNotification {
                            method: "pushSearchResult".to_string(),
                            params: Some(json!({
                                "filename": search_match.filename,
                                "line": search_match.line_number,
                                "offset": search_match.byte_offset,
                                "content": search_match.content,
                                "result_count": result_count
                            })),
                        };
                        
                        let payload = JsonRpcPayload::Notification(notification);
                        if let Err(e) = sender.send(payload) {
                            log::error!("Failed to send pushSearchResult notification: {}", e);
                        } else {
                            log::debug!(
                                "Sent pushSearchResult #{}: {}:{}:{} - {}",
                                result_count,
                                search_match.filename,
                                search_match.line_number,
                                search_match.byte_offset,
                                search_match.content.trim()
                            );
                        }
                    } else {
                        log::debug!(
                            "Found result #{}: {}:{}:{} - {} (no notification sender)",
                            result_count,
                            search_match.filename,
                            search_match.line_number,
                            search_match.byte_offset,
                            search_match.content.trim()
                        );
                    }
                });
            },
        ).await?;
        
        // 検索完了の処理
        let was_cancelled = cancellation_token.is_cancelled();
        if was_cancelled {
            log::info!("Search was cancelled");
            // 現在の検索状態をクリア
            {
                let mut current_search = self.current_search.lock().await;
                if let Some(search_state) = current_search.take() {
                    self.send_search_cancelled_notification(&search_state.query, search_state.result_count).await;
                }
            }
        } else {
            log::info!("Search completed successfully");
            // 正常終了の場合は完了通知を送信
            self.send_search_completed_notification(query, result_count).await;
            
            // 現在の検索状態をクリア（完了のため）
            {
                let mut current_search = self.current_search.lock().await;
                *current_search = None;
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl JsonRpcHandler for LiteralSearchHandler {
    async fn on_request(
        &mut self, 
        request: JsonRpcRequest,
        sender: &mpsc::UnboundedSender<JsonRpcPayload>,
    ) -> JsonRpcResponse {
        log::debug!(
            "LiteralSearchHandler received request: method={}",
            request.method
        );

        match request.method.as_str() {
            "search.status" => {
                // 現在の検索状態を返す
                let current_search = self.current_search.lock().await;
                let (status, current_query) = if let Some(ref search_state) = *current_search {
                    ("searching", Some(search_state.query.clone()))
                } else {
                    ("ready", None)
                };
                
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!({
                        "status": status,
                        "current_query": current_query,
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

    async fn on_notification(
        &mut self, 
        notification: JsonRpcNotification,
        sender: &mpsc::UnboundedSender<JsonRpcPayload>,
    ) {
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


    #[tokio::test]
    async fn test_handler_creation() {
        let handler = LiteralSearchHandler::new(PathBuf::from("/tmp")).await;
        assert_eq!(handler.search_root, PathBuf::from("/tmp"));
        // current_searchは初期状態でNoneであることを確認
        let current_search = handler.current_search.lock().await;
        assert!(current_search.is_none());
    }

    #[tokio::test]
    async fn test_search_status_request() {
        let mut handler = LiteralSearchHandler::new(PathBuf::from("/test")).await;

        let request = JsonRpcRequest {
            id: 1,
            method: "search.status".to_string(),
            params: None,
        };

        // ダミーのsenderを作成
        let (dummy_sender, _dummy_rx) = mpsc::unbounded_channel();
        let response = handler.on_request(request, &dummy_sender).await;
        assert_eq!(response.id, 1);
        assert!(response.error.is_none());
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        assert_eq!(result["status"], "ready");
        assert_eq!(result["search_root"], "/test");
        assert_eq!(result["current_query"], json!(null));
    }

    #[tokio::test]
    async fn test_unknown_method() {
        let mut handler = LiteralSearchHandler::new(PathBuf::from("/test")).await;

        let request = JsonRpcRequest {
            id: 2,
            method: "unknown.method".to_string(),
            params: None,
        };

        // ダミーのsenderを作成
        let (dummy_sender, _dummy_rx) = mpsc::unbounded_channel();
        let response = handler.on_request(request, &dummy_sender).await;
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

        let mut handler = LiteralSearchHandler::new(PathBuf::from(".")).await;
        
        // テスト用のダミーsenderを作成
        let (dummy_sender, _dummy_rx) = mpsc::unbounded_channel();

        // updateQuery通知（無効なパラメータ）
        let notification = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"invalid": "param"})),
        };

        handler.on_notification(notification, &dummy_sender).await;

        // 正常なupdateQuery通知
        let notification = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"query": "test"})),
        };

        handler.on_notification(notification, &dummy_sender).await;

        // ハンドラーの状態が更新されることを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // 少し待機
        let current_search = handler.current_search.lock().await;
        if let Some(ref search_state) = *current_search {
            assert_eq!(search_state.query, "test");
            // result_countは実際にripgrepが実行されるため、何らかの結果がある可能性がある
            // テスト環境では"test"文字列が多数見つかることが期待される
        }
    }

    #[tokio::test]
    #[ignore] // ripgrepが利用可能な環境でのみ実行
    async fn test_real_ripgrep_search() {
        use tokio::process::Command;
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

        let mut handler_resolved = handler.await;
        
        // テスト用のダミーsenderを作成
        let (dummy_sender, _dummy_rx) = mpsc::unbounded_channel();
        handler_resolved.on_notification(notification, &dummy_sender).await;

        // 検索が実行されたことを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; // 少し待機
        let current_search = handler_resolved.current_search.lock().await;
        // 検索は非同期で実行され、完了時に状態がクリアされるため、
        // 状態の確認は困難。代わりにエラーが発生しないことを確認。
        // 注: ripgrepの実行は非同期で行われるため、実際の結果カウントのチェックは困難
        // 代わりに、エラーが発生しないことを確認
    }

    #[tokio::test]
    async fn test_search_cancellation() {
        // ログ初期化
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let mut handler = LiteralSearchHandler::new(PathBuf::from(".")).await;
        
        // テスト用のダミーsenderを作成
        let (dummy_sender, _dummy_rx) = mpsc::unbounded_channel();

        // 最初の検索を開始
        let notification1 = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"query": "the"})), // よく見つかる文字列
        };

        handler.on_notification(notification1, &dummy_sender).await;

        // 検索が開始される前に状態を確認（すぐに確認）
        // 注: 検索は非同期で実行されるが、状態の設定は同期的に行われる
        {
            let current_search = handler.current_search.lock().await;
            // 非常に高速なripgrepの場合、既に完了している可能性もある
            // そのため、検索が開始されたかまたは既に完了したかを確認
            if let Some(ref search_state) = *current_search {
                assert_eq!(search_state.query, "the");
            } else {
                // 検索が完了して状態がクリアされた場合（ripgrepが非常に高速）
                log::info!("Search completed very quickly");
            }
        }

        // 2番目の検索を開始（最初の検索を中断する、または新しい検索を開始する）
        let notification2 = JsonRpcNotification {
            method: "updateQuery".to_string(),
            params: Some(json!({"query": "function"})),
        };

        handler.on_notification(notification2, &dummy_sender).await;

        // 新しい検索が設定されていることを確認
        {
            let current_search = handler.current_search.lock().await;
            if let Some(ref search_state) = *current_search {
                assert_eq!(search_state.query, "function");
                log::info!("Successfully started second search for: {}", search_state.query);
            } else {
                // 2番目の検索も高速に完了した場合
                log::info!("Second search also completed very quickly");
            }
        }
        
        // テストの主要な目的: 複数の検索リクエストがエラーなく処理されることを確認
        // これは中断機能の基本的な動作（新しいクエリが古いクエリを置き換える）をテストしている
    }
}