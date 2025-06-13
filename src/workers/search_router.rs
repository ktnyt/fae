//! SearchRouter - Message routing between TUI and search workers
//! 
//! TuiWorkerからのクエリを適切な検索ワーカーにルーティングし、
//! 検索結果をTuiWorkerに転送するメッセージルーター

use crate::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, RpcError, Request};
use crate::workers::message_types::{QueryRequest, SearchMatch};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error, warn};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::oneshot;

/// SearchRouter handler
pub struct SearchRouterHandler {
    /// Graceful shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Connected search workers
    workers: HashMap<String, Arc<JsonRpcBase>>,
    /// Current working directory
    working_dir: String,
}

impl SearchRouterHandler {
    pub fn new() -> Self {
        let working_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .to_string_lossy()
            .to_string();
            
        Self {
            shutdown_tx: None,
            workers: HashMap::new(),
            working_dir,
        }
    }

    /// Add a search worker to the router
    pub async fn add_worker(&mut self, worker_type: &str, worker_rpc: Arc<JsonRpcBase>) -> RpcResult<()> {
        info!("🔌 Adding {} worker to SearchRouter", worker_type);
        self.workers.insert(worker_type.to_string(), worker_rpc);
        Ok(())
    }

    /// Route query to appropriate worker based on query prefix
    async fn route_query(&self, tui_rpc: &Arc<JsonRpcBase>, query: &str) -> RpcResult<()> {
        debug!("🧭 Routing query: '{}'", query);

        // For now, route all queries to ContentSearchWorker
        // Future: Parse query prefixes (#, >, /) for different workers
        let worker_type = "content_search";
        
        if let Some(worker_rpc) = self.workers.get(worker_type) {
            info!("📤 Forwarding query to {} worker", worker_type);
            
            // Forward query to worker
            let query_req = json!({
                "query": query
            });
            
            match worker_rpc.request("user.query", Some(query_req)).await {
                Ok(response) => {
                    debug!("✅ Worker responded: {}", response);
                    Ok(())
                }
                Err(e) => {
                    error!("❌ Worker request failed: {}", e);
                    Err(e)
                }
            }
        } else {
            warn!("⚠️ No {} worker available", worker_type);
            Err(RpcError::Rpc {
                code: -32603,
                message: format!("Worker {} not available", worker_type),
            })
        }
    }

    /// Forward worker notification to TUI
    async fn forward_to_tui(&self, tui_rpc: &Arc<JsonRpcBase>, method: &str, params: Option<Value>) -> RpcResult<()> {
        debug!("📡 Forwarding {} to TUI", method);
        tui_rpc.notify(method, params).await
    }

    /// Start ContentSearchWorker as a child process
    async fn start_content_search_worker(&mut self) -> RpcResult<()> {
        use tokio::process::Command;
        use std::process::Stdio;
        
        info!("🚀 Starting ContentSearchWorker process");
        
        // Get the path to content_search_worker binary
        let current_exe = std::env::current_exe()
            .map_err(|e| RpcError::Rpc {
                code: -32603,
                message: format!("Failed to get current exe: {}", e),
            })?;
        
        let worker_path = current_exe.parent()
            .ok_or_else(|| RpcError::Rpc {
                code: -32603,
                message: "Failed to get parent directory".to_string(),
            })?
            .join("content_search_worker");
        
        info!("📍 ContentSearchWorker path: {}", worker_path.display());
        
        // Spawn the worker process
        let child = Command::new(&worker_path)
            .env("RUST_LOG", "debug")
            .current_dir(&self.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RpcError::Rpc {
                code: -32603,
                message: format!("Failed to spawn ContentSearchWorker: {}", e),
            })?;
        
        // Create JsonRpcBase from the spawned child
        let worker_rpc = Arc::new(JsonRpcBase::from_child(child).await?);
        
        // Store the worker
        self.workers.insert("content_search".to_string(), worker_rpc.clone());
        
        info!("✅ ContentSearchWorker started and registered");
        
        // Note: In a full implementation, we would start a separate task to listen 
        // for notifications from this worker and forward them to TUI.
        // For now, this will be handled in the notification handler.
        
        Ok(())
    }
}

