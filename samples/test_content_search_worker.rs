//! ContentSearchWorker Integration Test
//! 
//! ContentSearchWorkerのJSON-RPC機能をテストする

use fae::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, Request};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error};
use std::env;
use std::sync::Arc;
use std::process::Stdio;
use tokio::time::Duration;
use tokio::process::Command;
use std::collections::HashMap;

/// Test client handler for ContentSearchWorker
struct ContentSearchTestHandler {
    test_phase: u32,
    tick_count: u32,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    received_matches: Vec<SearchMatch>,
    search_cleared: bool,
}

#[derive(Debug, Clone)]
struct SearchMatch {
    filename: String,
    line: u32,
    column: u32,
    content: String,
}

impl ContentSearchTestHandler {
    fn new() -> Self {
        Self {
            test_phase: 0,
            tick_count: 0,
            shutdown_tx: None,
            received_matches: Vec::new(),
            search_cleared: false,
        }
    }
}

#[async_trait]
impl MainLoopHandler for ContentSearchTestHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🧪 ContentSearchWorker test client started");
        
        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        let tick_interval = Duration::from_millis(500); // Test timing
        let mut tick_timer = tokio::time::interval(tick_interval);
        
        loop {
            tokio::select! {
                _ = tick_timer.tick() => {
                    self.tick_count += 1;
                    
                    match self.test_phase {
                        0 => {
                            // Phase 0: Send search query
                            info!("📤 Testing content search query");
                            let query_params = json!({
                                "query": "use"  // Common keyword likely to be found
                            });
                            
                            match rpc_base.request_timeout("user.query", Some(query_params), Duration::from_secs(10)).await {
                                Ok(result) => {
                                    info!("📥 Search request response: {}", result);
                                    self.test_phase = 1;
                                }
                                Err(e) => {
                                    error!("❌ Search request failed: {}", e);
                                    if let Some(shutdown_tx) = self.shutdown_tx.take() {
                                        let _ = shutdown_tx.send(());
                                    }
                                }
                            }
                        }
                        1 => {
                            // Phase 1: Wait for results and then test shutdown
                            if self.tick_count > 10 { // Wait for a few ticks
                                info!("📊 Test results summary:");
                                info!("   Search cleared: {}", self.search_cleared);
                                info!("   Matches received: {}", self.received_matches.len());
                                
                                // Log some example matches
                                for (i, m) in self.received_matches.iter().take(3).enumerate() {
                                    info!("   Match {}: {}:{}:{} - {}", 
                                          i + 1, m.filename, m.line, m.column, 
                                          m.content.chars().take(50).collect::<String>());
                                }
                                
                                self.test_phase = 2;
                            }
                        }
                        2 => {
                            // Phase 2: Test shutdown
                            info!("📤 Testing shutdown request");
                            match rpc_base.request_timeout("shutdown", None, Duration::from_secs(2)).await {
                                Ok(result) => {
                                    info!("📥 Shutdown response: {}", result);
                                    info!("✅ All tests completed successfully");
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
                            if self.tick_count > 30 {
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
        info!("🔗 Test client connected to ContentSearchWorker");
        Ok(())
    }
    
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 Test client disconnected from ContentSearchWorker");
        Ok(())
    }
}

impl ContentSearchTestHandler {
    /// Handle incoming requests from ContentSearchWorker
    async fn handle_request(&self, rpc_base: &JsonRpcBase, request: Request) {
        debug!("📥 Client handling request: {}", request.method);
        // ContentSearchWorker doesn't typically send requests to clients
        // This is here for completeness
    }
    
    /// Handle incoming notifications from ContentSearchWorker
    async fn handle_notification(&mut self, notification: Request) {
        match notification.method.as_str() {
            "search.clear" => {
                info!("🧹 Received search.clear notification");
                self.search_cleared = true;
                self.received_matches.clear();
            }
            "search.match" => {
                if let Some(params) = notification.params {
                    if let Ok(match_data) = serde_json::from_value::<HashMap<String, Value>>(params) {
                        let search_match = SearchMatch {
                            filename: match_data.get("filename")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown").to_string(),
                            line: match_data.get("line")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            column: match_data.get("column")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            content: match_data.get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("").to_string(),
                        };
                        
                        debug!("🎯 Received match: {}:{}:{} - {}", 
                               search_match.filename, search_match.line, search_match.column,
                               search_match.content.chars().take(50).collect::<String>());
                        
                        self.received_matches.push(search_match);
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
    
    info!("🧪 Starting ContentSearchWorker integration test");
    
    // Get the path to the content_search_worker binary
    let current_exe = env::current_exe()?;
    let worker_path = current_exe.parent()
        .unwrap()
        .join("content_search_worker");
    
    info!("🚀 Spawning ContentSearchWorker: {}", worker_path.display());
    
    // Spawn the worker process
    let child = Command::new(&worker_path)
        .env("RUST_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    info!("📡 Creating JsonRpcBase from spawned ContentSearchWorker");
    
    // Create JsonRpcBase from the spawned child
    let rpc_base = JsonRpcBase::from_child(child).await?;
    
    // Create test client handler
    let test_handler = ContentSearchTestHandler::new();
    
    // Run the main event loop
    info!("🔄 Starting test client loop");
    match rpc_base.run_main_loop(Box::new(test_handler)).await {
        Ok(()) => {
            info!("✅ ContentSearchWorker test completed successfully");
        }
        Err(e) => {
            error!("❌ ContentSearchWorker test error: {}", e);
        }
    }
    
    info!("🎉 ContentSearchWorker integration test completed");
    Ok(())
}