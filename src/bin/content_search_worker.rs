//! ContentSearchWorker Binary
//! 
//! ripgrepベースのテキスト検索ワーカープロセス
//! JSON-RPC stdio通信でクエリを受信し、検索結果を返す

use fae::jsonrpc::{JsonRpcBase};
use fae::workers::content_search_worker::ContentSearchHandler;
use log::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🚀 Starting ContentSearchWorker process");
    
    // Create JSON-RPC base for stdio communication
    let rpc_base = JsonRpcBase::new_stdio().await?;
    
    // Create content search handler
    let handler = ContentSearchHandler::new();
    
    // Run the main event loop
    match rpc_base.run_main_loop(Box::new(handler)).await {
        Ok(()) => {
            info!("✅ ContentSearchWorker stopped gracefully");
        }
        Err(e) => {
            error!("❌ ContentSearchWorker error: {}", e);
            return Err(e.into());
        }
    }
    
    info!("🎉 ContentSearchWorker shutdown complete");
    Ok(())
}