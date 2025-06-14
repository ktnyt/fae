use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
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
    // ハンドラーから通知を受信するためのチャンネル（送信側）
    notification_sender: mpsc::UnboundedSender<JsonRpcPayload>,
    // シャットダウン通知を送信するためのチャンネル
    shutdown_sender: Option<oneshot::Sender<()>>,
    // メインループのスレッドハンドル
    thread_handle: Option<JoinHandle<()>>,
    // リクエストID -> レスポンス待機チャンネルのマップ
    pending_requests:
        Arc<Mutex<HashMap<u64, oneshot::Sender<crate::jsonrpc::message::JsonRpcResponse>>>>,
    // 次のリクエストIDのためのatomicカウンター
    next_id: AtomicU64,
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
        
        // ハンドラーからの通知用チャンネルを作成
        let (notification_sender, notification_receiver) = mpsc::unbounded_channel();

        // メインループを別スレッドで実行
        let thread_handle = std::thread::spawn(move || {
            // 新しいtokioランタイムを作成してメインループを実行
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(Self::run_main_loop_internal(
                receiver,
                internal_receiver,
                notification_receiver,
                sender_clone,
                handler,
                shutdown_receiver,
                pending_requests_clone,
            ));
        });

        Self {
            sender,
            internal_sender,
            notification_sender,
            shutdown_sender: Some(shutdown_sender),
            thread_handle: Some(thread_handle),
            pending_requests,
            next_id: AtomicU64::new(1),
            _phantom: PhantomData,
        }
    }

    pub async fn send(&self, payload: JsonRpcPayload) -> Result<(), JsonRpcSendError> {
        self.sender
            .send(payload)
            .map_err(|_| JsonRpcSendError::ChannelClosed)
    }

    /// ハンドラーからの通知を受信するためのチャンネル送信端を取得
    pub fn notification_sender(&self) -> mpsc::UnboundedSender<JsonRpcPayload> {
        self.notification_sender.clone()
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

    /// Send a request with automatic ID generation and configurable timeout
    ///
    /// # Arguments
    /// * `method` - The JSON-RPC method name
    /// * `params` - Optional parameters for the request
    /// * `timeout_ms` - Timeout in milliseconds (0 for no timeout)
    pub async fn request(
        &self,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
        timeout_ms: u64,
    ) -> Result<JsonRpcResponse, JsonRpcRequestError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let method = method.into();

        log::debug!("Sending request: id={}, method={}", id, method);

        let request = JsonRpcRequest {
            id,
            method: method.clone(),
            params,
        };

        let (tx, rx) = oneshot::channel();

        // pending_requestsに登録
        {
            let mut pending = self.pending_requests.lock().unwrap();
            pending.insert(id, tx);
            log::trace!(
                "Registered pending request: id={}, total_pending={}",
                id,
                pending.len()
            );
        }

        // 内部チャンネル経由でリクエストを送信
        self.internal_sender
            .send(JsonRpcPayload::Request(request))
            .map_err(|_| JsonRpcSendError::ChannelClosed)?;

        // レスポンスを待機（タイムアウトの設定）
        log::trace!("Waiting for response...");

        if timeout_ms == 0 {
            // タイムアウトなし
            match rx.await {
                Ok(response) => {
                    log::debug!("Received response: id={}", response.id);
                    Ok(response)
                }
                Err(_) => {
                    log::warn!("Response channel closed unexpectedly");
                    Err(JsonRpcRequestError::ResponseTimeout)
                }
            }
        } else {
            // タイムアウト付き
            match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), rx).await {
                Ok(Ok(response)) => {
                    log::debug!("Received response: id={}", response.id);
                    Ok(response)
                }
                Ok(Err(_)) => {
                    log::warn!("Response channel closed unexpectedly");
                    Err(JsonRpcRequestError::ResponseTimeout)
                }
                Err(_) => {
                    log::warn!("Request timed out after {}ms", timeout_ms);
                    Err(JsonRpcRequestError::ResponseTimeout)
                }
            }
        }
    }

    async fn run_main_loop_internal(
        mut external_receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        mut internal_receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
        mut notification_receiver: mpsc::UnboundedReceiver<JsonRpcPayload>,
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
                // ハンドラーからの通知受信処理（新機能）
                payload = notification_receiver.recv() => {
                    match payload {
                        Some(payload) => {
                            log::debug!("Received notification from handler, forwarding to external: {:?}", payload);
                            // 通知を外部に直接転送
                            if let Err(e) = sender.send(payload) {
                                log::error!("Failed to forward notification to external: {}", e);
                            }
                        }
                        None => {
                            log::debug!("Notification receiver channel closed");
                            // 通知チャンネルが閉じてもメインループは継続
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

    /// 手動でエンジンをシャットダウンする
    /// stdio終了などのfail-safe処理で使用
    pub fn shutdown(&mut self) {
        log::info!("Manual shutdown requested for JsonRpcEngine");

        // シャットダウンシグナルを送信
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            log::debug!("Sending shutdown signal");
            let _ = shutdown_sender.send(());
        }

        // スレッドのjoinを待つ（graceful shutdown）
        if let Some(thread_handle) = self.thread_handle.take() {
            log::debug!("Waiting for main loop thread to finish");
            let _ = thread_handle.join();
        }

        log::info!("JsonRpcEngine shutdown completed");
    }
}

impl<H: JsonRpcHandler + Send + 'static> Drop for JsonRpcEngine<H> {
    fn drop(&mut self) {
        // 手動shutdownが呼ばれていない場合のみ実行
        if self.shutdown_sender.is_some() || self.thread_handle.is_some() {
            log::debug!("JsonRpcEngine dropped without explicit shutdown, performing cleanup");
            self.shutdown();
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

    // 双方向通信用のハンドラー実装
    struct PingHandler {
        counter: Arc<Mutex<u64>>,
    }

    impl PingHandler {
        fn new() -> Self {
            Self {
                counter: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl JsonRpcHandler for PingHandler {
        async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
            log::info!("PingHandler received request: method={}", request.method);

            match request.method.as_str() {
                "pong" => {
                    // pongを受信したらカウンターをインクリメント
                    let mut counter = self.counter.lock().unwrap();
                    *counter += 1;
                    log::info!(
                        "PingHandler received pong #{}, responding with ping",
                        *counter
                    );

                    JsonRpcResponse {
                        id: request.id,
                        result: Some(json!("ping")),
                        error: None,
                    }
                }
                "init" => {
                    // 初期化リクエスト: 最初のpingを送信
                    log::info!("PingHandler initialized, starting ping sequence");
                    JsonRpcResponse {
                        id: request.id,
                        result: Some(json!("ping")),
                        error: None,
                    }
                }
                _ => JsonRpcResponse {
                    id: request.id,
                    result: None,
                    error: Some(crate::jsonrpc::message::JsonRpcError::method_not_found(
                        Some(format!("Method '{}' not found", request.method)),
                        Some(serde_json::json!({"method": request.method})),
                    )),
                },
            }
        }

        async fn on_notification(&mut self, notification: JsonRpcNotification) {
            log::info!(
                "PingHandler received notification: method={}",
                notification.method
            );
        }
    }

    struct PongHandler {
        counter: Arc<Mutex<u64>>,
    }

    impl PongHandler {
        fn new() -> Self {
            Self {
                counter: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait]
    impl JsonRpcHandler for PongHandler {
        async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
            log::info!("PongHandler received request: method={}", request.method);

            match request.method.as_str() {
                "ping" => {
                    // pingを受信したらカウンターをインクリメント
                    let mut counter = self.counter.lock().unwrap();
                    *counter += 1;
                    log::info!(
                        "PongHandler received ping #{}, responding with pong",
                        *counter
                    );

                    JsonRpcResponse {
                        id: request.id,
                        result: Some(json!("pong")),
                        error: None,
                    }
                }
                _ => JsonRpcResponse {
                    id: request.id,
                    result: None,
                    error: Some(crate::jsonrpc::message::JsonRpcError::method_not_found(
                        Some(format!("Method '{}' not found", request.method)),
                        Some(serde_json::json!({"method": request.method})),
                    )),
                },
            }
        }

        async fn on_notification(&mut self, notification: JsonRpcNotification) {
            log::info!(
                "PongHandler received notification: method={}",
                notification.method
            );
        }
    }

    #[tokio::test]
    async fn test_bidirectional_ping_pong_engines() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        log::info!("Starting bidirectional ping/pong test");

        // 双方向チャンネルセットアップ
        // Engine A -> Engine B
        let (tx_a_to_b, rx_a_to_b) = mpsc::unbounded_channel();
        // Engine B -> Engine A
        let (tx_b_to_a, rx_b_to_a) = mpsc::unbounded_channel();

        // ハンドラー作成
        let ping_handler = PingHandler::new();
        let ping_counter = ping_handler.counter.clone();
        let pong_handler = PongHandler::new();
        let pong_counter = pong_handler.counter.clone();

        // Engine A (PingHandler) - Engine Bからの応答を受信
        let engine_a: JsonRpcEngine<PingHandler> =
            JsonRpcEngine::new(rx_b_to_a, tx_a_to_b, ping_handler);

        // Engine B (PongHandler) - Engine Aからのリクエストを受信
        let engine_b: JsonRpcEngine<PongHandler> =
            JsonRpcEngine::new(rx_a_to_b, tx_b_to_a, pong_handler);

        // 少し待ってエンジンが起動することを確認
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        log::info!("Both engines initialized, starting ping sequence");

        // Engine Aに初期化リクエストを送信して最初のpingを開始
        let init_request = JsonRpcRequest {
            id: 1,
            method: "init".to_string(),
            params: None,
        };

        let init_response = engine_a.send_request(init_request).await.unwrap();
        assert_eq!(init_response.result, Some(json!("ping")));
        log::info!("Initial ping sent successfully");

        // 最初のping -> pong交換
        let ping_request = JsonRpcRequest {
            id: 2,
            method: "ping".to_string(),
            params: None,
        };

        let pong_response = engine_b.send_request(ping_request).await.unwrap();
        assert_eq!(pong_response.result, Some(json!("pong")));
        log::info!("First ping -> pong exchange completed");

        // pong -> ping交換
        let pong_request = JsonRpcRequest {
            id: 3,
            method: "pong".to_string(),
            params: None,
        };

        let ping_response = engine_a.send_request(pong_request).await.unwrap();
        assert_eq!(ping_response.result, Some(json!("ping")));
        log::info!("First pong -> ping exchange completed");

        // カウンターを確認
        assert_eq!(ping_counter.lock().unwrap().clone(), 1);
        assert_eq!(pong_counter.lock().unwrap().clone(), 1);

        // 複数回の交換をテスト
        for i in 4..8 {
            let ping_req = JsonRpcRequest {
                id: i,
                method: "ping".to_string(),
                params: None,
            };
            let pong_resp = engine_b.send_request(ping_req).await.unwrap();
            assert_eq!(pong_resp.result, Some(json!("pong")));

            let pong_req = JsonRpcRequest {
                id: i + 100,
                method: "pong".to_string(),
                params: None,
            };
            let ping_resp = engine_a.send_request(pong_req).await.unwrap();
            assert_eq!(ping_resp.result, Some(json!("ping")));
        }

        // 最終カウンター確認
        let final_ping_count = ping_counter.lock().unwrap().clone();
        let final_pong_count = pong_counter.lock().unwrap().clone();

        log::info!(
            "Final counts - ping: {}, pong: {}",
            final_ping_count,
            final_pong_count
        );

        assert_eq!(final_ping_count, 5); // init時の1回 + 4回の交換
        assert_eq!(final_pong_count, 5); // 最初の1回 + 4回の交換

        log::info!("Bidirectional ping/pong test completed successfully!");

        // クリーンアップ
        drop(engine_a);
        drop(engine_b);
    }

    #[tokio::test]
    async fn test_new_request_api() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<PingPongHandler> =
            JsonRpcEngine::new(engine_rx, tx, PingPongHandler);

        // 新しいrequestメソッドでpingを送信
        let response = engine
            .request("ping", None, 5000) // 5秒タイムアウト
            .await
            .unwrap();

        // レスポンスの検証
        assert_eq!(response.result, Some(json!("pong")));
        assert!(response.error.is_none());
        // IDが自動生成されていることを確認
        assert_eq!(response.id, 1); // 最初のリクエストなのでID=1

        // 2番目のリクエスト
        let response2 = engine
            .request("ping", Some(json!({"test": "data"})), 3000)
            .await
            .unwrap();

        assert_eq!(response2.result, Some(json!("pong")));
        assert_eq!(response2.id, 2); // 2番目のリクエストなのでID=2

        // エンジンをdropしてクリーンアップ
        drop(engine);
    }

    #[tokio::test]
    async fn test_request_timeout() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // タイムアウトをテストするためのハンドラー
        struct SlowHandler;

        #[async_trait]
        impl JsonRpcHandler for SlowHandler {
            async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
                log::info!("SlowHandler received request, sleeping...");
                // 2秒間スリープしてタイムアウトを発生させる
                tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("slow_response")),
                    error: None,
                }
            }

            async fn on_notification(&mut self, _notification: JsonRpcNotification) {
                // 何もしない
            }
        }

        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<SlowHandler> = JsonRpcEngine::new(engine_rx, tx, SlowHandler);

        // 1秒タイムアウトでリクエスト（2秒かかるのでタイムアウトするはず）
        let result = engine.request("slow_method", None, 1000).await;

        // タイムアウトエラーになることを確認
        assert!(matches!(result, Err(JsonRpcRequestError::ResponseTimeout)));

        // エンジンをdropしてクリーンアップ
        drop(engine);
    }

    #[tokio::test]
    async fn test_request_no_timeout() {
        // ログ初期化（テスト用）
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        let (tx, _rx) = mpsc::unbounded_channel();
        let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

        let engine: JsonRpcEngine<PingPongHandler> =
            JsonRpcEngine::new(engine_rx, tx, PingPongHandler);

        // タイムアウトなし（0を指定）でリクエスト
        let response = engine.request("ping", None, 0).await.unwrap();

        // レスポンスの検証
        assert_eq!(response.result, Some(json!("pong")));
        assert!(response.error.is_none());

        // エンジンをdropしてクリーンアップ
        drop(engine);
    }
}
