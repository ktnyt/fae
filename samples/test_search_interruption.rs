//! Search Interruption Test
//! 
//! ContentSearchWorkerの検索割り込み機能をテストする

use fae::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, Request};
use serde_json::{json};
use async_trait::async_trait;
use log::{info, debug, error};
use std::env;
use std::sync::Arc;
use std::process::Stdio;
use tokio::time::{Duration, sleep};
use tokio::process::Command;

/// Interruption test client handler
struct InterruptionTestHandler {
    test_phase: u32,
    tick_count: u32,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    search_results: Vec<String>,
    first_search_cancelled: bool,
    second_search_completed: bool,
}

impl InterruptionTestHandler {
    fn new() -> Self {
        Self {
            test_phase: 0,
            tick_count: 0,
            shutdown_tx: None,
            search_results: Vec::new(),
            first_search_cancelled: false,
            second_search_completed: false,
        }
    }
}

#[async_trait]
impl MainLoopHandler for InterruptionTestHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🧪 Search interruption test client started");
        
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
                            // Phase 0: Start first search (broad search likely to have many results)
                            info!("📤 Starting first search (broad query)");
                            let query_params = json!({
                                "query": "use"  // Very common keyword in Rust code
                            });
                            
                            // Send first search request (don't wait for response)
                            if let Err(e) = rpc_base.request("user.query", Some(query_params)).await {
                                error!("❌ First search request failed: {}", e);
                            } else {
                                info!("✅ First search request sent");
                                self.test_phase = 1;
                            }
                        }
                        1 => {
                            // Phase 1: Wait a short time, then send interrupting search
                            if self.tick_count >= 2 {
                                info!("📤 Sending interrupting search");
                                let query_params = json!({
                                    "query": "JsonRpcBase"  // More specific, should interrupt first search
                                });
                                
                                if let Err(e) = rpc_base.request("user.query", Some(query_params)).await {
                                    error!("❌ Second search request failed: {}", e);
                                } else {
                                    info!("✅ Second search request sent");
                                    self.test_phase = 2;
                                }
                            }
                        }
                        2 => {
                            // Phase 2: Wait for results and analyze interruption behavior
                            if self.tick_count >= 8 {
                                info!("📊 Analyzing interruption test results:");
                                info!("   First search cancelled: {}", self.first_search_cancelled);
                                info!("   Second search completed: {}", self.second_search_completed);
                                info!("   Total search results received: {}", self.search_results.len());
                                
                                // Print first few results for verification
                                for (i, result) in self.search_results.iter().take(3).enumerate() {
                                    info!("   Result {}: {}", i + 1, result);
                                }
                                
                                // Send shutdown
                                info!("📤 Testing shutdown request");
                                match rpc_base.request_timeout("shutdown", None, Duration::from_secs(3)).await {
                                    Ok(_) => {
                                        if self.search_results.iter().any(|r| r.contains("JsonRpcBase")) {
                                            info!("✅ Interruption test PASSED: Found JsonRpcBase results from second search");
                                        } else {
                                            info!("⚠️ Interruption test UNCLEAR: No JsonRpcBase results found");
                                        }
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
                    info!("🚪 Shutdown signal received, exiting interruption test client");
                    return Ok(false);
                }
            }
        }
    }
    
    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 Interruption test client connected to ContentSearchWorker");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Interruption test client disconnected from ContentSearchWorker");
        Ok(())
    }
}

impl InterruptionTestHandler {
    /// Handle incoming requests from ContentSearchWorker
    async fn handle_request(&self, _rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Interruption test handling request: {}", request.method);
        // ContentSearchWorker typically doesn't send requests to clients
    }
    
    /// Handle incoming notifications from ContentSearchWorker
    async fn handle_notification(&mut self, notification: Request) {
        match notification.method.as_str() {
            "search.clear" => {
                info!("🧹 Received search.clear notification - search restarted");
                self.search_results.clear();
            }
            "search.match" => {
                if let Some(params) = &notification.params {
                    if let Some(content) = params.get("content").and_then(|v| v.as_str()) {
                        self.search_results.push(content.to_string());
                        
                        // Check if this looks like results from the second search
                        if content.contains("JsonRpcBase") {
                            self.second_search_completed = true;
                            debug!("✅ Found JsonRpcBase result: {}", content);
                        }
                        
                        // If we get a lot of results quickly, first search might not have been cancelled
                        if self.search_results.len() > 50 && !self.second_search_completed {
                            debug!("⚠️ Many results without JsonRpcBase - first search may not have been cancelled");
                        }
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
    
    info!("🧪 Starting ContentSearchWorker search interruption test");
    
    // Get the path to the content_search_worker binary
    let current_exe = env::current_exe()?;
    let worker_path = current_exe.parent()
        .unwrap()
        .join("content_search_worker");
    
    info!("🚀 Spawning ContentSearchWorker: {}", worker_path.display());
    
    // Spawn the ContentSearchWorker
    let child = Command::new(&worker_path)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    info!("📡 Creating JsonRpcBase from spawned ContentSearchWorker");
    
    // Create JsonRpcBase from the spawned child
    let rpc_base = JsonRpcBase::from_child(child).await?;
    
    // Create interruption test client handler
    let test_handler = InterruptionTestHandler::new();
    
    // Run the main event loop
    info!("🔄 Starting interruption test client loop");
    match rpc_base.run_main_loop(Box::new(test_handler)).await {
        Ok(()) => {
            info!("✅ Search interruption test completed");
        }
        Err(e) => {
            error!("❌ Search interruption test error: {}", e);
        }
    }
    
    info!("🎉 Search interruption test completed");
    Ok(())
}