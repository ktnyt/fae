use fae::jsonrpc_base::{JsonRpcBase, RequestHandler, NotificationHandler, MainLoopHandler, RpcResult, RpcError};
use fae::jsonrpc::{Request, ErrorObject, ErrorCode};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{debug, info, warn, error};
use std::sync::Arc;

/// Echo request handler - handles incoming requests
struct EchoRequestHandler;

#[async_trait]
impl RequestHandler for EchoRequestHandler {
    async fn handle_request(&self, request: Request) -> RpcResult<Value> {
        debug!("Handling request: {}", request.method);
        
        match request.method.as_str() {
            "echo" => {
                // Echo back the params
                Ok(request.params.unwrap_or(Value::Null))
            }
            "ping" => {
                // Simple ping-pong
                Ok(json!("pong"))
            }
            "add" => {
                // Add two numbers
                match request.params {
                    Some(Value::Array(ref arr)) if arr.len() == 2 => {
                        let a = arr[0].as_f64().ok_or_else(|| {
                            RpcError::Rpc {
                                code: ErrorCode::InvalidParams as i32,
                                message: "First parameter must be a number".to_string(),
                            }
                        })?;
                        let b = arr[1].as_f64().ok_or_else(|| {
                            RpcError::Rpc {
                                code: ErrorCode::InvalidParams as i32,
                                message: "Second parameter must be a number".to_string(),
                            }
                        })?;
                        Ok(json!(a + b))
                    }
                    Some(Value::Object(ref obj)) => {
                        let a = obj.get("a").and_then(|v| v.as_f64()).ok_or_else(|| {
                            RpcError::Rpc {
                                code: ErrorCode::InvalidParams as i32,
                                message: "Missing or invalid 'a' parameter".to_string(),
                            }
                        })?;
                        let b = obj.get("b").and_then(|v| v.as_f64()).ok_or_else(|| {
                            RpcError::Rpc {
                                code: ErrorCode::InvalidParams as i32,
                                message: "Missing or invalid 'b' parameter".to_string(),
                            }
                        })?;
                        Ok(json!(a + b))
                    }
                    _ => Err(RpcError::Rpc {
                        code: ErrorCode::InvalidParams as i32,
                        message: "Parameters must be [a, b] or {\"a\": number, \"b\": number}".to_string(),
                    })
                }
            }
            "info" => {
                // Return server info
                let info = json!({
                    "name": "bidirectional_echo_server",
                    "version": "1.0.0",
                    "methods": ["echo", "ping", "add", "info"],
                    "capabilities": ["bidirectional_rpc", "lsp_framing"],
                    "description": "Bidirectional JSON-RPC server that can also act as a client"
                });
                Ok(info)
            }
            _ => {
                // Method not found
                Err(RpcError::MethodNotImplemented(request.method))
            }
        }
    }
}

/// Log notification handler - handles incoming notifications
struct LogNotificationHandler;

#[async_trait]
impl NotificationHandler for LogNotificationHandler {
    async fn handle_notification(&self, notification: Request) -> RpcResult<()> {
        info!("📢 Notification received: {} - {:?}", notification.method, notification.params);
        
        match notification.method.as_str() {
            "log" => {
                if let Some(params) = notification.params {
                    info!("📝 Log: {}", params);
                }
            }
            "status" => {
                if let Some(params) = notification.params {
                    info!("📊 Status: {}", params);
                }
            }
            _ => {
                debug!("Unknown notification method: {}", notification.method);
            }
        }
        
        Ok(())
    }
}

/// Main loop handler - controls the server lifecycle
struct ServerMainLoopHandler {
    tick_count: u32,
    rpc_base: Option<Arc<JsonRpcBase>>,
    max_ticks: u32,
}

impl ServerMainLoopHandler {
    fn new(max_ticks: u32) -> Self {
        Self {
            tick_count: 0,
            rpc_base: None,
            max_ticks,
        }
    }
    
    fn set_rpc_base(&mut self, rpc_base: Arc<JsonRpcBase>) {
        self.rpc_base = Some(rpc_base);
    }
}

#[async_trait]
impl MainLoopHandler for ServerMainLoopHandler {
    async fn on_tick(&mut self) -> RpcResult<bool> {
        self.tick_count += 1;
        
        if self.tick_count % 50 == 0 {
            debug!("💓 Server heartbeat: tick #{}", self.tick_count);
        }
        
        // Demonstrate client functionality: send a notification every 100 ticks
        if self.tick_count % 100 == 0 {
            if let Some(rpc_base) = &self.rpc_base {
                let status = json!({
                    "tick": self.tick_count,
                    "status": "running",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                
                debug!("📤 Sending status notification");
                if let Err(e) = rpc_base.notify("server_status", Some(status)).await {
                    warn!("Failed to send status notification: {}", e);
                }
            }
        }
        
        // Stop after max_ticks (for testing)
        if self.max_ticks > 0 && self.tick_count >= self.max_ticks {
            info!("🛑 Reached max ticks ({}), stopping server", self.max_ticks);
            return Ok(false);
        }
        
        Ok(true)
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Bidirectional echo server connected and ready!");
        info!("🎯 Available methods: echo, ping, add, info");
        info!("📢 Accepts notifications: log, status");
        info!("🔄 Demonstrates bidirectional RPC capabilities");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Bidirectional echo server disconnected");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🚀 Starting bidirectional echo server with JSON-RPC over stdio");
    
    // Parse command line arguments for test mode
    let args: Vec<String> = std::env::args().collect();
    let max_ticks = if args.len() > 1 && args[1] == "--test" {
        500 // Run for limited time in test mode
    } else {
        0 // Run indefinitely in normal mode
    };
    
    // Create JsonRpcBase for stdio communication
    let rpc_base = JsonRpcBase::new_stdio()
        .await?
        .with_request_handler(Arc::new(EchoRequestHandler))
        .with_notification_handler(Arc::new(LogNotificationHandler));
    
    let rpc_base = Arc::new(rpc_base);
    
    // Create main loop handler
    let mut main_handler = ServerMainLoopHandler::new(max_ticks);
    main_handler.set_rpc_base(rpc_base.clone());
    
    // Run the main event loop
    info!("📡 Starting main event loop");
    match rpc_base.run_main_loop(Box::new(main_handler)).await {
        Ok(()) => {
            info!("✅ Server stopped gracefully");
        }
        Err(e) => {
            error!("❌ Server error: {}", e);
            return Err(e.into());
        }
    }
    
    // Shutdown
    Arc::try_unwrap(rpc_base)
        .map_err(|_| "Failed to unwrap Arc")?
        .shutdown()
        .await?;
    
    info!("🎉 Bidirectional echo server shutdown complete");
    Ok(())
}