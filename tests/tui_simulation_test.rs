use fae::workers::{
    WorkerManager, SearchHandlerWorker, ContentSearchWorker, SimpleTuiWorker,
    Message, MessageBus, WorkerMessage, SearchHandlerMessage, TuiMessage
};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{timeout, Duration};
use tempfile::TempDir;
use std::fs::File;
use std::io::Write;
use anyhow::Result;

/// TUIの動作をシミュレートするテストプログラム
/// 実際のTUI入力の代わりにプログラマティックにメッセージを送信してワーカー間の連携を検証
#[tokio::test]
async fn test_tui_worker_simulation() -> Result<()> {
    // テスト用プロジェクト作成
    let temp_dir = create_test_project()?;
    let project_root = temp_dir.path().to_path_buf();
    
    // ワーカーマネージャーを作成
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    // SearchHandlerWorkerワーカーを追加（TUIの代わりに直接メッセージを受信）
    let mut search_handler = SearchHandlerWorker::new("search_handler".to_string());
    search_handler.set_message_bus(message_bus.clone());
    manager.add_worker(search_handler).await?;
    
    // ContentSearchWorkerワーカーを追加
    let mut content_searcher = ContentSearchWorker::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearchWorker: {}", e))?;
    content_searcher.set_message_bus(message_bus.clone());
    manager.add_worker(content_searcher).await?;

    // テストシミュレーター（TUIの代わり）を作成
    let simulator = TuiSimulator::new("tui".to_string(), message_bus.clone());
    manager.add_worker(simulator.clone()).await?;
    
    // 短時間待機してワーカーが初期化されるのを確認
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // TUIシミュレーション: ユーザーがクエリを入力
    println!("🧪 Testing TUI simulation: sending search query");
    simulator.send_user_query("testFunction").await?;
    
    // 結果を待機
    let results = timeout(Duration::from_secs(3), simulator.wait_for_results()).await??;
    
    // 結果の検証
    assert!(!results.is_empty(), "検索結果が返されるべき");
    println!("✅ TUI simulation test passed! Received {} results", results.len());
    
    // 結果内容の検証
    let has_test_function = results.iter().any(|r| r.contains("testFunction"));
    assert!(has_test_function, "testFunctionを含む結果があるべき");
    
    // 別のクエリをテスト
    simulator.send_user_query("console.log").await?;
    let results2 = timeout(Duration::from_secs(3), simulator.wait_for_results()).await??;
    
    assert!(!results2.is_empty(), "2つ目の検索結果が返されるべき");
    println!("✅ Second query test passed! Received {} results", results2.len());
    
    // ワーカーシステムをシャットダウン
    manager.shutdown_all().await?;
    
    Ok(())
}

/// TUIワーカーとの統合テスト（実際のTUIワーカーを使用）
#[tokio::test]
async fn test_with_real_tui_worker() -> Result<()> {
    // テスト用プロジェクト作成
    let temp_dir = create_test_project()?;
    let project_root = temp_dir.path().to_path_buf();
    
    // ワーカーマネージャーを作成
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    // 実際のTUIワーカーを追加（但し、ターミナル初期化はスキップ）
    let mut tui_worker = SimpleTuiWorker::new("tui".to_string());
    tui_worker.set_message_bus(message_bus.clone());
    
    // SearchHandlerWorkerワーカーを追加
    let mut search_handler = SearchHandlerWorker::new("search_handler".to_string());
    search_handler.set_message_bus(message_bus.clone());
    manager.add_worker(search_handler).await?;
    
    // ContentSearchWorkerワーカーを追加
    let mut content_searcher = ContentSearchWorker::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearchWorker: {}", e))?;
    content_searcher.set_message_bus(message_bus.clone());
    manager.add_worker(content_searcher).await?;
    
    // TUIワーカーにメッセージを直接送信してレスポンスをテスト
    let query_message = WorkerMessage::tui_query("testFunction".to_string());
    let msg: Message = query_message.into();
    
    {
        let bus_guard = message_bus.read().await;
        bus_guard.send_to("search_handler", msg)?;
    }
    
    // 短時間待機
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    println!("✅ Real TUI worker integration test completed without errors");
    
    // ワーカーシステムをシャットダウン
    manager.shutdown_all().await?;
    
    Ok(())
}

