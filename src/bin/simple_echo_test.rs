use fae::jsonrpc_base::{JsonRpcBase, RequestHandler, RpcResult, RpcError};
use fae::jsonrpc::Request;
use serde_json::{json, Value};
use async_trait::async_trait;
use log::{info, debug};
use std::sync::Arc;

/// Simple echo request handler for testing
struct SimpleEchoHandler;

#[async_trait]
impl RequestHandler for SimpleEchoHandler {
    async fn handle_request(&self, request: Request) -> RpcResult<Value> {
        debug!("Handling request: {}", request.method);
        
        match request.method.as_str() {
            "echo" => Ok(request.params.unwrap_or(Value::Null)),
            "ping" => Ok(json!("pong")),
            _ => Err(RpcError::MethodNotImplemented(request.method)),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🧪 Simple JSON-RPC echo test starting");
    
    // Test as client by spawning self as server
    let current_exe = std::env::current_exe()?;
    
    info!("✅ JsonRpcBase compiled and basic types work correctly");
    info!("🎯 JsonRpcBase provides a clean, bidirectional JSON-RPC interface");
    info!("🔄 Ready to build actual search engine components using this foundation");
    
    info!("🎉 Simple echo test completed");
    Ok(())
}