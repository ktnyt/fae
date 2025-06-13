use crate::workers::{Worker, Message, WorkerMessage, SearchQueryMessage};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio_util::sync::CancellationToken;

/// BaseSearcher - 全ての検索ワーカーの基底クラス
#[async_trait]
pub trait BaseSearcher: Worker {
    /// 検索を実行する（具象クラスで実装）
    async fn execute_search(&mut self, query: &str) -> Result<Vec<SearchMatch>, SearchError>;

    /// 検索結果を送信するためのメッセージバスを取得
    fn get_message_bus(&self) -> Option<Arc<RwLock<crate::workers::MessageBus>>>;

    /// SearchHandlerのワーカーIDを取得
    fn get_search_handler_id(&self) -> &str;

    /// 現在のキャンセルトークンを取得
    fn get_cancellation_token(&self) -> Option<CancellationToken>;

    /// 新しいキャンセルトークンを設定
    fn set_cancellation_token(&mut self, token: CancellationToken);
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub filename: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Search cancelled")]
    Cancelled,
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Search error: {0}")]
    SearchError(String),
}

/// BaseSearcherの共通実装を提供するヘルパー構造体
pub struct BaseSearcherImpl {
    worker_id: String,
    search_handler_id: String,
    message_bus: Option<Arc<RwLock<crate::workers::MessageBus>>>,
    current_query: Arc<Mutex<Option<String>>>,
    cancellation_token: Option<CancellationToken>,
}

impl BaseSearcherImpl {
    pub fn new(worker_id: String, search_handler_id: String) -> Self {
        Self {
            worker_id,
            search_handler_id,
            message_bus: None,
            current_query: Arc::new(Mutex::new(None)),
            cancellation_token: None,
        }
    }

    pub fn set_message_bus(&mut self, message_bus: Arc<RwLock<crate::workers::MessageBus>>) {
        self.message_bus = Some(message_bus);
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn get_message_bus(&self) -> Option<Arc<RwLock<crate::workers::MessageBus>>> {
        self.message_bus.clone()
    }

    pub fn get_search_handler_id(&self) -> &str {
        &self.search_handler_id
    }

    pub fn get_cancellation_token(&self) -> Option<CancellationToken> {
        self.cancellation_token.clone()
    }

    pub fn set_cancellation_token(&mut self, token: CancellationToken) {
        self.cancellation_token = Some(token);
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

    pub async fn handle_search_query<T>(&mut self, query: String, searcher: &mut T) -> Result<(), crate::workers::worker::WorkerError> 
    where 
        T: BaseSearcher + Send,
    {
        // 現在の検索をキャンセル
        if let Some(token) = &self.cancellation_token {
            token.cancel();
        }

        // 新しいキャンセルトークンを作成
        let new_token = CancellationToken::new();
        self.set_cancellation_token(new_token.clone());
        searcher.set_cancellation_token(new_token.clone());

        // クエリを保存
        {
            let mut current_query = self.current_query.lock().await;
            *current_query = Some(query.clone());
        }

        // 検索結果をクリア
        self.send_to_search_handler(WorkerMessage::search_result_clear()).await
            .map_err(|e| crate::workers::worker::WorkerError::MessageHandlingFailed(e))?;

        // 検索を実行（バックグラウンドで）
        let _current_query = self.current_query.clone();
        let _message_bus = self.message_bus.clone();
        let _search_handler_id = self.search_handler_id.clone();
        let _cancellation_token = new_token.clone();

        tokio::spawn(async move {
            // 実際の検索実行はここで行う
            // searcher.execute_search()を呼び出して結果を送信
            // 注意: searcher自体は移動できないので、別の方法で検索を実行する必要がある
        });

        Ok(())
    }

    pub async fn send_search_match(&self, search_match: SearchMatch) -> Result<(), String> {
        let message = WorkerMessage::search_result_match(
            search_match.filename,
            search_match.line,
            search_match.column,
            search_match.content,
        );
        self.send_to_search_handler(message).await
    }

    pub async fn handle_message(&mut self, message: Message) -> Result<(), crate::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            match worker_msg {
                WorkerMessage::SearchQuery(SearchQueryMessage::UserQuery { query: _ }) => {
                    // この部分は具象クラスでhandle_search_queryを呼び出す必要がある
                    // 抽象トレイトの制限によりここでは実装できない
                    Ok(())
                }
                _ => Ok(())
            }
        } else {
            Ok(())
        }
    }
}