/// メッセージプロトコルのシリアライゼーションテスト
#[tokio::test]
async fn test_message_protocol() -> Result<()> {
    // TuiMessageのシリアライゼーション
    let tui_msg = TuiMessage::UserQuery { query: "test_query".to_string() };
    let worker_msg = WorkerMessage::Tui(tui_msg);
    let message: Message = worker_msg.into();
    
    // JSON変換
    let json_str = serde_json::to_string(&message)?;
    println!("📜 Serialized message: {}", json_str);
    
    // デシリアライゼーション
    let deserialized: Message = serde_json::from_str(&json_str)?;
    assert_eq!(message.method, deserialized.method);
    
    // WorkerMessageに戻す
    let recovered_worker_msg = WorkerMessage::try_from(deserialized)?;
    
    if let WorkerMessage::Tui(TuiMessage::UserQuery { query }) = recovered_worker_msg {
        assert_eq!(query, "test_query");
        println!("✅ Message protocol serialization test passed");
    } else {
        panic!("Message deserialization failed");
    }
    
    Ok(())
}

/// キャンセレーションテスト
#[tokio::test]
async fn test_search_cancellation() -> Result<()> {
    let temp_dir = create_test_project()?;
    let project_root = temp_dir.path().to_path_buf();
    
    let mut manager = WorkerManager::new();
    let message_bus = manager.get_message_bus();
    
    let mut search_handler = SearchHandlerWorker::new("search_handler".to_string());
    search_handler.set_message_bus(message_bus.clone());
    manager.add_worker(search_handler).await?;
    
    let mut content_searcher = ContentSearchWorker::new(
        "content_searcher".to_string(),
        "search_handler".to_string(),
        &project_root,
    ).map_err(|e| anyhow::anyhow!("Failed to create ContentSearchWorker: {}", e))?;
    content_searcher.set_message_bus(message_bus.clone());
    manager.add_worker(content_searcher).await?;
    
    // 長時間検索を開始
    let query_message = WorkerMessage::tui_query("test".to_string());
    let msg: Message = query_message.into();
    
    {
        let bus_guard = message_bus.read().await;
        bus_guard.send_to("search_handler", msg)?;
    }
    
    // 短時間後にキャンセル（新しいクエリを送信）
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    let cancel_message = WorkerMessage::tui_query("different".to_string());
    let cancel_msg: Message = cancel_message.into();
    
    {
        let bus_guard = message_bus.read().await;
        bus_guard.send_to("search_handler", cancel_msg)?;
    }
    
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    println!("✅ Search cancellation test completed");
    
    manager.shutdown_all().await?;
    Ok(())
}

/// TUIシミュレーター: 実際のTUIの代わりにプログラマティックにメッセージを送受信
#[derive(Clone)]
struct TuiSimulator {
    worker_id: String,
    message_bus: Option<Arc<RwLock<MessageBus>>>,
    result_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<String>>>>,
    result_sender: Option<mpsc::UnboundedSender<String>>,
}

