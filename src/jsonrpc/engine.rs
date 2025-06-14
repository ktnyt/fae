use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tokio::sync::{mpsc, oneshot};

use super::handler::JsonRpcHandler;
use super::message::{JsonRpcPayload, JsonRpcRequest, JsonRpcResponse, JsonRpcSendError};

#[derive(Debug)]
pub enum JsonRpcRequestError {
    SendError(JsonRpcSendError),
    ResponseTimeout,
}

impl From<JsonRpcSendError> for JsonRpcRequestError {
    fn from(err: JsonRpcSendError) -> Self {
        JsonRpcRequestError::SendError(err)
    }
}

pub struct JsonRpcEngine<H: JsonRpcHandler + Send + 'static> {
    // 外部へのレスポンス送信用チャンネル
    sender: mpsc::UnboundedSender<JsonRpcPayload>,
    // 内部からメインループへのリクエスト送信用チャンネル
    internal_sender: mpsc::UnboundedSender<JsonRpcPayload>,
    // シャットダウン通知を送信するためのチャンネル
    shutdown_sender: Option<oneshot::Sender<()>>,
    // メインループのスレッドハンドル
    thread_handle: Option<JoinHandle<()>>,
    // リクエストID -> レスポンス待機チャンネルのマップ
    pending_requests:
        Arc<Mutex<HashMap<u64, oneshot::Sender<crate::jsonrpc::message::JsonRpcResponse>>>>,
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
        let sender_clone = sender.clone();
        let pending_requests = Arc::new(Mutex::new(HashMap::new()));
        let pending_requests_clone = pending_requests.clone();

        // 内部通信用チャンネルを作成
        let (internal_sender, internal_receiver) = mpsc::unbounded_channel();

        // メインループを別スレッドで実行
        let thread_handle = std::thread::spawn(move || {
            // 新しいtokioランタイムを作成してメインループを実行
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(Self::run_main_loop_internal(
                receiver,
                internal_receiver,
                sender_clone,
                handler,
                shutdown_receiver,
                pending_requests_clone,
            ));
        });

        Self {
            sender,
            internal_sender,
            shutdown_sender: Some(shutdown_sender),
            thread_handle: Some(thread_handle),
            pending_requests,
            _phantom: PhantomData,
        }
    }

    pub async fn send(&self, payload: JsonRpcPayload) -> Result<(), JsonRpcSendError> {
        self.sender
            .send(payload)
            .map_err(|_| JsonRpcSendError::ChannelClosed)
    }

    pub async fn send_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, JsonRpcRequestError> {
        log::debug!(
            "Sending request: id={}, method={}",
            request.id,
            request.method
        );

        let (tx, rx) = oneshot::channel();

        // pending_requestsに登録
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.insert(request.id, tx);
            log::trace!(
                "Registered pending request: id={}, total_pending={}",
                request.id,
                pending.len()
            );
        }

        // 内部チャンネル経由でリクエストを送信
        self.internal_sender
            .send(JsonRpcPayload::Request(request))
            .map_err(|_| JsonRpcSendError::ChannelClosed)?;

        // レスポンスを待機（タイムアウト付き）
        log::trace!("Waiting for response...");
        match tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30秒タイムアウト
            rx,
        )
        .await
        {
            Ok(Ok(response)) => {
                log::debug!("Received response: id={}", response.id);
                Ok(response)
            }
            Ok(Err(_)) => {
                log::warn!("Response channel closed unexpectedly");
                Err(JsonRpcRequestError::ResponseTimeout)
            }
            Err(_) => {
                log::warn!("Request timed out after 30 seconds");
                Err(JsonRpcRequestError::ResponseTimeout)
            }
        }
    }

    async fn run_main_loop_internal(
        mut external_receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        mut internal_receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        sender: mpsc::UnboundedSender<JsonRpcPayload>,
        mut handler: H,
        mut shutdown_receiver: oneshot::Receiver<()>,
        pending_requests: Arc<
            Mutex<HashMap<u64, oneshot::Sender<crate::jsonrpc::message::JsonRpcResponse>>>,
        >,
    ) {
        loop {
            tokio::select! {
                // シャットダウン通知を受信
                _ = &mut shutdown_receiver => {
                    log::debug!("Received shutdown signal, stopping main loop");
                    break;
                }
                // 外部からのメッセージ受信処理
                payload = external_receiver.recv() => {
                    match payload {
                        Some(payload) => {
                            log::trace!("Received external payload");
                            Self::handle_received_payload(payload, &mut handler, &sender, &pending_requests).await;
                        }
                        None => {
                            log::debug!("External receiver channel closed");
                            break;
                        }
                    }
                }
                // 内部からのメッセージ受信処理
                payload = internal_receiver.recv() => {
                    match payload {
                        Some(payload) => {
                            log::trace!("Received internal payload");
                            Self::handle_received_payload(payload, &mut handler, &sender, &pending_requests).await;
                        }
                        None => {
                            log::debug!("Internal receiver channel closed");
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
        sender: &mpsc::UnboundedSender<JsonRpcPayload>,
        pending_requests: &Arc<
            Mutex<HashMap<u64, oneshot::Sender<crate::jsonrpc::message::JsonRpcResponse>>>,
        >,
    ) {
        match payload {
            JsonRpcPayload::Request(request) => {
                log::debug!(
                    "Handling request: id={}, method={}",
                    request.id,
                    request.method
                );
                let response = handler.on_request(request).await;
                log::trace!(
                    "Handler response: id={}, has_result={}, has_error={}",
                    response.id,
                    response.result.is_some(),
                    response.error.is_some()
                );

                // pending_requestsから対応するチャンネルを取得してレスポンスを送信
                let mut pending = pending_requests.lock().unwrap();
                if let Some(tx) = pending.remove(&response.id) {
                    log::trace!(
                        "Forwarding handler response to waiting request: id={}",
                        response.id
                    );
                    let _ = tx.send(response.clone());
                }

                // 外部向けにもレスポンスを送信
                let response_payload = JsonRpcPayload::Response(response);
                let _ = sender.send(response_payload);
            }
            JsonRpcPayload::Notification(notification) => {
                log::debug!("Handling notification: method={}", notification.method);
                handler.on_notification(notification).await;
            }
            JsonRpcPayload::Response(response) => {
                log::debug!("Handling response: id={}", response.id);
                // pending_requestsから対応するチャンネルを取得してレスポンスを送信
                let mut pending = pending_requests.lock().unwrap();
                if let Some(tx) = pending.remove(&response.id) {
                    log::trace!("Forwarding response to waiting request: id={}", response.id);
                    let _ = tx.send(response);
                } else {
                    log::warn!("Received response for unknown request id: {}", response.id);
                }
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

        // スレッドのjoinを待つ（graceful shutdown）
        if let Some(thread_handle) = self.thread_handle.take() {
            let _ = thread_handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonrpc::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
    use async_trait::async_trait;
    use serde_json::json;
    use tokio::sync::mpsc;

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

    // ping/pongテスト用のハンドラー
    struct PingPongHandler;

    #[async_trait]
    impl JsonRpcHandler for PingPongHandler {
        async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
            log::info!(
                "PingPongHandler received request: method={}",
                request.method
            );

            match request.method.as_str() {
                "ping" => {
                    log::info!("Responding with pong to request id={}", request.id);
                    JsonRpcResponse {
                        id: request.id,
                        result: Some(json!("pong")),
                        error: None,
                    }
                }
                _ => {
                    log::warn!("Unknown method: {}", request.method);
                    JsonRpcResponse {
                        id: request.id,
                        result: None,
                        error: Some(crate::jsonrpc::message::JsonRpcError::method_not_found(
                            Some(format!("Method '{}' not found", request.method)),
                            Some(serde_json::json!({"method": request.method})),
                        )),
                    }
                }
            }
        }

        async fn on_notification(&mut self, notification: JsonRpcNotification) {
            log::info!(
                "PingPongHandler received notification: method={}",
                notification.method
            );
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
    async fn test_send_request_with_handler_response() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<DummyHandler> = JsonRpcEngine::new(engine_rx, tx, DummyHandler);

        let request = JsonRpcRequest {
            id: 42,
            method: "test_method".to_string(),
            params: Some(json!({"test": "data"})),
        };

        // send_requestを実行（ハンドラーからのレスポンスを待機）
        let received_response = engine.send_request(request.clone()).await.unwrap();
        assert_eq!(received_response.id, 42);
        assert_eq!(received_response.result, Some(json!("test_result"))); // DummyHandlerの応答
        assert!(received_response.error.is_none());

        // engineをdropしてクリーンアップ
        drop(engine);
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
        engine_tx
            .send(JsonRpcPayload::Notification(notification.clone()))
            .unwrap();

        // 少し待って通知が処理されることを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let notifications = received_notifications.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].method, notification.method);
        assert_eq!(notifications[0].params, notification.params);

        // engineをdropしてクリーンアップ
        drop(engine);
    }

    #[tokio::test]
    async fn test_ping_pong_integration() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<PingPongHandler> =
            JsonRpcEngine::new(engine_rx, tx, PingPongHandler);

        let ping_request = JsonRpcRequest {
            id: 100,
            method: "ping".to_string(),
            params: None,
        };

        log::info!("Starting ping/pong integration test");

        // send_requestでpingを送信してpongレスポンスを待機
        let response = engine.send_request(ping_request).await.unwrap();

        log::info!("Received response: {:?}", response);

        // レスポンスの検証
        assert_eq!(response.id, 100);
        assert_eq!(response.result, Some(json!("pong")));
        assert!(response.error.is_none());

        log::info!("ping/pong test completed successfully!");

        // engineをdropしてクリーンアップ
        drop(engine);
    }

    #[tokio::test]
    async fn test_ping_pong_unknown_method() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<PingPongHandler> =
            JsonRpcEngine::new(engine_rx, tx, PingPongHandler);

        let unknown_request = JsonRpcRequest {
            id: 101,
            method: "unknown_method".to_string(),
            params: None,
        };

        log::info!("Testing unknown method handling");

        // unknown methodを送信してエラーレスポンスを期待
        let response = engine.send_request(unknown_request).await.unwrap();

        log::info!("Received error response: {:?}", response);

        // エラーレスポンスの検証
        assert_eq!(response.id, 101);
        assert!(response.result.is_none());
        assert!(response.error.is_some());

        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method 'unknown_method' not found");

        log::info!("Unknown method test completed successfully!");

        // engineをdropしてクリーンアップ
        drop(engine);
    }
}
