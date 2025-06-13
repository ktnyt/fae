//! SearchRouter Binary
//! 
//! TUIとワーカー間のメッセージルーティングを行うプロセス
//! JSON-RPC stdio通信でクエリをルーティングし、結果を転送

use fae::jsonrpc::{JsonRpcBase};
use fae::workers::search_router::SearchRouterHandler;
use log::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    info!("🚀 Starting SearchRouter process");
    
    // Create JSON-RPC base for stdio communication
    let rpc_base = JsonRpcBase::new_stdio().await?;
    
    // Create search router handler
    let handler = SearchRouterHandler::new();
    
    // Run the main event loop
    match rpc_base.run_main_loop(Box::new(handler)).await {
        Ok(()) => {
            info!("✅ SearchRouter stopped gracefully");
        }
        Err(e) => {
            error!("❌ SearchRouter error: {}", e);
            return Err(e.into());
        }
    }
    
    info!("🎉 SearchRouter shutdown complete");
    Ok(())
}