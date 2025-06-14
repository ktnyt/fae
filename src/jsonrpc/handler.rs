use async_trait::async_trait;
use tokio::sync::mpsc;

use super::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcPayload};

#[async_trait]
pub trait JsonRpcHandler {
    async fn on_request(
        &mut self, 
        request: JsonRpcRequest,
        sender: &mpsc::UnboundedSender<JsonRpcPayload>,
    ) -> JsonRpcResponse;
    
    async fn on_notification(
        &mut self, 
        notification: JsonRpcNotification,
        sender: &mpsc::UnboundedSender<JsonRpcPayload>,
    );
}
