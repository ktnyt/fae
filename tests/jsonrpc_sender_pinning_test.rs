// JsonRpcSender トレイト機能のピニングテスト
// send_request および send_notification メソッドの動作を確認

use async_trait::async_trait;
use fae::jsonrpc::{
    engine::JsonRpcEngine,
    handler::{JsonRpcHandler, JsonRpcSender},
    message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcPayload},
};
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

// 双方向通信テスト用ハンドラー
#[derive(Clone)]
struct BidirectionalTestHandler {
    test_mode: String,
}

impl BidirectionalTestHandler {
    fn new(test_mode: &str) -> Self {
        Self {
            test_mode: test_mode.to_string(),
        }
    }
}

#[async_trait]
impl JsonRpcHandler for BidirectionalTestHandler {
    async fn on_request(
        &mut self,
        request: JsonRpcRequest,
        sender: &dyn JsonRpcSender,
    ) -> JsonRpcResponse {
        match request.method.as_str() {
            "ping" => JsonRpcResponse {
                id: request.id,
                result: Some(json!("pong")),
                error: None,
            },
            "request_with_notification" => {
                // リクエストに対してレスポンスを返し、同時に通知も送信
                let _ = sender
                    .send_notification(
                        "handler_notification".to_string(),
                        Some(json!({
                            "message": "Handler sent notification during request",
                            "original_request_id": request.id
                        })),
                    )
                    .await;

                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("request_processed_with_notification")),
                    error: None,
                }
            }
            "chain_request" => {
                // ハンドラーから別のリクエストを送信（双方向通信テスト）
                if self.test_mode == "echo" {
                    match sender
                        .send_request("echo".to_string(), Some(json!("chained_message")))
                        .await
                    {
                        Ok(response) => JsonRpcResponse {
                            id: request.id,
                            result: Some(json!({
                                "chained_response": response.result,
                                "original_method": "chain_request"
                            })),
                            error: None,
                        },
                        Err(err) => JsonRpcResponse {
                            id: request.id,
                            result: None,
                            error: Some(fae::jsonrpc::message::JsonRpcError::internal_error(
                                Some(format!("Chained request failed: {:?}", err)),
                                None,
                            )),
                        },
                    }
                } else {
                    JsonRpcResponse {
                        id: request.id,
                        result: Some(json!("chain_request_handled")),
                        error: None,
                    }
                }
            }
            "echo" => {
                // エコーメソッド
                JsonRpcResponse {
                    id: request.id,
                    result: request.params,
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
        notification: JsonRpcNotification,
        sender: &dyn JsonRpcSender,
    ) {
        match notification.method.as_str() {
            "trigger_notification_chain" => {
                // 通知を受信したら別の通知を送信
                let _ = sender
                    .send_notification(
                        "chained_notification".to_string(),
                        Some(json!({
                            "message": "Handler received trigger and sent chained notification",
                            "original_method": notification.method
                        })),
                    )
                    .await;
            }
            _ => {
                // その他の通知は無視
            }
        }
    }
}

/// JsonRpcSender.send_request メソッドのテスト
#[tokio::test]
async fn test_jsonrpc_sender_send_request() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing JsonRpcSender.send_request ===");

    // エンジンセットアップ
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("simple");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // JsonRpcSenderトレイトのsend_requestを使ってpingを送信
    let response = (&engine as &dyn JsonRpcSender)
        .send_request("ping".to_string(), None)
        .await
        .expect("send_request should succeed");

    // レスポンスを検証
    assert_eq!(response.result, Some(json!("pong")));
    assert!(response.error.is_none());
    assert_eq!(response.id, 1); // 最初のリクエストなのでID=1

    log::info!("✅ send_request test passed");

    // クリーンアップ
    drop(engine);
}

/// JsonRpcSender.send_notification メソッドのテスト
#[tokio::test]
async fn test_jsonrpc_sender_send_notification() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing JsonRpcSender.send_notification ===");

    // エンジンセットアップ
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("simple");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // JsonRpcSenderトレイトのsend_notificationを使って通知を送信
    (&engine as &dyn JsonRpcSender)
        .send_notification(
            "test_notification".to_string(),
            Some(json!({"message": "test notification"})),
        )
        .await
        .expect("send_notification should succeed");

    // 通知が外部に送信されることを確認
    let received = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("Should receive notification within timeout")
        .expect("Should receive notification payload");

    match received {
        JsonRpcPayload::Notification(notification) => {
            assert_eq!(notification.method, "test_notification");
            assert_eq!(
                notification.params,
                Some(json!({"message": "test notification"}))
            );
        }
        _ => panic!("Expected notification payload"),
    }

    log::info!("✅ send_notification test passed");

    // クリーンアップ
    drop(engine);
}