impl TuiSimulator {
    fn new(worker_id: String, message_bus: Arc<RwLock<MessageBus>>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            worker_id,
            message_bus: Some(message_bus),
            result_receiver: Arc::new(RwLock::new(Some(receiver))),
            result_sender: Some(sender),
        }
    }
    
    async fn send_user_query(&self, query: &str) -> Result<()> {
        if let Some(bus) = &self.message_bus {
            let message = WorkerMessage::tui_query(query.to_string());
            let msg: Message = message.into();
            
            let bus_guard = bus.read().await;
            bus_guard.send_to("search_handler", msg)?;
        }
        Ok(())
    }
    
    async fn wait_for_results(&self) -> Result<Vec<String>> {
        let mut results = Vec::new();
        
        // レシーバーを取得
        let receiver_opt = {
            let mut guard = self.result_receiver.write().await;
            guard.take()
        };
        
        if let Some(mut receiver) = receiver_opt {
            // タイムアウト付きで結果を収集
            let timeout_duration = Duration::from_millis(500);
            let start = tokio::time::Instant::now();
            
            while start.elapsed() < timeout_duration {
                match timeout(Duration::from_millis(10), receiver.recv()).await {
                    Ok(Some(result)) => {
                        results.push(result);
                    }
                    Ok(None) => break, // チャンネルクローズ
                    Err(_) => {
                        // タイムアウト、少し待ってから再試行
                        if results.is_empty() {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                            continue;
                        } else {
                            break; // 結果があるので終了
                        }
                    }
                }
            }
            
            // レシーバーを戻す
            let mut guard = self.result_receiver.write().await;
            *guard = Some(receiver);
        }
        
        Ok(results)
    }
}

#[async_trait::async_trait]
impl fae::workers::Worker for TuiSimulator {
    fn worker_id(&self) -> &str {
        &self.worker_id
    }
    
    async fn initialize(&mut self) -> Result<(), fae::workers::worker::WorkerError> {
        println!("🚀 TUI Simulator initialized");
        Ok(())
    }
    
    async fn handle_message(&mut self, message: Message) -> Result<(), fae::workers::worker::WorkerError> {
        if let Ok(worker_msg) = WorkerMessage::try_from(message) {
            match worker_msg {
                WorkerMessage::SearchHandler(SearchHandlerMessage::SearchMatch { filename, line, column, content }) => {
                    let result = format!("{}:{}:{} {}", filename, line, column, content);
                    if let Some(sender) = &self.result_sender {
                        let _ = sender.send(result);
                    }
                }
                WorkerMessage::SearchHandler(SearchHandlerMessage::SearchClear) => {
                    println!("🧹 Search cleared");
                }
                WorkerMessage::SearchHandler(SearchHandlerMessage::IndexProgress { indexed_files, total_files, symbols, elapsed }) => {
                    println!("📊 Index progress: {}/{} files, {} symbols, {}ms", indexed_files, total_files, symbols, elapsed);
                }
                _ => {
                    // 他のメッセージタイプは無視
                }
            }
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), fae::workers::worker::WorkerError> {
        println!("🧹 TUI Simulator cleaned up");
        Ok(())
    }
}

/// テスト用プロジェクトの作成
fn create_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path();

    // TypeScriptファイル
    let mut ts_file = File::create(root.join("test.ts"))?;
    writeln!(ts_file, "function testFunction() {{")?;
    writeln!(ts_file, "    console.log('Hello from test');")?;
    writeln!(ts_file, "    return 'test result';")?;
    writeln!(ts_file, "}}")?;
    writeln!(ts_file, "")?;
    writeln!(ts_file, "export class TestClass {{")?;
    writeln!(ts_file, "    method() {{")?;
    writeln!(ts_file, "        console.log('method called');")?;
    writeln!(ts_file, "    }}")?;
    writeln!(ts_file, "}}")?;

    // Rustファイル
    let mut rs_file = File::create(root.join("main.rs"))?;
    writeln!(rs_file, "fn main() {{")?;
    writeln!(rs_file, "    println!(\"Hello, world!\");")?;
    writeln!(rs_file, "}}")?;
    writeln!(rs_file, "")?;
    writeln!(rs_file, "fn testFunction() {{")?;
    writeln!(rs_file, "    println!(\"test function\");")?;
    writeln!(rs_file, "}}")?;

    // サブディレクトリとファイル
    std::fs::create_dir_all(root.join("src"))?;
    let mut mod_file = File::create(root.join("src").join("lib.rs"))?;
    writeln!(mod_file, "pub fn library_function() {{")?;
    writeln!(mod_file, "    // Library implementation")?;
    writeln!(mod_file, "    console.log('library');")?;
    writeln!(mod_file, "}}")?;

    Ok(temp_dir)
}