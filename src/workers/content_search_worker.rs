//! ContentSearchWorker - Simple ripgrep-based text search worker
//! 
//! ripgrepを直接呼び出してテキスト検索を行い、結果をJSON-RPC経由で返すワーカー

use crate::jsonrpc::{JsonRpcBase, MainLoopHandler, RpcResult, RpcError, Request};
use crate::workers::message_types::{QueryRequest, SearchMatch};
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug, error, warn};
use std::sync::Arc;
use std::process::Command;
use tokio::sync::{oneshot, Mutex};
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

/// ContentSearchWorker handler with search interruption support
pub struct ContentSearchHandler {
    /// Graceful shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Current working directory for search
    working_dir: String,
    /// Current search cancellation token
    search_cancellation: Arc<Mutex<Option<CancellationToken>>>,
}

impl ContentSearchHandler {
    pub fn new() -> Self {
        let working_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .to_string_lossy()
            .to_string();
            
        Self {
            shutdown_tx: None,
            working_dir,
            search_cancellation: Arc::new(Mutex::new(None)),
        }
    }

    /// Cancel current search if any
    async fn cancel_current_search(&self) {
        debug!("🛑 Cancelling current search");
        
        // Cancel via cancellation token
        if let Some(cancellation) = self.search_cancellation.lock().await.take() {
            cancellation.cancel();
        }
    }

    /// Execute ripgrep search and send results via JSON-RPC with interruption support
    async fn execute_search(&self, rpc_base: &Arc<JsonRpcBase>, query: &str) -> RpcResult<()> {
        debug!("🔍 Starting interruptible ripgrep search for: '{}'", query);

        // Cancel any existing search
        self.cancel_current_search().await;

        // Clear previous results
        rpc_base.notify("search.clear", None).await?;

        // Check if ripgrep is available
        if !self.is_ripgrep_available() {
            error!("❌ ripgrep (rg) is not available on this system");
            return Err(RpcError::Rpc {
                code: -32603,
                message: "ripgrep not found".to_string(),
            });
        }

        // Create cancellation token for this search
        let cancellation_token = CancellationToken::new();
        *self.search_cancellation.lock().await = Some(cancellation_token.clone());

        // Execute ripgrep asynchronously
        let child = tokio::process::Command::new("rg")
            .args([
                "--vimgrep",           // file:line:column:content format
                "--byte-offset",       // include byte offset
                "-i",                  // case insensitive
                "-F",                  // literal search (no regex)
                "--max-filesize", "1M", // exclude files > 1MB
                query,
            ])
            .current_dir(&self.working_dir)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                error!("❌ Failed to spawn ripgrep: {}", e);
                RpcError::Rpc {
                    code: -32603,
                    message: format!("Failed to spawn ripgrep: {}", e),
                }
            })?;

        // Wait for the process to complete with timeout to avoid hanging
        let timeout_duration = Duration::from_secs(10); // 10 second timeout
        let result = tokio::time::timeout(timeout_duration, child.wait_with_output()).await;

        let final_result = match result {
            Ok(Ok(output)) => {
                if output.status.success() || output.status.code() == Some(1) {
                    // Exit code 1 means "no matches found" which is normal
                    self.parse_and_send_results(rpc_base, &output.stdout).await?;
                    info!("✅ Search completed for: '{}'", query);
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("❌ ripgrep failed with exit code {:?}: {}", output.status.code(), stderr);
                    Err(RpcError::Rpc {
                        code: -32603,
                        message: format!("ripgrep failed: {}", stderr),
                    })
                }
            }
            Ok(Err(e)) => {
                error!("❌ Failed to wait for ripgrep: {}", e);
                Err(RpcError::Rpc {
                    code: -32603,
                    message: format!("Failed to wait for ripgrep: {}", e),
                })
            }
            Err(_timeout) => {
                error!("❌ Search timed out for: '{}'", query);
                Err(RpcError::Rpc {
                    code: -32603,
                    message: "Search timed out".to_string(),
                })
            }
        };

        // Clean up
        *self.search_cancellation.lock().await = None;

        final_result
    }

    /// Parse ripgrep output and send results
    async fn parse_and_send_results(&self, rpc_base: &Arc<JsonRpcBase>, stdout: &[u8]) -> RpcResult<()> {
        let output = String::from_utf8_lossy(stdout);
        let mut match_count = 0;

        for line in output.lines() {
            if let Some(search_match) = self.parse_ripgrep_line(line) {
                // Send each match as a notification
                let match_data = json!({
                    "filename": search_match.filename,
                    "line": search_match.line,
                    "column": search_match.column,
                    "content": search_match.content
                });

                rpc_base.notify("search.match", Some(match_data)).await?;
                match_count += 1;

                // Limit results to prevent overwhelming
                if match_count >= 1000 {
                    warn!("⚠️ Limiting results to 1000 matches");
                    break;
                }
            }
        }

        debug!("📊 Sent {} search matches", match_count);
        Ok(())
    }

    /// Parse a single ripgrep output line
    /// Format: file:line:column:byte_offset:content
    fn parse_ripgrep_line(&self, line: &str) -> Option<SearchMatch> {
        let parts: Vec<&str> = line.splitn(5, ':').collect();
        if parts.len() != 5 {
            return None;
        }

        let filename = parts[0].to_string();
        let line_num: u32 = parts[1].parse().ok()?;
        let column_num: u32 = parts[2].parse().ok()?;
        // parts[3] is byte_offset, we don't need it for basic functionality
        let content = parts[4].to_string();

        Some(SearchMatch::new(filename, line_num, column_num, content))
    }

    /// Check if ripgrep is available
    fn is_ripgrep_available(&self) -> bool {
        Command::new("rg")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl MainLoopHandler for ContentSearchHandler {
    async fn run_loop(
        &mut self,
        rpc_base: Arc<JsonRpcBase>,
        mut request_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
        mut notification_rx: tokio::sync::mpsc::UnboundedReceiver<Request>,
    ) -> RpcResult<bool> {
        info!("🚀 ContentSearchWorker started");
        info!("📁 Working directory: {}", self.working_dir);

        // Setup shutdown channel
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
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
                    info!("🚪 Shutdown signal received, exiting ContentSearchWorker");
                    return Ok(false);
                }
            }
        }
    }

    async fn on_connected(&mut self) -> RpcResult<()> {
        info!("🔗 ContentSearchWorker connected");
        Ok(())
    }

    async fn on_disconnected(&mut self) -> RpcResult<()> {
        info!("👋 ContentSearchWorker disconnected");
        Ok(())
    }
}

impl ContentSearchHandler {
    /// Handle incoming requests
    async fn handle_request(&mut self, rpc_base: &Arc<JsonRpcBase>, request: Request) {
        debug!("📥 Handling request: {}", request.method);
        let id = request.id.clone().unwrap_or(Value::Null);

        let result = match request.method.as_str() {
            "user.query" => {
                // Parse query request
                match request.params {
                    Some(params) => {
                        match serde_json::from_value::<QueryRequest>(params) {
                            Ok(query_req) => {
                                // Execute search synchronously for now
                                match self.execute_search(rpc_base, &query_req.query).await {
                                    Ok(()) => Ok(json!("search_completed")),
                                    Err(e) => {
                                        error!("❌ Search execution failed: {}", e);
                                        Err(e)
                                    }
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

    /// Handle incoming notifications
    async fn handle_notification(&self, _rpc_base: &Arc<JsonRpcBase>, notification: Request) {
        debug!("📢 Handling notification: {}", notification.method);
        // ContentSearchWorker doesn't currently handle notifications
        // Future: Could handle cancellation requests here
    }

}