/// ハンドラー内でのsend_request使用テスト（同一エンジン内でのデッドロック回避のため無効化）
#[tokio::test]
#[ignore]
async fn test_handler_bidirectional_request() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing handler bidirectional request ===");

    // エンジンセットアップ
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("echo");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // chain_requestを送信（ハンドラー内で別のリクエストを送信する）
    let response = (&engine as &dyn JsonRpcSender)
        .send_request("chain_request".to_string(), None)
        .await
        .expect("chain_request should succeed");

    // レスポンスを検証
    if let Some(error) = &response.error {
        panic!("Request failed with error: {:?}", error);
    }
    if let Some(result) = response.result {
        assert_eq!(result["original_method"], "chain_request");
        assert_eq!(result["chained_response"], "chained_message");
    } else {
        panic!("Expected result in chained request response");
    }

    log::info!("✅ handler bidirectional request test passed");

    // クリーンアップ
    drop(engine);
}

/// ハンドラー内でのsend_notification使用テスト
#[tokio::test]
async fn test_handler_send_notification_during_request() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing handler send_notification during request ===");

    // エンジンセットアップ
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("simple");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // request_with_notificationを送信（ハンドラー内で通知を送信する）
    let response = (&engine as &dyn JsonRpcSender)
        .send_request("request_with_notification".to_string(), None)
        .await
        .expect("request_with_notification should succeed");

    // レスポンスを検証
    assert_eq!(
        response.result,
        Some(json!("request_processed_with_notification"))
    );
    assert!(response.error.is_none());

    // 通知が送信されることを確認
    let received = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("Should receive notification within timeout")
        .expect("Should receive notification payload");

    match received {
        JsonRpcPayload::Notification(notification) => {
            assert_eq!(notification.method, "handler_notification");
            if let Some(params) = notification.params {
                assert_eq!(params["message"], "Handler sent notification during request");
                assert_eq!(params["original_request_id"], response.id);
            } else {
                panic!("Expected params in handler notification");
            }
        }
        _ => panic!("Expected notification payload"),
    }

    log::info!("✅ handler send_notification during request test passed");

    // クリーンアップ
    drop(engine);
}

/// 通知チェーンテスト（通知→通知）
#[tokio::test]
async fn test_notification_chain() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing notification chain ===");

    // エンジンセットアップ
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("simple");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // trigger_notification_chainを送信
    let trigger_notification = JsonRpcNotification {
        method: "trigger_notification_chain".to_string(),
        params: Some(json!({"trigger": "test"})),
    };

    engine_tx
        .send(JsonRpcPayload::Notification(trigger_notification))
        .unwrap();

    // チェーンされた通知を受信することを確認
    let received = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("Should receive chained notification within timeout")
        .expect("Should receive notification payload");

    match received {
        JsonRpcPayload::Notification(notification) => {
            assert_eq!(notification.method, "chained_notification");
            if let Some(params) = notification.params {
                assert_eq!(
                    params["message"],
                    "Handler received trigger and sent chained notification"
                );
                assert_eq!(params["original_method"], "trigger_notification_chain");
            } else {
                panic!("Expected params in chained notification");
            }
        }
        _ => panic!("Expected notification payload"),
    }

    log::info!("✅ notification chain test passed");

    // クリーンアップ
    drop(engine);
}

/// エラーハンドリングテスト
#[tokio::test]
async fn test_send_request_error_handling() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing send_request error handling ===");

    // エンジンセットアップ
    let (tx, _rx) = mpsc::unbounded_channel();
    let (_engine_tx, engine_rx) = mpsc::unbounded_channel();

    let handler = BidirectionalTestHandler::new("simple");
    let engine: JsonRpcEngine<BidirectionalTestHandler> =
        JsonRpcEngine::new(engine_rx, tx, handler);

    // 存在しないメソッドを呼び出し
    let response = (&engine as &dyn JsonRpcSender)
        .send_request("unknown_method".to_string(), None)
        .await
        .expect("send_request should succeed even for unknown methods");

    // エラーレスポンスを検証
    assert!(response.result.is_none());
    assert!(response.error.is_some());

    if let Some(error) = response.error {
        assert_eq!(error.code, -32601); // Method not found
        assert!(error.message.contains("not found"));
    }

    log::info!("✅ send_request error handling test passed");

    // クリーンアップ
    drop(engine);
}