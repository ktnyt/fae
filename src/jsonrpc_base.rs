use crate::jsonrpc::{Request, Response, Message, ErrorObject, ErrorCode};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};
use log::{debug, error, warn};
use regex::Regex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use async_trait::async_trait;

/// Result type for RPC operations
pub type RpcResult<T> = Result<T, RpcError>;

/// RPC error types
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("RPC error [{code}]: {message}")]
    Rpc { code: i32, message: String },
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Process terminated")]
    ProcessTerminated,
    
    #[error("Method not implemented: {0}")]
    MethodNotImplemented(String),
}

/// Handler for incoming requests (expects response)
#[async_trait]
pub trait RequestHandler: Send + Sync {
    /// Handle an incoming request and return a response
    async fn handle_request(&self, request: Request) -> RpcResult<Value>;
}

/// Handler for incoming notifications (no response expected)
#[async_trait]
pub trait NotificationHandler: Send + Sync {
    /// Handle an incoming notification
    async fn handle_notification(&self, notification: Request) -> RpcResult<()>;
}

/// Handler for the main event loop
#[async_trait]
pub trait MainLoopHandler: Send + Sync {
    /// Called periodically during the main loop
    /// Return false to stop the main loop
    async fn on_tick(&mut self) -> RpcResult<bool>;
    
    /// Called when the RPC connection is established
    async fn on_connected(&mut self) -> RpcResult<()> {
        Ok(())
    }
    
    /// Called when the RPC connection is closed
    async fn on_disconnected(&mut self) -> RpcResult<()> {
        Ok(())
    }
}

/// Bidirectional JSON-RPC base that can act as both client and server
pub struct JsonRpcBase {
    child: Option<Child>,
    request_id: Arc<AtomicU64>,
    pending_requests: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<RpcResult<Response>>>>>,
    
    // Communication channels
    outbound_tx: mpsc::UnboundedSender<Message>,
    inbound_request_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Request>>>,
    inbound_notification_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Request>>>,
    
    // Handlers
    request_handler: Option<Arc<dyn RequestHandler>>,
    notification_handler: Option<Arc<dyn NotificationHandler>>,
    
