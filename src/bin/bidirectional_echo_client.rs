use fae::jsonrpc_base::{JsonRpcBase, RequestHandler, NotificationHandler, MainLoopHandler, RpcResult, RpcError};
use fae::jsonrpc::Request;
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{debug, info, warn};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, interval};

/// Client request handler - handles server-initiated requests
struct ClientRequestHandler;

#[async_trait]
impl RequestHandler for ClientRequestHandler {
    async fn handle_request(&self, request: Request) -> RpcResult<Value> {
        debug!("Client handling request: {}", request.method);
        
        match request.method.as_str() {
            "get_client_info" => {
                // Server asking for client info
                Ok(json!({
                    "client_name": "bidirectional_echo_client",
                    "version": "1.0.0",
                    "capabilities": ["handle_server_requests", "send_notifications"]
                }))
            }
            "shutdown" => {
                // Server requesting shutdown
                info!("🛑 Server requested shutdown");
                Ok(json!("acknowledged"))
            }
            _ => {
                Err(RpcError::MethodNotImplemented(request.method))
            }
        }
    }
}

/// Client notification handler - handles server-initiated notifications
struct ClientNotificationHandler;

#[async_trait]
impl NotificationHandler for ClientNotificationHandler {
    async fn handle_notification(&self, notification: Request) -> RpcResult<()> {
        match notification.method.as_str() {
            "server_status" => {
                if let Some(params) = notification.params {
                    info!("📊 Server status update: {}", params);
                }
            }
            "log" => {
                if let Some(params) = notification.params {
                    info!("📝 Server log: {}", params);
                }
            }
            _ => {
                debug!("Unknown notification from server: {}", notification.method);
            }
        }
        Ok(())
    }
}

/// Client main loop handler
struct ClientMainLoopHandler {
    rpc_base: Option<Arc<JsonRpcBase>>,
    test_phase: u32,
    tick_count: u32,
}

impl ClientMainLoopHandler {
    fn new() -> Self {
        Self {
            rpc_base: None,
            test_phase: 0,
            tick_count: 0,
        }
    }
    
    fn set_rpc_base(&mut self, rpc_base: Arc<JsonRpcBase>) {
        self.rpc_base = Some(rpc_base);
    }
}

#[async_trait]
impl MainLoopHandler for ClientMainLoopHandler {
    async fn on_tick(&mut self) -> RpcResult<bool> {
        self.tick_count += 1;
        
        if let Some(rpc_base) = &self.rpc_base {
            // Run different test phases
            match self.test_phase {
                0..=50 => {
                    // Phase 1: Basic requests every 10 ticks
                    if self.tick_count % 10 == 0 {
                        self.run_basic_requests(rpc_base).await?;
                    }
                }
                51..=100 => {
                    // Phase 2: Notifications every 5 ticks
                    if self.tick_count % 5 == 0 {
                        self.send_notifications(rpc_base).await?;
                    }
                }
                101..=150 => {
                    // Phase 3: Mixed operations
                    if self.tick_count % 8 == 0 {
                        self.run_mixed_operations(rpc_base).await?;
                    }
                }
                _ => {
                    // Phase 4: Cleanup and exit
                    info!("🎯 Test phases completed, shutting down");
                    return Ok(false);
                }
            }
            
            if self.tick_count % 50 == 0 {
                self.test_phase += 1;
                info!("📈 Entering test phase {}", self.test_phase);
            }
        }
        
        Ok(true)
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Bidirectional echo client connected!");
        info!("🧪 Starting comprehensive bidirectional RPC test");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Bidirectional echo client disconnected");
        Ok(())
    }
}

impl ClientMainLoopHandler {
    async fn run_basic_requests(&self, rpc_base: &JsonRpcBase) -> RpcResult<()> {
        debug!("🔬 Running basic requests");
        
        // Test echo
        match rpc_base.request("echo", Some(json!({"test": "basic_echo"}))).await {
            Ok(result) => info!("✅ Echo result: {}", result),
            Err(e) => warn!("❌ Echo failed: {}", e),
        }
        
        // Test ping
        match rpc_base.request("ping", None).await {
            Ok(result) => info!("✅ Ping result: {}", result),
            Err(e) => warn!("❌ Ping failed: {}", e),
        }
        
        // Test add
        match rpc_base.request("add", Some(json!([self.tick_count, 10]))).await {
            Ok(result) => info!("✅ Add result: {}", result),
            Err(e) => warn!("❌ Add failed: {}", e),
        }
        
        Ok(())
    }
    
    async fn send_notifications(&self, rpc_base: &JsonRpcBase) -> RpcResult<()> {
        debug!("📢 Sending notifications");
        
        let status = json!({
            "client_tick": self.tick_count,
            "phase": "notifications",
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        rpc_base.notify("status", Some(status)).await?;
        
        let log_message = json!({
            "level": "info",
            "message": format!("Client notification #{}", self.tick_count),
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        rpc_base.notify("log", Some(log_message)).await?;
        
        Ok(())
    }
    
    async fn run_mixed_operations(&self, rpc_base: &JsonRpcBase) -> RpcResult<()> {
        debug!("🔀 Running mixed operations");
        
        // Server info request
        match rpc_base.request("info", None).await {
            Ok(result) => info!("📋 Server info: {}", result),
            Err(e) => warn!("❌ Info request failed: {}", e),
        }
        
        // Concurrent requests
        let futures = vec![
            rpc_base.request("echo", Some(json!({"concurrent": 1}))),
            rpc_base.request("echo", Some(json!({"concurrent": 2}))),
            rpc_base.request("add", Some(json!([100, 200]))),
        ];
        
        let results = futures_util::future::join_all(futures).await;
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(value) => info!("✅ Concurrent request {}: {}", i + 1, value),
                Err(e) => warn!("❌ Concurrent request {} failed: {}", i + 1, e),
            }
        }
        
        // Send a notification
        let mixed_notification = json!({
            "type": "mixed_phase",
            "data": format!("Mixed operation #{}", self.tick_count)
        });
        
        rpc_base.notify("log", Some(mixed_notification)).await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🚀 Starting bidirectional echo client");
    
    // Get the path to the bidirectional echo server binary
    let current_exe = env::current_exe()?;
    let server_path = current_exe.parent()
        .unwrap()
        .join("bidirectional_echo_server");
    
    // Create JsonRpcBase by spawning the server
    let rpc_base = JsonRpcBase::spawn(
        server_path.to_str().unwrap(),
        &["--test"] // Run server in test mode
    )
    .await?
    .with_request_handler(Arc::new(ClientRequestHandler))
    .with_notification_handler(Arc::new(ClientNotificationHandler));
    
    let rpc_base = Arc::new(rpc_base);
    
    // Create main loop handler
    let mut main_handler = ClientMainLoopHandler::new();
    main_handler.set_rpc_base(rpc_base.clone());
    
    // Run the main event loop
    info!("🔄 Starting bidirectional client test loop");
    match rpc_base.run_main_loop(Box::new(main_handler)).await {
        Ok(()) => {
            info!("✅ Client test completed successfully");
        }
        Err(e) => {
            warn!("❌ Client test error: {}", e);
        }
    }
    
    // Shutdown
    info!("🛑 Shutting down client");
    Arc::try_unwrap(rpc_base)
        .map_err(|_| "Failed to unwrap Arc")?
        .shutdown()
        .await?;
    
    info!("🎉 Bidirectional echo client shutdown complete");
    Ok(())
}