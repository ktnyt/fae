use crate::workers::{Worker, Message, WorkerMessage, SearchQueryMessage};
use crate::searchers::ContentSearcher as CoreContentSearcher;
use async_trait::async_trait;
use std::sync::Arc;
use std::path::Path;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// ContentSearcher - リテラル検索を行うワーカー
pub struct ContentSearcher {
    worker_id: String,
    search_handler_id: String,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    core_searcher: CoreContentSearcher,
    cancellation_token: Option<CancellationToken>,
}

impl ContentSearcher {
    pub fn new(worker_id: String, search_handler_id: String, search_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            worker_id,
            search_handler_id,
            message_bus: None,
            core_searcher: CoreContentSearcher::new(search_path.as_ref().to_path_buf())?,
            cancellation_token: None,
        })
    }

    pub fn set_message_bus(&mut self, message_bus: Arc<RwLock<crate::workers::MessageBus>>) {
        self.message_bus = Some(message_bus);
    }

    async fn send_to_search_handler(&self, message: WorkerMessage) -> Result<(), String> {
        if let Some(bus) = &self.message_bus {
            let msg: Message = message.into();
            let bus_guard = bus.read().await;
            bus_guard.send_to(&self.search_handler_id, msg)
                .map_err(|e| format!("Failed to send to SearchHandler: {}", e))?;
        }
        Ok(())
    }

    async fn execute_search(&mut self, query: &str) -> Result<(), String> {
        // キャンセルトークンを作成
        let cancel_token = CancellationToken::new();
        self.cancellation_token = Some(cancel_token.clone());

        // 検索結果をクリア
        self.send_to_search_handler(WorkerMessage::search_result_clear()).await?;

        // 検索を実行（既存のContentSearcherを使用）
        let mut search_stream = self.core_searcher.search_stream(query)
            .map_err(|e| format!("Failed to create search stream: {}", e))?;
        
        // ストリームから結果を読み取って送信
        loop {
            // キャンセルチェック
            if cancel_token.is_cancelled() {
                break;
            }

            // 非ブロッキングで結果を取得
            match search_stream.next() {
                Some(search_result) => {
                    // DisplayInfoからコンテンツを取得
                    let content = match &search_result.display_info {
                        crate::types::DisplayInfo::Content { line_content, .. } => line_content.clone(),
                        _ => "".to_string(),
                    };

                    let message = WorkerMessage::search_result_match(
                        search_result.file_path.to_string_lossy().to_string(),
                        search_result.line,
                        search_result.column,
                        content,
                    );
                    self.send_to_search_handler(message).await?;
                }
                None => {
                    // ストリーム終了
                    break;
                }
            }

            // CPU使用率を下げるため少し待機
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        Ok(())
    }

    async fn handle_search_query(&mut self, query: String) -> Result<(), crate::workers::worker::WorkerError> {
        // 現在の検索をキャンセル
        if let Some(token) = &self.cancellation_token {
            token.cancel();
        }

        // 直接検索を実行
        if let Err(e) = self.execute_search(&query).await {
            eprintln!("Search execution error: {}", e);
        }

        Ok(())
    }
}

#[async_trait]
impl Worker for ContentSearcher {
    fn worker_id(&self) -> &str {
        &self.worker_id
    }

    async fn handle_message(&mut self, message: Message) -> Result<(), crate::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            match worker_msg {
                WorkerMessage::SearchQuery(SearchQueryMessage::UserQuery { query }) => {
                    self.handle_search_query(query).await?;
                }
                _ => {
                    // 他のメッセージタイプは処理しない
                }
            }
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), crate::workers::worker::WorkerError> {
        // 進行中の検索をキャンセル
        if let Some(token) = &self.cancellation_token {
            token.cancel();
        }
        Ok(())
    }
}