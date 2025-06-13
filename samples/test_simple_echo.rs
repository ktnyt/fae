use fae::jsonrpc_base::{JsonRpcBase, MainLoopHandler, RpcResult, RpcError};
use fae::jsonrpc::{Request, ErrorObject, ErrorCode};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error};
use std::env;
use std::sync::Arc;
use std::process::Stdio;
use tokio::time::Duration;
use tokio::process::Command;

/// Test client handler
struct TestClientHandler {
    test_phase: u32,
    tick_count: u32,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    child_pid: Option<u32>,
}

impl TestClientHandler {
    fn new() -> Self {
        Self {
            test_phase: 0,
            tick_count: 0,
            shutdown_tx: None,
            child_pid: None,
        }
    }
    
    fn set_child_pid(&mut self, pid: u32) {
        self.child_pid = Some(pid);
    }
    
    fn kill_child_if_needed(&self) {
        if let Some(pid) = self.child_pid {
            info!("🔪 Killing child process {} due to timeout", pid);
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
            #[cfg(windows)]
            {
                // On Windows, we could use taskkill but for simplicity just log
                info!("⚠️ Process termination not implemented on Windows");
            }
        }
    }
}

#[async_trait]
impl MainLoopHandler for TestClientHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🧪 Test client started");
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        let tick_interval = Duration::from_millis(200); // Slower for testing
        let mut tick_timer = tokio::time::interval(tick_interval);
        
        loop {
            tokio::select! {
                _ = tick_timer.tick() => {
                    self.tick_count += 1;
                    
                    match self.test_phase {
                        0 => {
                            // Phase 0: Echo test
                            info!("📤 Testing echo request");
                            match rpc_base.request_timeout("echo", Some(json!({"message": "Hello World!"})), Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 Echo response: {}", result);
                                    self.test_phase = 1;
                                }
                                Err(e) => {
                                    error!("❌ Echo failed: {}", e);
                                    self.kill_child_if_needed();
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                        }
                        1 => {
                            // Phase 1: Poke notification test
                            info!("📤 Testing poke notification");
                            if let Err(e) = rpc_base.notify("poke", Some(json!({"from": "client"}))).await {
                                error!("❌ Poke notification failed: {}", e);
                            }
                            self.test_phase = 2;
                        }
                        2 => {
                            // Phase 2: Ping test
                            info!("📤 Testing ping request");
                            match rpc_base.request_timeout("ping", None, Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 Ping response: {}", result);
                                    self.test_phase = 3;
                                }
                                Err(e) => {
                                    error!("❌ Ping failed: {}", e);
                                    self.kill_child_if_needed();
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                        }
                        3 => {
                            // Phase 3: Bye test (should shutdown server)
                            info!("📤 Testing bye request (should shutdown server)");
                            match rpc_base.request_timeout("bye", None, Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 Bye response: {}", result);
                                    info!("✅ All tests completed successfully");
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                                Err(e) => {
                                    error!("❌ Bye failed: {}", e);
                                    self.kill_child_if_needed();
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                            self.test_phase = 4;
                        }
                        _ => {
                            // Wait for shutdown or timeout
                            if self.tick_count > 20 {
                                info!("⏱️ Test timeout, shutting down");
                                self.kill_child_if_needed();
                                if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                    let _ = shutdown_tx.send(());
                                }
                            }
                        }
                    }
                }
                
                request = request_rx.recv() => {
                    if let Some(request) = request {
                        self.handle_request(&rpc_base, request).await;
                    }
                }
                
                notification = notification_rx.recv() => {
                    if let Some(notification) = notification {
                        self.handle_notification(notification).await;
                    }
                }
                
                _ = &mut shutdown_rx => {
                    info!("🚪 Shutdown signal received, exiting test client");
                    return Ok(false);
                }
            }
        }
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Test client connected to server");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Test client disconnected from server");
        Ok(())
    }
}

impl TestClientHandler {
    /// Handle incoming requests from server
    async fn handle_request(&self, rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Client handling request: {}", request.method);
        let id = request.id.clone().unwrap_or(Value::Null);
        
        let result = match request.method.as_str() {
            "get_client_info" => {
                // Server asking for client info
                Ok(json!({
                    "client_name": "test_simple_echo",
                    "version": "1.0.0",
                    "capabilities": ["test_client"]
                }))
            }
            "ping" => {
                // Server asking for ping - respond with pong
                info!("🏓 Ping request received from server - responding with pong");
                Ok(json!("pong"))
            }
            _ => {
                Err(RpcError::MethodNotImplemented(request.method))
            }
        };
        
        match result {
            Ok(value) => {
                if let Err(e) = rpc_base.respond(id, value).await {
                    error!("❌ Failed to send response: {}", e);
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
    
    /// Handle incoming notifications from server
    async fn handle_notification(&self, notification: Request) {
        match notification.method.as_str() {
            "server_status" => {
                if let Some(params) = notification.params {
                    info!("📊 Server status update: {}", params);
                }
            }
            _ => {
                debug!("❓ Unknown notification from server: {}", notification.method);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🧪 Starting JSON-RPC test client");
    
    // Get the path to the simple_echo_test binary
    let current_exe = env::current_exe()?;
    let server_path = current_exe.parent()
        .unwrap()
        .join("simple_echo_test");
    
    info!("🚀 Spawning server: {}", server_path.display());
    
    // Spawn the server process with custom configuration
    let child = Command::new(&server_path)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    info!("📡 Creating JsonRpcBase from spawned process");
    
    // Store child PID for emergency kill
    let child_pid = child.id();
    
    // Create JsonRpcBase from the spawned child
    let rpc_base = JsonRpcBase::from_child(child).await?;
    
    // Create test client handler
    let mut test_handler = TestClientHandler::new();
    if let Some(pid) = child_pid {
        test_handler.set_child_pid(pid);
    }
    
    // Run the main event loop
    info!("🔄 Starting test client loop");
    match rpc_base.run_main_loop(Box::new(test_handler)).await {
        Ok(()) => {
            info!("✅ Test client completed successfully");
        }
        Err(e) => {
            error!("❌ Test client error: {}", e);
        }
    }
    
    info!("🎉 JSON-RPC test completed");
    Ok(())
}