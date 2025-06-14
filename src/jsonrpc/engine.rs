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
                // メッセージ受信処理
                payload = self.receiver.recv() => {
                    match payload {
                        Some(payload) => {
                            self.handle_received_payload(payload).await;
                        }
                        None => {
                            // チャンネルが閉じられた場合はループを終了
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_received_payload(&mut self, payload: JsonRpcPayload) {
        match payload {
            JsonRpcPayload::Request(request) => {
                let response = self.handler.on_request(request).await;
                let response_payload = JsonRpcPayload::Response(response);
                // レスポンスを送信（エラーは無視）
                let _ = self.sender.send(response_payload);
            }
            JsonRpcPayload::Notification(notification) => {
                self.handler.on_notification(notification).await;
            }
            JsonRpcPayload::Response(_response) => {
                // レスポンスの処理は今後実装予定（リクエスト送信とのペア処理）
                // 今のところは何もしない
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

    #[tokio::test]
    async fn test_handler_receives_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler, shutdown_rx);
        
        // メインループを非同期で開始
        let loop_handle = tokio::spawn(async move {
            engine.run_main_loop().await;
        });
        
        // リクエストを送信
        let request = JsonRpcRequest {
            id: 42,
            method: "test_method".to_string(),
            params: Some(json!({"test": "data"})),
        };
        engine_tx.send(JsonRpcPayload::Request(request)).unwrap();
        
        // ハンドラーからのレスポンスを受信
        let response = rx.recv().await.unwrap();
        match response {
            JsonRpcPayload::Response(resp) => {
                assert_eq!(resp.id, 42);
                assert_eq!(resp.result, Some(json!("test_result")));
                assert!(resp.error.is_none());
            }
            _ => panic!("Expected Response payload"),
        }
        
        // クリーンアップ
        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            loop_handle
        ).await;
    }

    #[tokio::test]
    async fn test_handler_receives_notification() {
        use std::sync::{Arc, Mutex};
        
        // 通知を受信したかどうかを記録するハンドラー
        #[derive(Clone)]
        struct TestHandler {
            received_notifications: Arc<Mutex<Vec<JsonRpcNotification>>>,
        }
        
        #[async_trait]
        impl JsonRpcHandler for TestHandler {
            async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("test_result")),
                    error: None,
                }
            }

            async fn on_notification(&mut self, notification: JsonRpcNotification) {
                let mut notifications = self.received_notifications.lock().unwrap();
                notifications.push(notification);
            }
        }
        
        let received_notifications = Arc::new(Mutex::new(Vec::new()));
        let test_handler = TestHandler {
            received_notifications: received_notifications.clone(),
        };
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, test_handler, shutdown_rx);
        
        // メインループを非同期で開始
        let loop_handle = tokio::spawn(async move {
            engine.run_main_loop().await;
        });
        
        // 通知を送信
        let notification = JsonRpcNotification {
            method: "test_notification".to_string(),
            params: Some(json!({"notification": "data"})),
        };
        engine_tx.send(JsonRpcPayload::Notification(notification.clone())).unwrap();
        
        // 少し待って通知が処理されることを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let notifications = received_notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, notification.method);
        assert_eq!(notifications[0].params, notification.params);
        
        // クリーンアップ
        shutdown_tx.send(()).unwrap();
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            loop_handle
        ).await;
    }
}