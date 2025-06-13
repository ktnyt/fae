use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// 抽象的なメッセージ型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub method: String,
    pub payload: serde_json::Value,
    pub correlation_id: Option<String>,
}

impl Message {
    pub fn new(method: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            method: method.into(),
            payload,
            correlation_id: None,
        }
    }

    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }
}

/// メッセージバス - ワーカー間の通信を管理
#[derive(Clone)]
pub struct MessageBus {
    senders: HashMap<String, mpsc::UnboundedSender<Message>>,
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
        }
    }

    /// ワーカーをバスに登録
    pub fn register_worker(&mut self, worker_id: String, sender: mpsc::UnboundedSender<Message>) {
        self.senders.insert(worker_id, sender);
    }

    /// ワーカーをバスから削除
    pub fn unregister_worker(&mut self, worker_id: &str) {
        self.senders.remove(worker_id);
    }

    /// 特定のワーカーにメッセージを送信
    pub fn send_to(&self, worker_id: &str, message: Message) -> Result<(), MessageError> {
        if let Some(sender) = self.senders.get(worker_id) {
            sender.send(message).map_err(|_| MessageError::WorkerUnavailable)?;
            Ok(())
        } else {
            Err(MessageError::WorkerNotFound)
        }
    }

    /// 全ワーカーにブロードキャスト
    pub fn broadcast(&self, message: Message) {
        for sender in self.senders.values() {
            let _ = sender.send(message.clone());
        }
    }

    /// 特定のワーカー以外にブロードキャスト
    pub fn broadcast_except(&self, exclude_worker_id: &str, message: Message) {
        for (worker_id, sender) in &self.senders {
            if worker_id != exclude_worker_id {
                let _ = sender.send(message.clone());
            }
        }
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("Worker not found")]
    WorkerNotFound,
    #[error("Worker unavailable")]
    WorkerUnavailable,
}