    // Control channels
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl JsonRpcBase {
    /// Create a new JsonRpcBase by spawning a child process
    pub async fn spawn(command: &str, args: &[&str]) -> RpcResult<Self> {
        debug!("Spawning process: {} {:?}", command, args);
        
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        Self::new_with_streams(Some(child), stdin, stdout, stderr).await
    }
    
    /// Create a new JsonRpcBase using stdio (for server mode)
    pub async fn new_stdio() -> RpcResult<Self> {
        debug!("Creating JsonRpcBase with stdio");
        
        // For stdio mode, we'll create a simplified version
        // that works directly with stdin/stdout
        Self::new_stdio_streams().await
    }
    
    /// Create JsonRpcBase for stdio mode
    async fn new_stdio_streams() -> RpcResult<Self> {
        let request_id = Arc::new(AtomicU64::new(1));
        let pending_requests = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        
        // Create communication channels
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_request_tx, inbound_request_rx) = mpsc::unbounded_channel();
        let (inbound_notification_tx, inbound_notification_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        // Spawn I/O handlers for stdio
        tokio::spawn(Self::stdio_stdin_handler(outbound_rx, shutdown_rx));
        tokio::spawn(Self::stdio_stdout_handler(
            pending_requests.clone(),
            inbound_request_tx,
            inbound_notification_tx,
        ));
        
        Ok(Self {
            child: None,
            request_id,
            pending_requests,
            outbound_tx,
            inbound_request_rx: Arc::new(tokio::sync::Mutex::new(inbound_request_rx)),
            inbound_notification_rx: Arc::new(tokio::sync::Mutex::new(inbound_notification_rx)),
            request_handler: None,
            notification_handler: None,
            shutdown_tx: Some(shutdown_tx),
        })
    }
    
    /// Internal constructor with streams
    async fn new_with_streams(
        child: Option<Child>,
        stdin: tokio::process::ChildStdin,
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
    ) -> RpcResult<Self> {
        let request_id = Arc::new(AtomicU64::new(1));
        let pending_requests = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        
        // Create communication channels
        let (outbound_tx, outbound_rx) = mpsc::unbounded_channel();
        let (inbound_request_tx, inbound_request_rx) = mpsc::unbounded_channel();
        let (inbound_notification_tx, inbound_notification_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        // Spawn I/O handlers
        tokio::spawn(Self::stdin_handler(stdin, outbound_rx, shutdown_rx));
        tokio::spawn(Self::stdout_handler(
            stdout,
            pending_requests.clone(),
            inbound_request_tx,
            inbound_notification_tx,
        ));
        tokio::spawn(Self::stderr_handler(stderr));
        
        Ok(Self {
            child,
            request_id,
            pending_requests,
            outbound_tx,
            inbound_request_rx: Arc::new(tokio::sync::Mutex::new(inbound_request_rx)),
            inbound_notification_rx: Arc::new(tokio::sync::Mutex::new(inbound_notification_rx)),
            request_handler: None,
            notification_handler: None,
            shutdown_tx: Some(shutdown_tx),
        })
    }
    
    /// Set the request handler
    pub fn with_request_handler(mut self, handler: Arc<dyn RequestHandler>) -> Self {
        self.request_handler = Some(handler);
        self
    }
    
    /// Set the notification handler
    pub fn with_notification_handler(mut self, handler: Arc<dyn NotificationHandler>) -> Self {
        self.notification_handler = Some(handler);
        self
    }
    
    /// Send a request and wait for response (client functionality)
    pub async fn request(&self, method: &str, params: Option<Value>) -> RpcResult<Value> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = Request::new(
            method.to_string(),
            params,
            Some(Value::Number(id.into())),
        );
        
        debug!("Sending request: {} (id: {})", method, id);
        
        // Set up response channel
        let (response_tx, response_rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id, response_tx);
        }
        
        // Send request
        self.outbound_tx
            .send(Message::Request(request))
            .map_err(|_| RpcError::ProcessTerminated)?;
        
        // Wait for response
        let response = response_rx.await.map_err(|_| RpcError::ProcessTerminated)??;
        
        match response.error {
            Some(error) => Err(RpcError::Rpc {
                code: error.code,
                message: error.message,
            }),
            None => Ok(response.result.unwrap_or(Value::Null)),
        }
    }
    
    /// Send a request with timeout
    pub async fn request_timeout(
        &self,
        method: &str,
        params: Option<Value>,
        duration: Duration,
    ) -> RpcResult<Value> {
        timeout(duration, self.request(method, params))
            .await
            .map_err(|_| RpcError::Timeout)?
    }
    
    /// Send a notification (client functionality)
    pub async fn notify(&self, method: &str, params: Option<Value>) -> RpcResult<()> {
        let notification = Request::new(method.to_string(), params, None);
        
        debug!("Sending notification: {}", method);
        
        self.outbound_tx
            .send(Message::Request(notification))
            .map_err(|_| RpcError::ProcessTerminated)?;
        
        Ok(())
    }
    
    /// Send a response to a request (server functionality)
    pub async fn respond(&self, id: Value, result: Value) -> RpcResult<()> {
        let response = Response::success(result, id);
        
        debug!("Sending response: {:?}", response);
        
        self.outbound_tx
            .send(Message::Response(response))
            .map_err(|_| RpcError::ProcessTerminated)?;
        
        Ok(())
    }
    
    /// Send an error response (server functionality)
    pub async fn respond_error(&self, id: Value, error: ErrorObject) -> RpcResult<()> {
        let response = Response::error(error, id);
        
        debug!("Sending error response: {:?}", response);
        
        self.outbound_tx
            .send(Message::Response(response))
            .map_err(|_| RpcError::ProcessTerminated)?;
        
        Ok(())
    }
    
