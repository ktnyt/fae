use tokio::sync::{mpsc, oneshot};

use super::message::{JsonRpcPayload, JsonRpcSendError};
use super::handler::JsonRpcHandler;

pub struct JsonRpcEngine<H: JsonRpcHandler> {
    // リクエスト・通知を受信するためのチャンネル
    receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
    // レスポンス・リクエストを送信するためのチャンネル
    sender: mpsc::UnboundedSender<JsonRpcPayload>,
    // 受信時の処理を担当するハンドラー
    handler: H,
    // シャットダウン通知を受信するためのチャンネル
    shutdown_receiver: oneshot::Receiver<()>,
}

impl<H: JsonRpcHandler> JsonRpcEngine<H> {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        sender: mpsc::UnboundedSender<JsonRpcPayload>,
        handler: H,
        shutdown_receiver: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            receiver,
            sender,
            handler,
            shutdown_receiver,
        }
    }

    pub async fn send(&self, payload: JsonRpcPayload) -> Result<(), JsonRpcSendError> {
        self.sender.send(payload).map_err(|_| JsonRpcSendError::ChannelClosed)
    }

    pub async fn run_main_loop(mut self) {
        loop {
            tokio::select! {
                // シャットダウン通知を受信
                _ = &mut self.shutdown_receiver => {
                    break;
                }
                // メッセージ受信処理（後で実装）
                // payload = self.receiver.recv() => {
                //     // TODO: ハンドラーでの処理
                // }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use tokio::sync::{mpsc, oneshot};
    use crate::jsonrpc::message::{JsonRpcRequest, JsonRpcNotification, JsonRpcResponse};

    // テスト用のダミーハンドラー
    struct DummyHandler;

    #[async_trait]
    impl JsonRpcHandler for DummyHandler {
        async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
            JsonRpcResponse {
                id: request.id,
                result: Some(json!("test_result")),
                error: None,
            }
        }

        async fn on_notification(&mut self, _notification: JsonRpcNotification) {
            // 何もしない
        }
    }

    #[tokio::test]
    async fn test_send_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        let (_shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler, shutdown_rx);
        
        let request = JsonRpcRequest {
            id: 1,
            method: "test_method".to_string(),
            params: Some(json!({"key": "value"})),
        };
        
        let payload = JsonRpcPayload::Request(request.clone());
        engine.send(payload).await.unwrap();
        
        // チャンネルからメッセージを受信して確認
        let received = rx.recv().await.unwrap();
        match received {
            JsonRpcPayload::Request(received_req) => {
                assert_eq!(received_req.id, request.id);
                assert_eq!(received_req.method, request.method);
                assert_eq!(received_req.params, request.params);
            }
            _ => panic!("Expected Request payload"),
        }
    }

    #[tokio::test]
    async fn test_send_notification() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        let (_shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler, shutdown_rx);
        
        let notification = JsonRpcNotification {
            method: "test_notification".to_string(),
            params: Some(json!({"key": "value"})),
        };
        
        let payload = JsonRpcPayload::Notification(notification.clone());
        engine.send(payload).await.unwrap();
        
        // チャンネルからメッセージを受信して確認
        let received = rx.recv().await.unwrap();
        match received {
            JsonRpcPayload::Notification(received_notif) => {
                assert_eq!(received_notif.method, notification.method);
                assert_eq!(received_notif.params, notification.params);
            }
            _ => panic!("Expected Notification payload"),
        }
    }

    #[tokio::test]
    async fn test_shutdown_stops_main_loop() {
        let (tx, rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler, shutdown_rx);
        
        // メインループを非同期で開始
        let loop_handle = tokio::spawn(async move {
            engine.run_main_loop().await;
        });
        
        // 少し待ってからシャットダウンシグナルを送信
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        shutdown_tx.send(()).unwrap();
        
        // メインループが終了することを確認
        let result = tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            loop_handle
        ).await;
        
        assert!(result.is_ok(), "Main loop should have stopped after shutdown signal");
    }
}