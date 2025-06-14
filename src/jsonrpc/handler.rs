use async_trait::async_trait;

use super::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

#[async_trait]
pub trait JsonRpcHandler {
    async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse;
    async fn on_notification(&mut self, notification: JsonRpcNotification);
}