    /// Run the main event loop (server functionality)
    pub async fn run_main_loop(&self, mut main_handler: Box<dyn MainLoopHandler>) -> RpcResult<()> {
        debug!("Starting main event loop");
        
        main_handler.on_connected().await?;
        
        let tick_interval = Duration::from_millis(100); // 10 FPS
        let mut tick_timer = tokio::time::interval(tick_interval);
        
        loop {
            tokio::select! {
                // Handle tick
                _ = tick_timer.tick() => {
                    if !main_handler.on_tick().await? {
                        debug!("Main loop handler requested shutdown");
                        break;
                    }
                }
                
                // Handle incoming requests
                request = self.get_request() => {
                    if let Some(request) = request {
                        self.handle_incoming_request(request).await;
                    }
                }
                
                // Handle incoming notifications
                notification = self.get_notification() => {
                    if let Some(notification) = notification {
                        self.handle_incoming_notification(notification).await;
                    }
                }
            }
        }
        
        main_handler.on_disconnected().await?;
        debug!("Main event loop stopped");
        
        Ok(())
    }
    
    /// Get an incoming request (non-blocking)
    async fn get_request(&self) -> Option<Request> {
        let mut rx = self.inbound_request_rx.lock().await;
        rx.try_recv().ok()
    }
    
    /// Get an incoming notification (non-blocking)
    async fn get_notification(&self) -> Option<Request> {
        let mut rx = self.inbound_notification_rx.lock().await;
        rx.try_recv().ok()
    }
    
    /// Try to get an incoming request (non-blocking, public)
    pub async fn try_get_request(&self) -> Option<Request> {
        self.get_request().await
    }
    
    /// Try to get an incoming notification (non-blocking, public)
    pub async fn try_get_notification(&self) -> Option<Request> {
        self.get_notification().await
    }
    
    /// Handle an incoming request
    async fn handle_incoming_request(&self, request: Request) {
        let id = request.id.clone().unwrap_or(Value::Null);
        
        if let Some(handler) = &self.request_handler {
            match handler.handle_request(request).await {
                Ok(result) => {
                    let _ = self.respond(id, result).await;
                }
                Err(RpcError::MethodNotImplemented(method)) => {
                    let error = ErrorObject::new(ErrorCode::MethodNotFound, None);
                    let _ = self.respond_error(id, error).await;
                }
                Err(e) => {
                    let error = ErrorObject::custom(-32603, e.to_string(), None);
                    let _ = self.respond_error(id, error).await;
                }
            }
        } else {
            // No handler - method not found
            let error = ErrorObject::new(ErrorCode::MethodNotFound, None);
            let _ = self.respond_error(id, error).await;
        }
    }
    
    /// Handle an incoming notification
    async fn handle_incoming_notification(&self, notification: Request) {
        if let Some(handler) = &self.notification_handler {
            if let Err(e) = handler.handle_notification(notification).await {
                warn!("Notification handler error: {}", e);
            }
        } else {
            debug!("Received notification but no handler set: {}", notification.method);
        }
    }
    
