use crate::workers::{Worker, Message, WorkerMessage, TuiMessage, SearchResultMessage};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// SearchHandler - TUIとBaseSearcher間の仲介役
pub struct SearchHandler {
    worker_id: String,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    current_query: Option<String>,
    content_searcher_id: String,
}

impl SearchHandler {
    pub fn new(worker_id: String) -> Self {
        Self {
            worker_id,
            message_bus: None,
            current_query: None,
            content_searcher_id: "content_searcher".to_string(),
        }
    }

    pub fn set_message_bus(&mut self, message_bus: Arc<RwLock<crate::workers::MessageBus>>) {
        self.message_bus = Some(message_bus);
    }

    async fn send_to_tui(&self, message: WorkerMessage) -> Result<(), String> {
        if let Some(bus) = &self.message_bus {
            let msg: Message = message.into();
            let bus_guard = bus.read().await;
            bus_guard.send_to("tui", msg)
                .map_err(|e| format!("Failed to send to TUI: {}", e))?;
        }
        Ok(())
    }

    async fn send_to_searcher(&self, searcher_id: &str, message: WorkerMessage) -> Result<(), String> {
        if let Some(bus) = &self.message_bus {
            let msg: Message = message.into();
            let bus_guard = bus.read().await;
            bus_guard.send_to(searcher_id, msg)
                .map_err(|e| format!("Failed to send to searcher {}: {}", searcher_id, e))?;
        }
        Ok(())
    }

    async fn handle_tui_message(&mut self, message: TuiMessage) -> Result<(), String> {
        match message {
            TuiMessage::UserQuery { query } => {
                self.current_query = Some(query.clone());
                
                // まず検索結果をクリア
                self.send_to_tui(WorkerMessage::search_clear()).await?;
                
                // ContentSearcherに検索クエリを送信
                let search_msg = WorkerMessage::search_query(query);
                self.send_to_searcher(&self.content_searcher_id, search_msg).await?;
            }
        }
        Ok(())
    }

    async fn handle_search_result(&mut self, message: SearchResultMessage) -> Result<(), String> {
        match message {
            SearchResultMessage::SearchClear => {
                // Searcherからのクリア要求をTUIに転送
                self.send_to_tui(WorkerMessage::search_clear()).await?;
            }
            SearchResultMessage::SearchMatch { filename, line, column, content } => {
                // SearcherからのマッチをTUIに転送
                let match_msg = WorkerMessage::search_match(filename, line, column, content);
                self.send_to_tui(match_msg).await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Worker for SearchHandler {
    fn worker_id(&self) -> &str {
        &self.worker_id
    }

    async fn handle_message(&mut self, message: Message) -> Result<(), crate::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            let result = match worker_msg {
                WorkerMessage::Tui(tui_msg) => {
                    self.handle_tui_message(tui_msg).await
                }
                WorkerMessage::SearchResult(search_result) => {
                    self.handle_search_result(search_result).await
                }
                _ => {
                    // 他のメッセージタイプは処理しない
                    Ok(())
                }
            };

            if let Err(e) = result {
                return Err(crate::workers::worker::WorkerError::MessageHandlingFailed(e));
            }
        }
        Ok(())
    }
}