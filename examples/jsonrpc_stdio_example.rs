use async_trait::async_trait;
use fae::jsonrpc::{
    handler::JsonRpcHandler,
    message::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse},
    stdio::JsonRpcStdioAdapter,
};
use serde_json::json;

/// Echo サーバーのハンドラー
struct EchoHandler {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EchoHandler {
    fn new(shutdown_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            shutdown_tx: Some(shutdown_tx),
        }
    }
}

#[async_trait]
impl JsonRpcHandler for EchoHandler {
    async fn on_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        log::info!(
            "Received request: method={}, id={}",
            request.method,
            request.id
        );

        match request.method.as_str() {
            "ping" => {
                log::info!("Responding with pong");
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("pong")),
                    error: None,
                }
            }
            "echo" => {
                log::info!("Echoing params: {:?}", request.params);
                JsonRpcResponse {
                    id: request.id,
                    result: request.params,
                    error: None,
                }
            }
            "reverse" => {
                if let Some(params) = request.params {
                    if let Some(text) = params.as_str() {
                        let reversed: String = text.chars().rev().collect();
                        JsonRpcResponse {
                            id: request.id,
                            result: Some(json!(reversed)),
                            error: None,
                        }
                    } else {
                        JsonRpcResponse {
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError::invalid_params(
                                Some("Parameter must be a string"),
                                None,
                            )),
                        }
                    }
                } else {
                    JsonRpcResponse {
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError::invalid_params(
                            Some("Parameter required"),
                            None,
                        )),
                    }
                }
            }
            "shutdown" => {
                log::info!("Shutdown request received, terminating server");
                if let Some(tx) = self.shutdown_tx.take() {
                    let _ = tx.send(());
                }
                JsonRpcResponse {
                    id: request.id,
                    result: Some(json!("Server shutting down")),
                    error: None,
                }
            }
            _ => {
                log::warn!("Unknown method: {}", request.method);
                JsonRpcResponse {
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError::method_not_found(
                        Some(format!("Method '{}' not found", request.method)),
                        Some(json!({"method": request.method})),
                    )),
                }
            }
        }
    }

    async fn on_notification(&mut self, notification: JsonRpcNotification) {
        log::info!("Received notification: method={}", notification.method);

        match notification.method.as_str() {
            "log" => {
                if let Some(params) = notification.params {
                    println!("LOG: {}", params);
                }
            }
            _ => {
                log::info!("Ignoring unknown notification: {}", notification.method);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ログ初期化（デバッグレベルに設定）
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    log::info!("Starting JSON-RPC stdio server...");
    log::info!("Supported methods: ping, echo, reverse, shutdown");
    log::info!("Supported notifications: log");

    // シャットダウン用チャンネル
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    // エコーハンドラーを作成
    let handler = EchoHandler::new(shutdown_tx);

    // stdio アダプターを作成
    let adapter = JsonRpcStdioAdapter::new(handler);

    log::info!("JSON-RPC server running. Send requests via stdin.");

    // アダプターとシャットダウンシグナルを並行実行
    tokio::select! {
        result = adapter.run() => {
            if let Err(e) = result {
                log::error!("Adapter error: {}", e);
            }
        }
        _ = &mut shutdown_rx => {
            log::info!("Shutdown signal received, stopping server");
        }
    }

    log::info!("JSON-RPC server shutting down.");
    Ok(())
}
