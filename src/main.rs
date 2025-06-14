use clap::{Parser, Subcommand};
use fae::jsonrpc::stdio::JsonRpcStdioAdapter;
use fae::services::service_factory::ServiceType;
use std::path::PathBuf;
use std::process;

/// fae サービスランナー - JSON-RPC マイクロサービスアーキテクチャ
#[derive(Parser, Debug)]
#[command(name = "fae-service")]
#[command(author, version, about = "fae JSON-RPC microservices runner")]
struct Args {
    /// サービスコマンド
    #[command(subcommand)]
    command: Option<Commands>,

    /// 検索対象のルートディレクトリ
    #[arg(short, long, default_value = ".", global = true)]
    root: PathBuf,

    /// ログレベル
    #[arg(short, long, default_value = "info", global = true)]
    log_level: String,

    /// ripgrepバイナリのパス（デフォルトはPATHから検索）
    #[arg(long, global = true)]
    ripgrep_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// サービスを起動
    #[command(name = "start")]
    Start {
        /// サービスタイプ (e.g., search:literal)
        service: String,
    },
    /// 利用可能なサービス一覧を表示
    #[command(name = "list")]
    List,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // ログ初期化
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    // コマンド処理
    match args.command {
        Some(Commands::Start { ref service }) => {
            log::info!("Starting fae service: {}", service);
            log::info!("Search root: {}", args.root.display());
            
            // サービスタイプを解析
            let service_type = match ServiceType::from_str(&service) {
                Ok(st) => st,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    eprintln!("{}", fae::services::service_factory::ServiceFactory::list_services());
                    process::exit(1);
                }
            };
            
            if let Err(e) = start_service(service_type, &args).await {
                eprintln!("Service error: {}", e);
                process::exit(1);
            }
        }
        Some(Commands::List) => {
            println!("{}", fae::services::service_factory::ServiceFactory::list_services());
            return;
        }
        None => {
            eprintln!("Error: No command specified.");
            eprintln!("\nUsage examples:");
            eprintln!("  fae-service start search:literal");
            eprintln!("  fae-service list");
            eprintln!("\nFor more help: fae-service --help");
            process::exit(1);
        }
    }

    log::info!("Service terminated");
}

async fn start_service(service_type: ServiceType, args: &Args) -> Result<(), Box<dyn std::error::Error>> {

    // 検索ルートディレクトリの存在確認
    if !args.root.exists() {
        return Err(format!("Search root directory does not exist: {}", args.root.display()).into());
    }

    if !args.root.is_dir() {
        return Err(format!("Search root is not a directory: {}", args.root.display()).into());
    }

    // サービス固有の依存関係チェック
    match service_type {
        ServiceType::LiteralSearch => {
            // ripgrep の存在確認
            if let Err(e) = check_ripgrep_availability(args.ripgrep_path.as_ref()).await {
                return Err(format!("ripgrep is not available: {}. Please install ripgrep (https://github.com/BurntSushi/ripgrep)", e).into());
            }
            log::info!("ripgrep is available");
        }
        // 将来のサービスの依存関係チェックを追加
    }

    // サービス固有のサーバー起動
    match service_type {
        ServiceType::LiteralSearch => {
            start_jsonrpc_server_with_literal_search(args.root.clone()).await
        }
        // 将来のサービス追加時はここにマッチを追加
    }
}

async fn check_ripgrep_availability(
    ripgrep_path: Option<&PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let cmd_name = ripgrep_path
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "rg".to_string());

    let output = tokio::process::Command::new(&cmd_name)
        .arg("--version")
        .output()
        .await?;

    if !output.status.success() {
        return Err(format!("ripgrep command '{}' failed", cmd_name).into());
    }

    let version = String::from_utf8_lossy(&output.stdout);
    log::debug!("ripgrep version: {}", version.trim());

    Ok(())
}

async fn start_jsonrpc_server_with_literal_search(search_root: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::sync::mpsc;
    use fae::jsonrpc::message::JsonRpcPayload;
    
    // 通知チャンネルを先に作成
    let (notification_tx, notification_rx) = mpsc::unbounded_channel::<JsonRpcPayload>();
    
    // ハンドラーを作成（通知チャンネル付き）
    let handler = fae::services::literal_search::LiteralSearchHandler::new(search_root)
        .await
        .with_notification_sender(notification_tx);
    
    // JSON-RPC Stdio Adapter を作成
    let mut adapter = JsonRpcStdioAdapter::new(handler);
    
    // 通知転送タスクを起動
    let engine_sender = adapter.engine().notification_sender();
    tokio::spawn(async move {
        let mut receiver = notification_rx;
        while let Some(payload) = receiver.recv().await {
            if let Err(e) = engine_sender.send(payload) {
                log::error!("Failed to forward notification to engine: {}", e);
                break;
            }
        }
        log::debug!("Notification forwarding task terminated");
    });

    log::info!("JSON-RPC server ready, listening on stdin/stdout");

    // JSON-RPC サーバーを実行
    let server_result = adapter.run().await;

    server_result.map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ripgrep_availability() {
        // ログ初期化
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();

        // ripgrepが利用可能な場合のテスト
        if tokio::process::Command::new("rg")
            .arg("--version")
            .output()
            .await
            .is_ok()
        {
            assert!(check_ripgrep_availability(None).await.is_ok());
        }

        // 存在しないコマンドのテスト
        let result = check_ripgrep_availability(Some(&PathBuf::from("nonexistent_command"))).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_command_parsing() {
        // startコマンドのテスト
        let args = Args::try_parse_from(&["fae-service", "start", "search:literal"]).unwrap();
        assert_eq!(args.root, PathBuf::from("."));
        assert_eq!(args.log_level, "info");
        assert!(args.ripgrep_path.is_none());
        
        match &args.command {
            Some(Commands::Start { service }) => {
                assert_eq!(service, "search:literal");
            }
            _ => panic!("Expected Start command"),
        }

        // listコマンドのテスト
        let args = Args::try_parse_from(&["fae-service", "list"]).unwrap();
        match &args.command {
            Some(Commands::List) => {}
            _ => panic!("Expected List command"),
        }

        // カスタム引数でstartコマンド
        let args = Args::try_parse_from(&[
            "fae-service",
            "--root",
            "/tmp",
            "--log-level",
            "debug",
            "--ripgrep-path",
            "/usr/local/bin/rg",
            "start",
            "search:literal",
        ])
        .unwrap();
        assert_eq!(args.root, PathBuf::from("/tmp"));
        assert_eq!(args.log_level, "debug");
        assert_eq!(args.ripgrep_path, Some(PathBuf::from("/usr/local/bin/rg")));
        
        match &args.command {
            Some(Commands::Start { service }) => {
                assert_eq!(service, "search:literal");
            }
            _ => panic!("Expected Start command"),
        }
    }
}
