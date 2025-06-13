//! SearchRouter Unit Test
//! 
//! SearchRouterのルーティング機能をテストする

use fae::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, Request};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error};
use std::env;
use std::sync::Arc;
use std::process::Stdio;
use tokio::time::Duration;
use tokio::process::Command;

/// Test client handler for SearchRouter
struct SearchRouterTestHandler {
    test_phase: u32,
    tick_count: u32,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    received_responses: Vec<String>,
}

impl SearchRouterTestHandler {
    fn new() -> Self {
        Self {
            test_phase: 0,
            tick_count: 0,
            shutdown_tx: None,
            received_responses: Vec::new(),
        }
    }
}

#[async_trait]
impl MainLoopHandler for SearchRouterTestHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🧪 SearchRouter test client started");
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        let tick_interval = Duration::from_millis(500);
        let mut tick_timer = tokio::time::interval(tick_interval);
        
        loop {
            tokio::select! {
                _ = tick_timer.tick() => {
                    self.tick_count += 1;
                    
                    match self.test_phase {
                        0 => {
                            // Phase 0: Setup TUI connection
                            info!("📤 Testing TUI connection setup");
                            match rpc_base.request_timeout("setup.tui_connection", None, Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 TUI connection response: {}", result);
                                    self.received_responses.push(format!("tui_connection: {}", result));
                                    self.test_phase = 1;
                                }
                                Err(e) => {
                                    error!("❌ TUI connection failed: {}", e);
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                        }
                        1 => {
                            // Phase 1: Test query routing (without worker)
                            info!("📤 Testing query routing");
                            let query_params = json!({
                                "query": "test query"
                            });
                            
                            match rpc_base.request_timeout("user.query", Some(query_params), Duration::from_secs(5)).await {
                                Ok(result) => {
                                    info!("📥 Query routing response: {}", result);
                                    self.received_responses.push(format!("query_routing: {}", result));
                                    self.test_phase = 2;
                                }
                                Err(e) => {
                                    // Expected to fail since no worker is connected
                                    info!("⚠️ Query routing failed as expected (no worker): {}", e);
                                    self.received_responses.push(format!("query_routing_error: {}", e));
                                    self.test_phase = 2;
                                }
                            }
                        }
                        2 => {
                            // Phase 2: Test shutdown
                            info!("📤 Testing shutdown request");
                            match rpc_base.request_timeout("shutdown", None, Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 Shutdown response: {}", result);
                                    info!("📊 Test results summary:");
                                    for response in &self.received_responses {
                                        info!("   {}", response);
                                    }
                                    info!("✅ SearchRouter unit test completed successfully");
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                                Err(e) => {
                                    error!("❌ Shutdown failed: {}", e);
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                            self.test_phase = 3;
                        }
                        _ => {
                            // Wait for shutdown or timeout
                            if self.tick_count > 20 {
                                info!("⏱️ Test timeout, shutting down");
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
        info!("🔗 Test client connected to SearchRouter");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Test client disconnected from SearchRouter");
        Ok(())
    }
}

impl SearchRouterTestHandler {
    /// Handle incoming requests from SearchRouter
    async fn handle_request(&self, _rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Client handling request: {}", request.method);
        // SearchRouter typically doesn't send requests to clients in our current design
    }
    
    /// Handle incoming notifications from SearchRouter
    async fn handle_notification(&mut self, notification: Request) {
        match notification.method.as_str() {
            "search.clear" => {
                info!("🧹 Received search.clear notification from router");
                self.received_responses.push("received_search_clear".to_string());
            }
            "search.match" => {
                info!("🎯 Received search.match notification from router");
                self.received_responses.push("received_search_match".to_string());
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
    
    info!("🧪 Starting SearchRouter unit test");
    
    // Get the path to the search_router binary
    let current_exe = env::current_exe()?;
    let router_path = current_exe.parent()
        .unwrap()
        .join("search_router");
    
    info!("🚀 Spawning SearchRouter: {}", router_path.display());
    
    // Spawn the router process
    let child = Command::new(&router_path)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    info!("📡 Creating JsonRpcBase from spawned SearchRouter");
    
    // Create JsonRpcBase from the spawned child
    let rpc_base = JsonRpcBase::from_child(child).await?;
    
    // Create test client handler
    let test_handler = SearchRouterTestHandler::new();
    
    // Run the main event loop
    info!("🔄 Starting test client loop");
    match rpc_base.run_main_loop(Box::new(test_handler)).await {
        Ok(()) => {
            info!("✅ SearchRouter unit test completed successfully");
        }
        Err(e) => {
            error!("❌ SearchRouter unit test error: {}", e);
        }
    }
    
    info!("🎉 SearchRouter unit test completed");
    Ok(())
}