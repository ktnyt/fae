use crate::workers::message::{Message, MessageBus};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// ワーカーの抽象トレイト
#[async_trait]
pub trait Worker: Send + Sync + 'static {
    /// ワーカーの一意識別子
    fn worker_id(&self) -> &str;

    /// ワーカーの初期化処理
    async fn initialize(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }

    /// メッセージ処理のメインループ
    async fn handle_message(&mut self, message: Message) -> Result<(), WorkerError>;

    /// ワーカーの終了処理
    async fn cleanup(&mut self) -> Result<(), WorkerError> {
        Ok(())
    }

    /// ワーカーが他のワーカーにメッセージを送信するためのヘルパー
    fn get_message_bus(&self) -> Option<Arc<RwLock<MessageBus>>> {
        None
    }
}

/// ワーカーのハンドル - 外部からワーカーを制御するためのインターフェース
pub struct WorkerHandle {
    worker_id: String,
    message_sender: mpsc::UnboundedSender<Message>,
    join_handle: JoinHandle<Result<(), WorkerError>>,
    cancellation_token: CancellationToken,
}

impl WorkerHandle {
    /// 新しいワーカーを起動
    pub async fn spawn<W: Worker>(
        mut worker: W,
        message_bus: Arc<RwLock<MessageBus>>,
    ) -> Result<Self, WorkerError> {
        let worker_id = worker.worker_id().to_string();
        let (message_sender, mut message_receiver) = mpsc::unbounded_channel::<Message>();
        let cancellation_token = CancellationToken::new();
        let cancel_token_clone = cancellation_token.clone();

        // メッセージバスにワーカーを登録
        {
            let mut bus = message_bus.write().await;
            bus.register_worker(worker_id.clone(), message_sender.clone());
        }

        let join_handle = tokio::spawn(async move {
            // 初期化
            worker.initialize().await?;

            loop {
                tokio::select! {
                    // キャンセル要求を受信
                    _ = cancel_token_clone.cancelled() => {
                        break;
                    }
                    
                    // メッセージを受信
                    message = message_receiver.recv() => {
                        match message {
                            Some(msg) => {
                                if let Err(e) = worker.handle_message(msg).await {
                                    eprintln!("Worker {} error: {}", worker.worker_id(), e);
                                    // エラーが発生しても継続
                                }
                            }
                            None => {
                                // チャネルが閉じられた
                                break;
                            }
                        }
                    }
                }
            }

            // クリーンアップ
            worker.cleanup().await?;

            // メッセージバスからワーカーを削除
            {
                let mut bus = message_bus.write().await;
                bus.unregister_worker(&worker.worker_id());
            }

            Ok(())
        });

        Ok(Self {
            worker_id,
            message_sender,
            join_handle,
            cancellation_token,
        })
    }

    /// ワーカーID取得
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// ワーカーにメッセージを送信
    pub fn send_message(&self, message: Message) -> Result<(), WorkerError> {
        self.message_sender
            .send(message)
            .map_err(|_| WorkerError::WorkerUnavailable)
    }

    /// ワーカーを優雅に停止
    pub async fn shutdown(self) -> Result<(), WorkerError> {
        // キャンセル要求を送信
        self.cancellation_token.cancel();
        
        // ワーカーの終了を待機
        match self.join_handle.await {
            Ok(result) => result,
            Err(e) => Err(WorkerError::JoinError(e.to_string())),
        }
    }

    /// ワーカーを強制停止
    pub fn abort(&self) {
        self.join_handle.abort();
    }

    /// ワーカーが終了したかチェック
    pub fn is_finished(&self) -> bool {
        self.join_handle.is_finished()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("Worker initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Message handling failed: {0}")]
    MessageHandlingFailed(String),
    #[error("Worker cleanup failed: {0}")]
    CleanupFailed(String),
    #[error("Worker unavailable")]
    WorkerUnavailable,
    #[error("Join error: {0}")]
    JoinError(String),
    #[error("Communication error: {0}")]
    CommunicationError(String),
}

/// ワーカーマネージャー - 複数のワーカーを統合管理
pub struct WorkerManager {
    message_bus: Arc<RwLock<MessageBus>>,
    workers: Vec<WorkerHandle>,
}

impl WorkerManager {
    pub fn new() -> Self {
        Self {
            message_bus: Arc::new(RwLock::new(MessageBus::new())),
            workers: Vec::new(),
        }
    }

    /// ワーカーを追加
    pub async fn add_worker<W: Worker>(&mut self, worker: W) -> Result<(), WorkerError> {
        let handle = WorkerHandle::spawn(worker, self.message_bus.clone()).await?;
        self.workers.push(handle);
        Ok(())
    }

    /// メッセージバスを取得
    pub fn get_message_bus(&self) -> Arc<RwLock<MessageBus>> {
        self.message_bus.clone()
    }

    /// 特定のワーカーにメッセージを送信
    pub async fn send_message(&self, worker_id: &str, message: Message) -> Result<(), WorkerError> {
        let bus = self.message_bus.read().await;
        bus.send_to(worker_id, message)
            .map_err(|e| WorkerError::CommunicationError(e.to_string()))
    }

    /// 全ワーカーにブロードキャスト
    pub async fn broadcast(&self, message: Message) {
        let bus = self.message_bus.read().await;
        bus.broadcast(message);
    }

    /// 全ワーカーを停止
    pub async fn shutdown_all(self) -> Result<(), WorkerError> {
        let mut errors = Vec::new();
        
        for worker in self.workers {
            if let Err(e) = worker.shutdown().await {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(WorkerError::CleanupFailed(format!(
                "Multiple worker shutdown errors: {:?}",
                errors
            )))
        }
    }
}

impl Default for WorkerManager {
    fn default() -> Self {
        Self::new()
    }
}