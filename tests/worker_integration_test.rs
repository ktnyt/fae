use fae::workers::{
    WorkerManager, SearchHandler, ContentSearcher,
    Message, WorkerMessage, SearchHandlerMessage, TuiMessage
};
use tokio::time::Duration;
use tempfile::TempDir;
use std::fs::File;
use std::io::Write;
use anyhow::Result;

/// 基本的なワーカー間メッセージング動作テスト
#[tokio::test]
async fn test_basic_worker_messaging() -> Result<()> {
    let temp_dir = create_minimal_test_project()?;
    let project_root = temp_dir.path().to_path_buf();
    
    // ワーカーマネージャーを作成
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    // SearchHandlerを追加
    let mut search_handler = SearchHandler::new("search_handler".to_string());
    search_handler.set_message_bus(message_bus.clone());
    manager.add_worker(search_handler).await?;
    
    // ContentSearcherを追加
    let mut content_searcher = ContentSearcher::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearcher: {}", e))?;
    content_searcher.set_message_bus(message_bus.clone());
    manager.add_worker(content_searcher).await?;
    
    // 初期化を待機
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // TUIからSearchHandlerへクエリメッセージを送信
    let query_message = WorkerMessage::tui_query("test".to_string());
    let msg: Message = query_message.into();
    
    {
        let bus_guard = message_bus.read().await;
        bus_guard.send_to("search_handler", msg)?;
    }
    
    // 短時間待機して処理を完了させる
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    println!("✅ Basic worker messaging test completed successfully");
    
    manager.shutdown_all().await?;
    Ok(())
}

/// メッセージバスの送信テスト
#[tokio::test]
async fn test_message_bus_broadcasting() -> Result<()> {
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    // テストメッセージを作成
    let test_message = Message {
        method: "test/method".to_string(),
        payload: serde_json::json!({"test": "data"}),
        correlation_id: Some("test-123".to_string()),
    };
    
    // 存在しないワーカーに送信（エラーハンドリングテスト）
    {
        let bus_guard = message_bus.read().await;
        let result = bus_guard.send_to("nonexistent_worker", test_message.clone());
        assert!(result.is_err(), "存在しないワーカーへの送信はエラーになるべき");
    }
    
    // ブロードキャスト（ワーカーがいない状態）
    {
        let bus_guard = message_bus.read().await;
        bus_guard.broadcast(test_message); // ブロードキャストは常に()を返す
        println!("✅ Broadcast completed without error");
    }
    
    println!("✅ Message bus broadcasting test passed");
    
    Ok(())
}

/// ワーカーのライフサイクルテスト
#[tokio::test]
async fn test_worker_lifecycle() -> Result<()> {
    let temp_dir = create_minimal_test_project()?;
    let project_root = temp_dir.path().to_path_buf();
    
    let mut manager = WorkerManager::new();
    
    // SearchHandlerの追加とテスト
    let mut search_handler = SearchHandler::new("search_handler".to_string());
    search_handler.set_message_bus(manager.get_message_bus());
    
    // ワーカーを追加（initialization含む）
    manager.add_worker(search_handler).await?;
    println!("✅ SearchHandler worker added and initialized");
    
    // ContentSearcherの追加とテスト
    let mut content_searcher = ContentSearcher::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearcher: {}", e))?;
    content_searcher.set_message_bus(manager.get_message_bus());
    
    manager.add_worker(content_searcher).await?;
    println!("✅ ContentSearcher worker added and initialized");
    
    // 少し待機
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // すべてのワーカーをシャットダウン（cleanup含む）
    manager.shutdown_all().await?;
    println!("✅ All workers shut down successfully");
    
    Ok(())
}

