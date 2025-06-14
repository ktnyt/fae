use std::marker::PhantomData;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::message::{JsonRpcPayload, JsonRpcSendError};
use super::handler::JsonRpcHandler;

pub struct JsonRpcEngine<H: JsonRpcHandler + Send + 'static> {
    // レスポンス・リクエストを送信するためのチャンネル
    sender: mpsc::UnboundedSender<JsonRpcPayload>,
    // シャットダウン通知を送信するためのチャンネル
    shutdown_sender: Option<oneshot::Sender<()>>,
    // メインループのタスクハンドル
    task_handle: Option<JoinHandle<()>>,
    // ハンドラータイプを保持するためのマーカー
    _phantom: PhantomData<H>,
}

impl<H: JsonRpcHandler + Send + 'static> JsonRpcEngine<H> {
    pub fn new(
        receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        sender: mpsc::UnboundedSender<JsonRpcPayload>,
        handler: H,
    ) -> Self {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();
        
        // メインループを非同期タスクとして起動
        let task_handle = tokio::spawn(Self::run_main_loop_internal(
            receiver,
            sender.clone(),
            handler,
            shutdown_receiver,
        ));
        
        Self {
            sender,
            shutdown_sender: Some(shutdown_sender),
            task_handle: Some(task_handle),
            _phantom: PhantomData,
        }
    }

    pub async fn send(&self, payload: JsonRpcPayload) -> Result<(), JsonRpcSendError> {
        self.sender.send(payload).map_err(|_| JsonRpcSendError::ChannelClosed)
    }

    async fn run_main_loop_internal(
        mut receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        sender: mpsc::UnboundedSender<JsonRpcPayload>,
        mut handler: H,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                // シャットダウン通知を受信
                _ = &mut shutdown_receiver => {
                    break;
                }
                // メッセージ受信処理
                payload = receiver.recv() => {
                    match payload {
                        Some(payload) => {
                            Self::handle_received_payload(payload, &mut handler, &sender).await;
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

    async fn handle_received_payload(
        payload: JsonRpcPayload, 
        handler: &mut H, 
        sender: &mpsc::UnboundedSender<JsonRpcPayload>
    ) {
        match payload {
            JsonRpcPayload::Request(request) => {
                let response = handler.on_request(request).await;
                let response_payload = JsonRpcPayload::Response(response);
                // レスポンスを送信（エラーは無視）
                let _ = sender.send(response_payload);
            }
            JsonRpcPayload::Notification(notification) => {
                handler.on_notification(notification).await;
            }
            JsonRpcPayload::Response(_response) => {
                // レスポンスの処理は今後実装予定（リクエスト送信とのペア処理）
                // 今のところは何もしない
            }
        }
    }
}

impl<H: JsonRpcHandler + Send + 'static> Drop for JsonRpcEngine<H> {
    fn drop(&mut self) {
        // シャットダウンシグナルを送信
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }
        
        // タスクをabortしてリソースを確実にクリーンアップ
        if let Some(task_handle) = self.task_handle.take() {
            task_handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use tokio::sync::mpsc;
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
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();
        
        let engine: JsonRpcEngine<DummyHandler> = JsonRpcEngine::new(engine_rx, tx, DummyHandler);
        
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
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler);
        
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
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler);
        
        // 少し待つ
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        // engineをdropすることでシャットダウンが発生する
        drop(engine);
        
        // 少し待って確実にクリーンアップされることを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        
        // これ以上の特定の検証は難しいが、パニックしないことを確認
    }

    #[tokio::test]
    async fn test_handler_receives_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        
        let engine = JsonRpcEngine::new(engine_rx, tx, DummyHandler);
        
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
        
        // engineをdropしてクリーンアップ
        drop(engine);
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
        
        let (tx, _rx) = mpsc::unbounded_channel();
        let (engine_tx, engine_rx) = mpsc::unbounded_channel();
        
        let engine: JsonRpcEngine<TestHandler> = JsonRpcEngine::new(engine_rx, tx, test_handler);
        
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
        
        // engineをdropしてクリーンアップ
        drop(engine);
    }
}