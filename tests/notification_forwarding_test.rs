/// 通知転送機能のテストモジュール
/// JsonRpcEngineがハンドラーからの通知を外部に転送するかをテストする

use async_trait::async_trait;
use fae::jsonrpc::handler::{JsonRpcHandler, JsonRpcSender};
use fae::jsonrpc::message::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, JsonRpcPayload};
use fae::jsonrpc::stdio::JsonRpcStdioAdapter;
use serde_json::json;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// 通知を生成するテスト用ハンドラー
#[derive(Clone)]
struct NotificationTestHandler {
    notification_count: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl NotificationTestHandler {
    fn new() -> Self {
        Self {
            notification_count: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }
    
    /// 通知を外部に送信する（現在は実装されていない機能をテスト）
    async fn send_test_notification(&self, message: &str) {
        // TODO: これが実際に外部に送信されるべき
        log::debug!("Attempting to send notification: {}", message);
        // 現在は何もしない - これが問題
    }
}

#[async_trait]
impl JsonRpcHandler for NotificationTestHandler {
    async fn on_request(
        &mut self, 
        request: JsonRpcRequest,
        _sender: &dyn JsonRpcSender,
    ) -> JsonRpcResponse {
        match request.method.as_str() {
            "test.triggerNotification" => {
                // 通知を送信を試みる
                self.send_test_notification("Hello from handler").await;
                
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("notification_triggered")),
                    error: None,
                }
            }
            _ => JsonRpcResponse {
                id: request.id,
                result: None,
                error: Some(fae::jsonrpc::message::JsonRpcError::method_not_found(
                    Some(format!("Method '{}' not found", request.method)),
                    None,
                )),
            },
        }
    }

    async fn on_notification(
        &mut self, 
        notification: JsonRpcNotification,
        _sender: &dyn JsonRpcSender,
    ) {
        match notification.method.as_str() {
            "test.echo" => {
                let message = notification
                    .params
                    .as_ref()
                    .and_then(|p| p.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("default");
                
                // エコー通知を送信を試みる
                self.send_test_notification(&format!("Echo: {}", message)).await;
            }
            _ => {
                log::debug!("Unknown notification: {}", notification.method);
            }
        }
    }
}

/// JsonRpcStdioAdapterを使った通知転送テスト（現在は失敗するはず）
#[tokio::test]
async fn test_notification_forwarding_current_implementation() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing notification forwarding with current implementation ===");
    
    // テストハンドラーを作成
    let handler = NotificationTestHandler::new();
    
    // JsonRpcStdioAdapterを作成
    let adapter = JsonRpcStdioAdapter::new(handler);
    
    // stdio経由でのテストプロセスを起動
    let mut child = Command::new("cargo")
        .args(&["test", "--bin", "notification_test_server", "--"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn test server");
    
    // 少し待ってからテスト
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // プロセスを終了
    child.kill().await.ok();
    let _ = child.wait().await;
    
    // このテストは現在は通知が送信されないことを確認するためのもの
    log::info!("Current implementation test completed (notifications not forwarded as expected)");
}

/// バイナリテスト用のサーバー（将来の成功テスト用）
#[tokio::test]
async fn test_manual_stdio_communication() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Manual stdio communication test ===");
    
    // 手動でのstdio通信をテスト
    // リクエストを送信して、レスポンスが返ってくることを確認
    
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "test.triggerNotification"
    });
    
    let json_str = serde_json::to_string(&request).unwrap();
    let message = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);
    
    log::debug!("Would send: {}", message);
    
    // 実際のstdio通信テストは別途実装
    log::info!("Manual test completed");
}

/// 期待される動作をテストする（修正後に成功するはず）
#[tokio::test]
#[ignore] // 修正前は失敗するのでignore
async fn test_notification_forwarding_expected_behavior() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    log::info!("=== Testing expected notification forwarding behavior ===");
    
    // このテストは修正後に成功するはず
    // 1. ハンドラーがエンジンに通知を送信
    // 2. エンジンが通知を外部（stdio）に転送
    // 3. 外部でその通知を受信できる
    
    // TODO: 修正後に実装
    log::info!("Expected behavior test - to be implemented after fix");
    
    // 現在は失敗することを明示
    panic!("This test should fail until notification forwarding is implemented");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    /// JsonRpcEngineの内部動作を直接テスト
    #[tokio::test]
    async fn test_engine_internal_notification_handling() {
        // ログ初期化
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        log::info!("=== Testing engine internal notification handling ===");
        
        // エンジンが通知をどう処理するかを直接テスト
        use tokio::sync::mpsc;
        use fae::jsonrpc::engine::JsonRpcEngine;
        
        let handler = NotificationTestHandler::new();
        let (stdio_to_engine_tx, stdio_to_engine_rx) = mpsc::unbounded_channel();
        let (engine_to_stdio_tx, mut engine_to_stdio_rx) = mpsc::unbounded_channel();
        
        let engine = JsonRpcEngine::new(stdio_to_engine_rx, engine_to_stdio_tx, handler);
        
        // 通知を送信
        let notification = JsonRpcNotification {
            method: "test.echo".to_string(),
            params: Some(json!({"message": "test"})),
        };
        
        stdio_to_engine_tx.send(JsonRpcPayload::Notification(notification)).unwrap();
        
        // 少し待機
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // engine_to_stdio_rxで通知を受信できるかチェック
        let result = timeout(Duration::from_millis(500), engine_to_stdio_rx.recv()).await;
        
        match result {
            Ok(Some(payload)) => {
                log::info!("Received payload from engine: {:?}", payload);
                // これが成功すれば通知転送が動作している
            }
            Ok(None) => {
                log::warn!("Engine output channel closed");
            }
            Err(_) => {
                log::warn!("Timeout waiting for notification from engine - this indicates the bug");
                // これが現在の状況（通知が転送されない）
            }
        }
        
        // エンジンをシャットダウン
        drop(engine);
        
        log::info!("Engine internal test completed");
    }
}