/// プロトコルメッセージの型安全性テスト
#[tokio::test]
async fn test_protocol_type_safety() -> Result<()> {
    // TuiMessage
    let tui_msg = TuiMessage::UserQuery { query: "search_term".to_string() };
    let worker_msg = WorkerMessage::Tui(tui_msg);
    let message: Message = worker_msg.into();
    
    // メッセージからWorkerMessageへの変換
    let recovered = WorkerMessage::try_from(message)?;
    
    match recovered {
        WorkerMessage::Tui(TuiMessage::UserQuery { query }) => {
            assert_eq!(query, "search_term");
            println!("✅ TuiMessage type safety verified");
        }
        _ => panic!("Wrong message type recovered"),
    }
    
    // SearchHandlerMessage
    let search_msg = SearchHandlerMessage::SearchMatch {
        filename: "test.rs".to_string(),
        line: 42,
        column: 10,
        content: "fn test()".to_string(),
    };
    let worker_msg = WorkerMessage::SearchHandler(search_msg);
    let message: Message = worker_msg.into();
    let recovered = WorkerMessage::try_from(message)?;
    
    match recovered {
        WorkerMessage::SearchHandler(SearchHandlerMessage::SearchMatch { filename, line, column, content }) => {
            assert_eq!(filename, "test.rs");
            assert_eq!(line, 42);
            assert_eq!(column, 10);
            assert_eq!(content, "fn test()");
            println!("✅ SearchHandlerMessage type safety verified");
        }
        _ => panic!("Wrong message type recovered"),
    }
    
    Ok(())
}

/// 複数クエリでのワーカー動作テスト
#[tokio::test]
async fn test_multiple_queries() -> Result<()> {
    let temp_dir = create_test_project_with_multiple_files()?;
    let project_root = temp_dir.path().to_path_buf();
    
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    // ワーカーを追加
    let mut search_handler = SearchHandler::new("search_handler".to_string());
    search_handler.set_message_bus(message_bus.clone());
    manager.add_worker(search_handler).await?;
    
    let mut content_searcher = ContentSearcher::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearcher: {}", e))?;
    content_searcher.set_message_bus(message_bus.clone());
    manager.add_worker(content_searcher).await?;
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // 複数のクエリを順次送信
    let queries = vec!["function", "class", "console"];
    
    for query in queries {
        let query_message = WorkerMessage::tui_query(query.to_string());
        let msg: Message = query_message.into();
        
        {
            let bus_guard = message_bus.read().await;
            bus_guard.send_to("search_handler", msg)?;
        }
        
        // 各クエリ間で短時間待機
        tokio::time::sleep(Duration::from_millis(100)).await;
        println!("📝 Sent query: '{}'", query);
    }
    
    // 最終処理の完了を待機
    tokio::time::sleep(Duration::from_millis(300)).await;
    
    println!("✅ Multiple queries test completed");
    
    manager.shutdown_all().await?;
    Ok(())
}

/// 最小限のテストプロジェクト作成
fn create_minimal_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    let mut file = File::create(root.join("simple.txt"))?;
    writeln!(file, "This is a test file.")?;
    writeln!(file, "It contains some test content.")?;

    Ok(temp_dir)
}

/// 複数ファイルのテストプロジェクト作成
fn create_test_project_with_multiple_files() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // JavaScript file
    let mut js_file = File::create(root.join("app.js"))?;
    writeln!(js_file, "function initialize() {{")?;
    writeln!(js_file, "    console.log('App initialized');")?;
    writeln!(js_file, "}}")?;
    writeln!(js_file, "")?;
    writeln!(js_file, "class Application {{")?;
    writeln!(js_file, "    start() {{")?;
    writeln!(js_file, "        console.log('Starting app');")?;
    writeln!(js_file, "    }}")?;
    writeln!(js_file, "}}")?;

    // Python file
    let mut py_file = File::create(root.join("script.py"))?;
    writeln!(py_file, "def main():")?;
    writeln!(py_file, "    print('Hello from Python')")?;
    writeln!(py_file, "")?;
    writeln!(py_file, "class Helper:")?;
    writeln!(py_file, "    def function(self):")?;
    writeln!(py_file, "        console.log('helper function')")?;

    // Text file
    let mut txt_file = File::create(root.join("readme.txt"))?;
    writeln!(txt_file, "This is a readme file.")?;
    writeln!(txt_file, "function documentation here")?;
    writeln!(txt_file, "class descriptions")?;
    writeln!(txt_file, "console output examples")?;

    Ok(temp_dir)
}