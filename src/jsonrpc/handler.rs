use async_trait::async_trait;
use tokio::sync::mpsc;

use super::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcPayload, JsonRpcSendError};
use super::engine::JsonRpcRequestError;

/// JsonRPC双方向通信インターフェース
/// ハンドラーがリクエスト送信→レスポンス待機と通知送信を実行可能にする
#[async_trait]
pub trait JsonRpcSender: Send + Sync {
    /// リクエストを送信してレスポンスを待機
    async fn send_request(
        &self,
        method: String,
        params: Option<serde_json::Value>,
    ) -> Result<JsonRpcResponse, JsonRpcRequestError>;

    /// 通知を送信（レスポンス待機なし）
    async fn send_notification(
        &self,
        method: String,
        params: Option<serde_json::Value>,
    ) -> Result<(), JsonRpcSendError>;
}

#[async_trait]
pub trait JsonRpcHandler {
    async fn on_request(
        &mut self, 
        request: JsonRpcRequest,
        sender: &dyn JsonRpcSender,
    ) -> JsonRpcResponse;
    
    async fn on_notification(
        &mut self, 
        notification: JsonRpcNotification,
        sender: &dyn JsonRpcSender,
    );
}
