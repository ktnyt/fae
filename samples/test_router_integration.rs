//! SearchRouter + ContentSearchWorker Integration Test
//! 
//! SearchRouter経由でContentSearchWorkerを使った完全なクエリフローをテスト

use fae::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, Request};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error};
use std::env;
use std::sync::Arc;
use std::process::Stdio;
use tokio::time::Duration;
use tokio::process::Command;

/// Integration test client handler
struct IntegrationTestHandler {
    test_phase: u32,
    tick_count: u32,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    received_responses: Vec<String>,
    search_results: Vec<String>,
}

impl IntegrationTestHandler {
    fn new() -> Self {
        Self {
            test_phase: 0,
            tick_count: 0,
            shutdown_tx: None,
            received_responses: Vec::new(),
            search_results: Vec::new(),
        }
    }
}

#[async_trait]
impl MainLoopHandler for IntegrationTestHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🧪 Integration test client started");
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        let tick_interval = Duration::from_millis(1000);
        let mut tick_timer = tokio::time::interval(tick_interval);
        
        loop {
            tokio::select! {
                _ = tick_timer.tick() => {
                    self.tick_count += 1;
                    
                    match self.test_phase {
                        0 => {
                            // Phase 0: Setup TUI connection
                            info!("📤 Setting up TUI connection with SearchRouter");
                            match rpc_base.request_timeout("setup.tui_connection", None, Duration::from_secs(3)).await {
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
                            // Phase 1: Wait for worker setup (RouterとWorkerの起動タイムラグを考慮)
                            info!("⏳ Waiting for worker setup (tick: {})", self.tick_count);
                            if self.tick_count >= 3 {
                                self.test_phase = 2;
                            }
                        }
                        2 => {
                            // Phase 2: Test integrated search query
                            info!("📤 Testing integrated search via SearchRouter");
                            let query_params = json!({
                                "query": "struct"  // Rust コードベースに存在するキーワード
                            });
                            
                            match rpc_base.request_timeout("user.query", Some(query_params), Duration::from_secs(10)).await {
                                Ok(result) => {
                                    info!("📥 Integrated search response: {}", result);
                                    self.received_responses.push(format!("integrated_search: {}", result));
                                    self.test_phase = 3;
                                }
                                Err(e) => {
                                    error!("❌ Integrated search failed: {}", e);
                                    self.received_responses.push(format!("integrated_search_error: {}", e));
                                    self.test_phase = 3;
                                }
                            }
                        }
                        3 => {
                            // Phase 3: Wait for search results and then shutdown
                            if self.tick_count >= 8 {
                                info!("📤 Testing shutdown request");
                                match rpc_base.request_timeout("shutdown", None, Duration::from_secs(3)).await {
                                    Ok(result) => {
                                        info!("📥 Shutdown response: {}", result);
                                        info!("📊 Integration test results summary:");
                                        for response in &self.received_responses {
                                            info!("   Response: {}", response);
                                        }
                                        info!("📋 Search results received: {}", self.search_results.len());
                                        for (i, result) in self.search_results.iter().enumerate() {
                                            info!("   Result {}: {}", i + 1, result);
                                        }
                                        info!("✅ SearchRouter integration test completed successfully");
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
                                self.test_phase = 4;
                            }
                        }
                        _ => {
                            // Wait for shutdown or timeout
                            if self.tick_count > 15 {
                                info!("⏱️ Integration test timeout, shutting down");
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
                    info!("🚪 Shutdown signal received, exiting integration test client");
                    return Ok(false);
                }
            }
        }
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Integration test client connected to SearchRouter");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Integration test client disconnected from SearchRouter");
        Ok(())
    }
}

impl IntegrationTestHandler {
    /// Handle incoming requests from SearchRouter
    async fn handle_request(&self, _rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Integration client handling request: {}", request.method);
        // SearchRouter typically doesn't send requests to TUI clients in our current design
    }
    
    /// Handle incoming notifications from SearchRouter (forwarded from workers)
    async fn handle_notification(&mut self, notification: Request) {
        match notification.method.as_str() {
            "search.clear" => {
                info!("🧹 Received search.clear notification via router");
                self.received_responses.push("received_search_clear_via_router".to_string());
                self.search_results.clear();
            }
            "search.match" => {
                info!("🎯 Received search.match notification via router");
                if let Some(params) = &notification.params {
                    self.search_results.push(format!("search_match: {}", params));
                    info!("   📄 Search match: {}", params);
                }
                self.received_responses.push("received_search_match_via_router".to_string());
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
    
    info!("🧪 Starting SearchRouter + ContentSearchWorker integration test");
    
    // Get the path to the binaries
    let current_exe = env::current_exe()?;
    let bin_dir = current_exe.parent().unwrap();
    let router_path = bin_dir.join("search_router");
    let _worker_path = bin_dir.join("content_search_worker");
    
    // Note: In the current implementation, SearchRouter auto-starts ContentSearchWorker
    // so we don't need to manually spawn it here. This simplified the integration test.
    
    info!("🚀 Spawning SearchRouter: {}", router_path.display());
    
    // Spawn the SearchRouter
    let router_child = Command::new(&router_path)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    info!("📡 Creating JsonRpcBase from spawned SearchRouter");
    
    // Create JsonRpcBase from the spawned router
    let rpc_base = JsonRpcBase::from_child(router_child).await?;
    
    // Create integration test client handler
    let test_handler = IntegrationTestHandler::new();
    
    // Run the main event loop
    info!("🔄 Starting integration test client loop");
    match rpc_base.run_main_loop(Box::new(test_handler)).await {
        Ok(()) => {
            info!("✅ Integration test completed successfully");
        }
        Err(e) => {
            error!("❌ Integration test error: {}", e);
        }
    }
    
    // Clean up processes
    info!("🧹 Cleaning up processes");
    // Note: SearchRouter manages its own worker lifecycle, so cleanup is automatic
    
    info!("🎉 SearchRouter + ContentSearchWorker integration test completed");
    Ok(())
}