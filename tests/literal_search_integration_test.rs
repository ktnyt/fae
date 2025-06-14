use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde_json::{json, Value};

/// JSON-RPC クライアント (Python test_client.py の Rust 版)
struct JsonRpcTestClient {
    process: Child,
    next_id: u64,
    result_count: Arc<Mutex<u32>>,
    received_messages: Arc<Mutex<Vec<Value>>>,
}

impl JsonRpcTestClient {
    /// 新しいクライアントを作成してサーバーを起動
    fn new() -> std::io::Result<Self> {
        let process = Command::new("cargo")
            .args(&["run", "--bin", "fae-service", "--", "start", "search:literal", "--log-level", "info"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        Ok(Self {
            process,
            next_id: 1,
            result_count: Arc::new(Mutex::new(0)),
            received_messages: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 通知を送信
    fn send_notification(&mut self, method: &str, params: Option<Value>) -> std::io::Result<()> {
        let mut message = json!({
            "jsonrpc": "2.0",
            "method": method
        });
        
        if let Some(params) = params {
            message["params"] = params;
        }

        self.send_message(message)
    }

    /// リクエストを送信
    fn send_request(&mut self, method: &str, params: Option<Value>) -> std::io::Result<u64> {
        let id = self.next_id;
        self.next_id += 1;

        let mut message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method
        });
        
        if let Some(params) = params {
            message["params"] = params;
        }

        self.send_message(message)?;
        Ok(id)
    }

    /// JSON-RPC メッセージを送信
    fn send_message(&mut self, message: Value) -> std::io::Result<()> {
        let json_str = serde_json::to_string(&message)?;
        let content_length = json_str.len();
        let full_message = format!("Content-Length: {}\r\n\r\n{}", content_length, json_str);

        if let Some(stdin) = &mut self.process.stdin {
            stdin.write_all(full_message.as_bytes())?;
            stdin.flush()?;
        }

        Ok(())
    }

    /// サーバーからのメッセージを読み取り処理
    fn start_message_reader(&mut self) -> std::io::Result<()> {
        let stdout = self.process.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let result_count = Arc::clone(&self.result_count);
        let received_messages = Arc::clone(&self.received_messages);

        thread::spawn(move || {
            Self::read_messages_loop(reader, result_count, received_messages);
        });

        Ok(())
    }

    /// メッセージ読み取りループ
    fn read_messages_loop(
        mut reader: BufReader<std::process::ChildStdout>,
        result_count: Arc<Mutex<u32>>,
        received_messages: Arc<Mutex<Vec<Value>>>,
    ) {
        let mut line = String::new();
        
        loop {
            line.clear();
            
            // Content-Length ヘッダーを読み取り
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            
            let header_line = line.trim();
            if !header_line.starts_with("Content-Length:") {
                continue;
            }
            
            let content_length: usize = header_line
                .strip_prefix("Content-Length:")
                .unwrap()
                .trim()
                .parse()
                .unwrap_or(0);
            
            if content_length == 0 {
                continue;
            }
            
            // 空行をスキップ
            line.clear();
            reader.read_line(&mut line).unwrap_or(0);
            
            // JSON メッセージを読み取り
            let mut json_buffer = vec![0u8; content_length];
            std::io::Read::read_exact(&mut reader, &mut json_buffer).unwrap_or(());
            
            if let Ok(json_str) = String::from_utf8(json_buffer) {
                if let Ok(message) = serde_json::from_str::<Value>(&json_str) {
                    Self::handle_message(&message, &result_count);
                    received_messages.lock().unwrap().push(message);
                }
            }
        }
    }

    /// 受信メッセージの処理
    fn handle_message(message: &Value, result_count: &Arc<Mutex<u32>>) {
        if let Some(method) = message.get("method").and_then(|m| m.as_str()) {
            match method {
                "clearSearchResults" => {
                    *result_count.lock().unwrap() = 0;
                }
                "pushSearchResult" => {
                    *result_count.lock().unwrap() += 1;
                }
                _ => {}
            }
        }
    }

    /// 結果数を取得
    fn get_result_count(&self) -> u32 {
        *self.result_count.lock().unwrap()
    }

    /// 受信したメッセージを取得
    fn get_received_messages(&self) -> Vec<Value> {
        self.received_messages.lock().unwrap().clone()
    }

    /// クライアントを終了
    fn close(&mut self) -> std::io::Result<()> {
        if let Some(stdin) = &mut self.process.stdin {
            stdin.flush()?;
        }
        
        self.process.kill()?;
        self.process.wait()?;
        Ok(())
    }
}

impl Drop for JsonRpcTestClient {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// test_client.py と同等の統合テスト
#[tokio::test]
async fn test_literal_search_integration() {
    // ログ初期化
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();

    // クライアントを作成
    let mut client = JsonRpcTestClient::new().expect("Failed to start client");
    
    // メッセージリーダーを開始
    client.start_message_reader().expect("Failed to start message reader");
    
    // サーバー起動を待機
    thread::sleep(Duration::from_millis(1000));
    
    // ステータス確認
    let _status_id = client.send_request("search.status", None).expect("Failed to send status request");
    thread::sleep(Duration::from_millis(500));
    
    // 検索実行
    let search_query = "function";
    client.send_notification("updateQuery", Some(json!({"query": search_query}))).expect("Failed to send search query");
    
    // 検索結果を待機
    thread::sleep(Duration::from_secs(3));
    
    // 結果を検証
    let result_count = client.get_result_count();
    let messages = client.get_received_messages();
    
    println!("✨ Search completed! Found {} results.", result_count);
    println!("📨 Total messages received: {}", messages.len());
    
    // 基本的な検証
    assert!(result_count > 0, "Result count should be non-negative");
    assert!(!messages.is_empty(), "Should receive at least some messages");
    
    // 通知の種類を確認
    let mut has_clear = false;
    let mut has_results = false;
    let mut has_completion = false;
    
    for message in &messages {
        if let Some(method) = message.get("method").and_then(|m| m.as_str()) {
            match method {
                "clearSearchResults" => has_clear = true,
                "pushSearchResult" => has_results = true,
                "searchCompleted" => has_completion = true,
                _ => {}
            }
        }
    }
    
    assert!(has_clear, "Should receive clearSearchResults notification");
    if result_count > 0 {
        assert!(has_results, "Should receive pushSearchResult notifications when results found");
    }
    assert!(has_completion, "Should receive searchCompleted notification");
    
    println!("🎯 Integration test passed successfully!");
}

/// 複数クエリのテスト
#[tokio::test]
async fn test_multiple_queries() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let mut client = JsonRpcTestClient::new().expect("Failed to start client");
    client.start_message_reader().expect("Failed to start message reader");
    
    thread::sleep(Duration::from_millis(1000));
    
    let queries = vec!["fn", "struct", "impl"];
    
    for query in queries {
        println!("🔍 Testing query: '{}'", query);
        
        client.send_notification("updateQuery", Some(json!({"query": query}))).expect("Failed to send query");
        thread::sleep(Duration::from_millis(1500));
        
        let result_count = client.get_result_count();
        println!("📊 Query '{}' found {} results", query, result_count);
        
        assert!(result_count > 0, "Result count should be non-negative for query: {}", query);
    }
    
    println!("🎯 Multiple queries test passed!");
}

/// エラー処理のテスト
#[tokio::test]
async fn test_error_handling() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .is_test(true)
        .try_init();

    let mut client = JsonRpcTestClient::new().expect("Failed to start client");
    client.start_message_reader().expect("Failed to start message reader");
    
    thread::sleep(Duration::from_millis(1000));
    
    // 無効なメソッドを送信
    let _invalid_id = client.send_request("invalid.method", None).expect("Failed to send invalid request");
    thread::sleep(Duration::from_millis(500));
    
    let messages = client.get_received_messages();
    
    // エラーレスポンスを確認
    let has_error = messages.iter().any(|msg| {
        msg.get("error").is_some()
    });
    
    assert!(has_error, "Should receive error response for invalid method");
    
    println!("🎯 Error handling test passed!");
}