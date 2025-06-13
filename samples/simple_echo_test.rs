use fae::jsonrpc_base::{JsonRpcBase, MainLoopHandler, RpcResult, RpcError};
use fae::jsonrpc::{Request, ErrorObject, ErrorCode};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error};
use std::sync::Arc;

/// Simple echo test handler
struct EchoTestHandler {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

#[async_trait]
impl MainLoopHandler for EchoTestHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🚀 Echo test server started and ready!");
        info!("📋 Available methods:");
        info!("   - echo: Returns the payload");
        info!("   - ping: Returns 'pong'");
        info!("   - bye: Returns 'bye' and exits");
        info!("📢 Available notifications:");
        info!("   - poke: Sends ping request and expects pong response");
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        loop {
            tokio::select! {
                request = request_rx.recv() => {
                    if let Some(request) = request {
                        self.handle_request(&rpc_base, request).await;
                    }
                }
                
                notification = notification_rx.recv() => {
                    if let Some(notification) = notification {
                        self.handle_notification(&rpc_base, notification).await;
                    }
                }
                
                _ = &mut shutdown_rx => {
                    info!("🚪 Shutdown signal received, exiting main loop");
                    return Ok(false);
                }
            }
        }
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Simple echo test server connected");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Simple echo test server disconnected");
        Ok(())
    }
}

impl EchoTestHandler {
    /// Handle incoming requests
    async fn handle_request(&mut self, rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Handling request: {}", request.method);
        let id = request.id.clone().unwrap_or(Value::Null);
        let method = request.method.clone();
        
        let result = match request.method.as_str() {
            "echo" => {
                // Echo back the params
                info!("🔄 Echo request received");
                Ok(request.params.unwrap_or(Value::Null))
            }
            "ping" => {
                // Respond with pong
                info!("🏓 Ping request received - responding with pong");
                Ok(json!("pong"))
            }
            "bye" => {
                // Return bye and signal exit
                info!("👋 Bye request received - shutting down");
                Ok(json!("bye"))
            }
            _ => {
                debug!("❓ Unknown method: {}", request.method);
                Err(RpcError::MethodNotImplemented(request.method))
            }
        };
        
        // Send response
        match result {
            Ok(value) => {
                if let Err(e) = rpc_base.respond(id, value).await {
                    error!("❌ Failed to send response: {}", e);
                }
                
                // Send shutdown signal if this was a "bye" request
                if method == "bye" {
                    info!("🚪 Sending shutdown signal due to bye request");
                    // Give a small delay to ensure response is sent
                    let shutdown_tx = self.shutdown_tx.take();
                    tokio::spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if let Some(tx) = shutdown_tx {
                            let _ = tx.send(());
                        }
                    });
                }
            }
            Err(RpcError::MethodNotImplemented(_)) => {
                let error = ErrorObject::new(ErrorCode::MethodNotFound, None);
                if let Err(e) = rpc_base.respond_error(id, error).await {
                    error!("❌ Failed to send error response: {}", e);
                }
            }
            Err(e) => {
                let error = ErrorObject::custom(-32603, e.to_string(), None);
                if let Err(e) = rpc_base.respond_error(id, error).await {
                    error!("❌ Failed to send error response: {}", e);
                }
            }
        }
    }
    
    /// Handle incoming notifications
    async fn handle_notification(&self, rpc_base: &JsonRpcBase, notification: Request) {
        debug!("📢 Handling notification: {}", notification.method);
        
        match notification.method.as_str() {
            "poke" => {
                info!("👆 Poke notification received - sending ping to client");
                
                // Send ping request to client
                match rpc_base.request("ping", None).await {
                    Ok(response) => {
                        if response == json!("pong") {
                            info!("🏓 pingpong");
                        } else {
                            info!("🤔 Unexpected ping response: {}", response);
                        }
                    }
                    Err(e) => {
                        error!("❌ Ping request failed: {}", e);
                    }
                }
            }
            _ => {
                debug!("❓ Unknown notification: {}", notification.method);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🧪 Simple JSON-RPC echo test server starting");
    
    // Create JsonRpcBase for stdio communication
    let rpc_base = JsonRpcBase::new_stdio().await?;
    
    // Create main loop handler
    let main_handler = EchoTestHandler {
        shutdown_tx: None,
    };
    
    // Run the main event loop
    match rpc_base.run_main_loop(Box::new(main_handler)).await {
        Ok(()) => {
            info!("✅ Echo test server stopped gracefully");
        }
        Err(e) => {
            error!("❌ Echo test server error: {}", e);
            return Err(e.into());
        }
    }
    
    info!("🎉 Simple echo test server shutdown complete");
    Ok(())
}