    /// Gracefully shutdown
    pub async fn shutdown(mut self) -> RpcResult<()> {
        debug!("Shutting down JsonRpcBase");
        
        // Signal shutdown to stdin handler
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // Close outbound channel
        drop(self.outbound_tx);
        
        // Handle child process if any
        if let Some(mut child) = self.child.take() {
            match timeout(Duration::from_secs(5), child.wait()).await {
                Ok(status) => {
                    debug!("Process exited with status: {:?}", status);
                }
                Err(_) => {
                    warn!("Process didn't exit gracefully, killing");
                    child.kill().await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle stdin - send messages
    async fn stdin_handler(
        mut stdin: tokio::process::ChildStdin,
        mut outbound_rx: mpsc::UnboundedReceiver<Message>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        debug!("Starting stdin handler");
        
        loop {
            tokio::select! {
                message = outbound_rx.recv() => {
                    match message {
                        Some(msg) => {
                            if let Err(e) = Self::send_message(&mut stdin, &msg).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        None => {
                            debug!("Outbound channel closed");
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    debug!("Shutdown signal received");
                    break;
                }
            }
        }
        
        debug!("Stdin handler stopped");
    }
    
    /// Handle stdout - receive messages
    async fn stdout_handler(
        stdout: tokio::process::ChildStdout,
        pending_requests: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<RpcResult<Response>>>>>,
        inbound_request_tx: mpsc::UnboundedSender<Request>,
        inbound_notification_tx: mpsc::UnboundedSender<Request>,
    ) {
        debug!("Starting stdout handler");
        
        let mut reader = BufReader::new(stdout);
        let header_regex = Regex::new(r"^Content-Length:\s*(\d+)\s*$").unwrap();
        
        loop {
            // Read Content-Length header
            let mut header = String::new();
            match reader.read_line(&mut header).await {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    error!("Error reading header: {}", e);
                    break;
                }
            }
            
            let header = header.trim();
            if header.is_empty() {
                continue;
            }
            
            let content_length = if let Some(captures) = header_regex.captures(header) {
                captures[1].parse::<usize>().unwrap_or(0)
            } else {
                warn!("Invalid header: {}", header);
                continue;
            };
            
            // Read empty line
            let mut empty_line = String::new();
            if reader.read_line(&mut empty_line).await.is_err() {
                break;
            }
            
            // Read message body
            let mut buffer = vec![0u8; content_length];
            if reader.read_exact(&mut buffer).await.is_err() {
                break;
            }
            
            let message_str = String::from_utf8_lossy(&buffer);
            debug!("Received: {}", message_str);
            
            // Parse and route message
            match serde_json::from_str::<Message>(&message_str) {
                Ok(Message::Response(response)) => {
                    if let Value::Number(id_num) = &response.id {
                        if let Some(id) = id_num.as_u64() {
                            let mut pending = pending_requests.lock().await;
                            if let Some(sender) = pending.remove(&id) {
                                let _ = sender.send(Ok(response));
                            }
                        }
                    }
                }
                Ok(Message::Request(request)) => {
                    if request.is_notification() {
                        let _ = inbound_notification_tx.send(request);
                    } else {
                        let _ = inbound_request_tx.send(request);
                    }
                }
                Err(e) => {
                    error!("Failed to parse message: {}", e);
                }
            }
        }
        
        debug!("Stdout handler stopped");
    }
    
    /// Handle stderr - log errors
    async fn stderr_handler(stderr: tokio::process::ChildStderr) {
        debug!("Starting stderr handler");
        
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        
        while reader.read_line(&mut line).await.is_ok() {
            if line.trim().is_empty() {
                break;
            }
            warn!("Child stderr: {}", line.trim());
            line.clear();
        }
        
        debug!("Stderr handler stopped");
    }
    
    /// Send a message with LSP-style framing
    async fn send_message(
        stdin: &mut tokio::process::ChildStdin,
        message: &Message,
    ) -> RpcResult<()> {
        let json = serde_json::to_string(message)?;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        
        debug!("Sending: {}", frame);
        
        stdin.write_all(frame.as_bytes()).await?;
        stdin.flush().await?;
        
        Ok(())
    }
    
    /// Handle stdin for stdio mode
    async fn stdio_stdin_handler(
        mut outbound_rx: mpsc::UnboundedReceiver<Message>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        debug!("Starting stdio stdin handler");
        
        let mut stdout = tokio::io::stdout();
        
        loop {
            tokio::select! {
                message = outbound_rx.recv() => {
                    match message {
                        Some(msg) => {
                            if let Err(e) = Self::send_stdio_message(&mut stdout, &msg).await {
                                error!("Failed to send message: {}", e);
                                break;
                            }
                        }
                        None => {
                            debug!("Outbound channel closed");
                            break;
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    debug!("Shutdown signal received");
                    break;
                }
            }
        }
        
        debug!("Stdio stdin handler stopped");
    }
    
    /// Handle stdout for stdio mode
    async fn stdio_stdout_handler(
        pending_requests: Arc<tokio::sync::Mutex<HashMap<u64, oneshot::Sender<RpcResult<Response>>>>>,
        inbound_request_tx: mpsc::UnboundedSender<Request>,
        inbound_notification_tx: mpsc::UnboundedSender<Request>,
    ) {
        debug!("Starting stdio stdout handler");
        
        let mut reader = BufReader::new(tokio::io::stdin());
        let header_regex = Regex::new(r"^Content-Length:\s*(\d+)\s*$").unwrap();
        
        loop {
            // Read Content-Length header
            let mut header = String::new();
            match reader.read_line(&mut header).await {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(e) => {
                    error!("Error reading header: {}", e);
                    break;
                }
            }
            
            let header = header.trim();
            if header.is_empty() {
                continue;
            }
            
            let content_length = if let Some(captures) = header_regex.captures(header) {
                captures[1].parse::<usize>().unwrap_or(0)
            } else {
                warn!("Invalid header: {}", header);
                continue;
            };
            
            // Read empty line
            let mut empty_line = String::new();
            if reader.read_line(&mut empty_line).await.is_err() {
                break;
            }
            
            // Read message body
            let mut buffer = vec![0u8; content_length];
            if reader.read_exact(&mut buffer).await.is_err() {
                break;
            }
            
            let message_str = String::from_utf8_lossy(&buffer);
            debug!("Received: {}", message_str);
            
            // Parse and route message
            match serde_json::from_str::<Message>(&message_str) {
                Ok(Message::Response(response)) => {
                    if let Value::Number(id_num) = &response.id {
                        if let Some(id) = id_num.as_u64() {
                            let mut pending = pending_requests.lock().await;
                            if let Some(sender) = pending.remove(&id) {
                                let _ = sender.send(Ok(response));
                            }
                        }
                    }
                }
                Ok(Message::Request(request)) => {
                    if request.is_notification() {
                        let _ = inbound_notification_tx.send(request);
                    } else {
                        let _ = inbound_request_tx.send(request);
                    }
                }
                Err(e) => {
                    error!("Failed to parse message: {}", e);
                }
            }
        }
        
        debug!("Stdio stdout handler stopped");
    }
    
    /// Send a message via stdio
    async fn send_stdio_message(
        stdout: &mut tokio::io::Stdout,
        message: &Message,
    ) -> RpcResult<()> {
        let json = serde_json::to_string(message)?;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        
        debug!("Sending: {}", frame);
        
        stdout.write_all(frame.as_bytes()).await?;
        stdout.flush().await?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    // Example request handler
    struct EchoRequestHandler;
    
    #[async_trait]
    impl RequestHandler for EchoRequestHandler {
        async fn handle_request(&self, request: Request) -> RpcResult<Value> {
            match request.method.as_str() {
                "echo" => Ok(request.params.unwrap_or(Value::Null)),
                "ping" => Ok(json!("pong")),
                _ => Err(RpcError::MethodNotImplemented(request.method)),
            }
        }
    }
    
    // Example notification handler  
    struct LogNotificationHandler;
    
    #[async_trait]
    impl NotificationHandler for LogNotificationHandler {
        async fn handle_notification(&self, notification: Request) -> RpcResult<()> {
            debug!("Received notification: {} - {:?}", notification.method, notification.params);
            Ok(())
        }
    }
    
    // Example main loop handler
    struct SimpleMainLoopHandler {
        tick_count: u32,
    }
    
    impl SimpleMainLoopHandler {
        fn new() -> Self {
            Self { tick_count: 0 }
        }
    }
    
    #[async_trait]
    impl MainLoopHandler for SimpleMainLoopHandler {
        async fn on_tick(&mut self) -> RpcResult<bool> {
            self.tick_count += 1;
            debug!("Tick #{}", self.tick_count);
            
            // Stop after 10 ticks for testing
            Ok(self.tick_count < 10)
        }
        
        async fn on_connected(&mut self) -> RpcResult<()> {
            debug!("Connected!");
            Ok(())
        }
        
        async fn on_disconnected(&mut self) -> RpcResult<()> {
            debug!("Disconnected!");
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_handlers() {
        let _ = env_logger::try_init();
        
        let handler = EchoRequestHandler;
        let request = Request::new("echo".to_string(), Some(json!("test")), Some(json!(1)));
        
        let result = handler.handle_request(request).await.unwrap();
        assert_eq!(result, json!("test"));
    }
}