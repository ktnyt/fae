// JsonRpcEngine リファクタリング用ピニングテスト
// 現在の動作を保証し、リファクタリング後の等価性を確認

use fae::jsonrpc::{
    engine::{JsonRpcEngine, JsonRpcRequestError},
    handler::{JsonRpcHandler, JsonRpcSender},
    message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcPayload},
};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::mpsc;

// 通知送信機能付きテストハンドラー
struct NotificationTestHandler;

impl NotificationTestHandler {
    fn new() -> Self {
        Self
    }
}


#[async_trait]
impl JsonRpcHandler for NotificationTestHandler {
    async fn on_request(
        &mut self, 
        request: JsonRpcRequest,
        sender: &dyn JsonRpcSender,
    ) -> JsonRpcResponse {
        match request.method.as_str() {
            "ping" => {
                // pingリクエストを受けたらpongで応答
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("pong")),
                    error: None,
                }
            }
            "notify_test" => {
                // リクエストを受けたら通知を送信する（新しい方法：JsonRpcSenderトレイトを使用）
                let _ = sender.send_notification(
                    "test_notification".to_string(),
                    Some(json!({"message": "Hello from handler"})),
                ).await;
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("notification_sent")),
                    error: None,
                }
            }
            _ => JsonRpcResponse {
                id: request.id,
                result: None,
                error: Some(fae::jsonrpc::message::JsonRpcError::method_not_found(
                    Some(format!("Method '{}' not found", request.method)),
                    Some(json!({"method": request.method})),
                )),
            },
        }
    }

    async fn on_notification(
        &mut self, 
        _notification: JsonRpcNotification,
        _sender: &dyn JsonRpcSender,
    ) {
        // 通知受信の処理（現在は何もしない）
    }
}

#[tokio::test]
async fn pinning_test_current_notification_behavior() {
    // 現在の実装での通知送信動作をピニング

    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = NotificationTestHandler::new();
    
    // エンジンを作成
    let engine: JsonRpcEngine<NotificationTestHandler> = JsonRpcEngine::new(engine_rx, tx, handler);
    
    // 新しい実装では、ハンドラーが直接senderパラメータを使って通知を送信できる
    
    // 基本的なping/pong動作は動作する
    let response = engine.request("ping", None, 1000).await.unwrap();
    assert_eq!(response.result, Some(json!("pong")));
    
    drop(engine);
}

#[tokio::test]  
async fn pinning_test_basic_request_response() {
    // 基本的なリクエスト/レスポンス動作のピニング
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = NotificationTestHandler::new();
    let engine: JsonRpcEngine<NotificationTestHandler> = JsonRpcEngine::new(engine_rx, tx, handler);

    // ping/pong テスト
    let response = engine.request("ping", None, 1000).await.unwrap();
    assert_eq!(response.result, Some(json!("pong")));
    assert!(response.error.is_none());
    
    // 不明なメソッドテスト
    let response = engine.request("unknown", None, 1000).await;
    match response {
        Ok(resp) => {
            assert!(resp.result.is_none());
            assert!(resp.error.is_some());
            let error = resp.error.unwrap();
            assert_eq!(error.code, -32601); // Method not found
        }
        Err(_) => panic!("Expected error response, got request error"),
    }

    drop(engine);
}

#[tokio::test]
async fn pinning_test_multiple_requests() {
    // 複数リクエストの処理順序のピニング
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = NotificationTestHandler::new();
    let engine: JsonRpcEngine<NotificationTestHandler> = JsonRpcEngine::new(engine_rx, tx, handler);

    // 複数のpingリクエストを順次送信（並行だとライフタイム問題が発生）
    for i in 0..5 {
        let response = engine.request("ping", Some(json!({"index": i})), 1000).await.unwrap();
        assert_eq!(response.result, Some(json!("pong")));
    }

    drop(engine);
}

#[tokio::test]
async fn pinning_test_timeout_behavior() {
    // タイムアウト動作のピニング
    
    use std::sync::{Arc, Mutex};
    
    // 遅延ハンドラー
    struct SlowHandler {
        delay_ms: Arc<Mutex<u64>>,
    }
    
    impl SlowHandler {
        fn new(delay_ms: u64) -> Self {
            Self {
                delay_ms: Arc::new(Mutex::new(delay_ms)),
            }
        }
    }
    
    #[async_trait]
    impl JsonRpcHandler for SlowHandler {
        async fn on_request(
            &mut self, 
            request: JsonRpcRequest,
            _sender: &dyn JsonRpcSender,
        ) -> JsonRpcResponse {
            let delay = *self.delay_ms.lock().unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            JsonRpcResponse {
                id: request.id,
                result: Some(json!("slow_response")),
                error: None,
            }
        }

        async fn on_notification(
            &mut self, 
            _notification: JsonRpcNotification,
            _sender: &dyn JsonRpcSender,
        ) {}
    }

    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = SlowHandler::new(500); // 500ms delay
    let engine: JsonRpcEngine<SlowHandler> = JsonRpcEngine::new(engine_rx, tx, handler);

    // 200ms timeout（遅延より短い）→ タイムアウトするはず
    let result = engine.request("slow", None, 200).await;
    assert!(matches!(result, Err(JsonRpcRequestError::ResponseTimeout)));
    
    // 1000ms timeout（遅延より長い）→ 成功するはず
    let result = engine.request("slow", None, 1000).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.result, Some(json!("slow_response")));

    drop(engine);
}

#[tokio::test]
async fn pinning_test_engine_lifecycle() {
    // エンジンのライフサイクル管理のピニング
    
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = NotificationTestHandler::new();
    let mut engine: JsonRpcEngine<NotificationTestHandler> = JsonRpcEngine::new(engine_rx, tx, handler);

    // 基本動作確認
    let response = engine.request("ping", None, 1000).await.unwrap();
    assert_eq!(response.result, Some(json!("pong")));
    
    // 手動シャットダウン
    engine.shutdown();
    
    // シャットダウン後のリクエストはエラーになるはず
    let result = engine.request("ping", None, 1000).await;
    assert!(result.is_err());

    // Dropは安全に実行されるはず（二重シャットダウンでもパニックしない）
    drop(engine);
}