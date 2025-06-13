use fae::cli;
use std::process;
use log::error;

#[tokio::main]
async fn main() {
    // ログ初期化（環境変数 RUST_LOG で制御）
    env_logger::init();
    
    if let Err(err) = cli::run_cli().await {
        error!("CLI execution failed: {}", err);
        
        // エラーチェーンをログに記録
        let mut source = err.source();
        while let Some(err) = source {
            error!("Caused by: {}", err);
            source = err.source();
        }
        
        // ユーザー向けエラーメッセージ
        eprintln!("Error: {}", err);
        process::exit(1);
    }
}