#[async_trait]
impl MainLoopHandler for SearchRouterHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🚀 SearchRouter started");
        info!("📁 Working directory: {}", self.working_dir);
        info!("🔌 Available workers: {:?}", self.workers.keys().collect::<Vec<_>>());

        // Setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // Auto-start ContentSearchWorker if not already present
        if !self.workers.contains_key("content_search") {
            if let Err(e) = self.start_content_search_worker().await {
                error!("❌ Failed to start ContentSearchWorker: {}", e);
            }
        }

        // For this implementation, we need a reference to TUI RPC
        // This will be provided through a special setup method or via request parameters
        let mut tui_rpc: Option<Arc<JsonRpcBase>> = None;

        loop {
            tokio::select! {
                request = request_rx.recv() => {
                    if let Some(request) = request {
                        self.handle_request(&rpc_base, &mut tui_rpc, request).await;
                    }
                }

                notification = notification_rx.recv() => {
                    if let Some(notification) = notification {
                        self.handle_notification(&tui_rpc, notification).await;
                    }
                }

                _ = &mut shutdown_rx => {
                    info!("🚪 Shutdown signal received, exiting SearchRouter");
                    return Ok(false);
                }
            }
        }
    }

    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 SearchRouter connected");
        Ok(())
    }

    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 SearchRouter disconnected");
        Ok(())
    }
}

impl SearchRouterHandler {
    /// Handle incoming requests from TUI
    async fn handle_request(
        &mut self, 
        rpc_base: &Arc<JsonRpcBase>, 
        tui_rpc: &mut Option<Arc<JsonRpcBase>>,
        request: Request
    ) {
        debug!("📥 SearchRouter handling request: {}", request.method);
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "user.query" => {
                // Parse query request from TUI
                match request.params {
                    Some(params) => {
                        match serde_json::from_value::<QueryRequest>(params) {
                            Ok(query_req) => {
                                // Create mock TUI RPC for testing
                                // In real implementation, this would be passed differently
                                if tui_rpc.is_none() {
                                    *tui_rpc = Some(rpc_base.clone());
                                }
                                
                                if let Some(tui) = tui_rpc {
                                    match self.route_query(tui, &query_req.query).await {
                                        Ok(()) => Ok(json!("query_routed")),
                                        Err(e) => {
                                            error!("❌ Query routing failed: {}", e);
                                            Err(e)
                                        }
                                    }
                                } else {
                                    error!("❌ No TUI connection available");
                                    Err(RpcError::Rpc {
                                        code: -32603,
                                        message: "No TUI connection".to_string(),
                                    })
                                }
                            }
                            Err(e) => {
                                error!("❌ Invalid query parameters: {}", e);
                                Err(RpcError::Rpc {
                                    code: -32602,
                                    message: format!("Invalid parameters: {}", e),
                                })
                            }
                        }
                    }
                    None => {
                        error!("❌ Missing query parameters");
                        Err(RpcError::Rpc {
                            code: -32602,
                            message: "Missing parameters".to_string(),
                        })
                    }
                }
            }
            "setup.tui_connection" => {
                // Special method to establish TUI connection
                // In practice, this would be handled differently
                info!("🔗 Setting up TUI connection");
                *tui_rpc = Some(rpc_base.clone());
                Ok(json!("tui_connected"))
            }
            "shutdown" => {
                info!("🛑 Shutdown request received");
                if let Some(shutdown_tx) = self.shutdown_tx.take() {
                    let _ = shutdown_tx.send(());
                }
                Ok(json!("shutting_down"))
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
            }
            Err(e) => {
                let error_obj = match e {
                    RpcError::MethodNotImplemented(_) => {
                        crate::jsonrpc::ErrorObject::new(crate::jsonrpc::ErrorCode::MethodNotFound, None)
                    }
                    _ => {
                        crate::jsonrpc::ErrorObject::custom(-32603, e.to_string(), None)
                    }
                };
                if let Err(e) = rpc_base.respond_error(id, error_obj).await {
                    error!("❌ Failed to send error response: {}", e);
                }
            }
        }
    }

    /// Handle incoming notifications from workers
    async fn handle_notification(&self, tui_rpc: &Option<Arc<JsonRpcBase>>, notification: Request) {
        debug!("📢 SearchRouter handling notification: {}", notification.method);
        
        // Forward worker notifications to TUI
        match notification.method.as_str() {
            "search.clear" => {
                if let Some(tui) = tui_rpc {
                    if let Err(e) = self.forward_to_tui(tui, "search.clear", None).await {
                        error!("❌ Failed to forward search.clear: {}", e);
                    }
                }
            }
            "search.match" => {
                if let Some(tui) = tui_rpc {
                    if let Err(e) = self.forward_to_tui(tui, "search.match", notification.params).await {
                        error!("❌ Failed to forward search.match: {}", e);
                    }
                }
            }
            _ => {
                debug!("❓ Unknown notification from worker: {}", notification.method);
            }
        }
    }
}