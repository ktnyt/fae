#!/usr/bin/env python3
"""
fae リテラル検索 JSON-RPC サーバーのテストクライアント

このスクリプトは、faeサーバーとJSON-RPC通信を行い、
updateQuery通知を送信して検索結果を受信します。

使用方法:
    python3 test_client.py "search_query"

例:
    python3 test_client.py "function"
"""

import json
import sys
import subprocess
import threading
import time


class JsonRpcClient:
    def __init__(self, server_command):
        """JSON-RPCクライアントを初期化"""
        self.process = subprocess.Popen(
            server_command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=0
        )
        self.next_id = 1
        self.result_count = 0
        
    def send_notification(self, method, params=None):
        """通知を送信"""
        message = {
            "jsonrpc": "2.0",
            "method": method
        }
        if params:
            message["params"] = params
            
        json_str = json.dumps(message)
        content_length = len(json_str)
        
        full_message = f"Content-Length: {content_length}\r\n\r\n{json_str}"
        print(f"Sending: {full_message}")
        
        self.process.stdin.write(full_message)
        self.process.stdin.flush()
        
    def send_request(self, method, params=None):
        """リクエストを送信"""
        message = {
            "jsonrpc": "2.0",
            "id": self.next_id,
            "method": method
        }
        if params:
            message["params"] = params
            
        self.next_id += 1
        
        json_str = json.dumps(message)
        content_length = len(json_str)
        
        full_message = f"Content-Length: {content_length}\r\n\r\n{json_str}"
        print(f"Sending: {full_message}")
        
        self.process.stdin.write(full_message)
        self.process.stdin.flush()
        
    def read_messages(self):
        """サーバーからのメッセージを読み取り"""
        while True:
            try:
                # Content-Lengthヘッダーを読み取り
                header_line = self.process.stdout.readline()
                if not header_line:
                    break
                    
                header_line = header_line.strip()
                if not header_line.startswith("Content-Length:"):
                    continue
                    
                content_length = int(header_line.split(":")[1].strip())
                
                # 空行をスキップ
                self.process.stdout.readline()
                
                # JSONメッセージを読み取り
                json_str = self.process.stdout.read(content_length)
                
                try:
                    message = json.loads(json_str)
                    self.handle_message(message)
                except json.JSONDecodeError as e:
                    print(f"JSON decode error: {e}")
                    
            except Exception as e:
                print(f"Read error: {e}")
                break
                
    def handle_message(self, message):
        """受信したメッセージを処理"""
        print(f"Received: {json.dumps(message, indent=2)}")
        
        if "method" in message:
            # 通知またはリクエスト
            method = message["method"]
            if method == "clearSearchResults":
                print("🧹 Search results cleared")
                self.result_count = 0
            elif method == "pushSearchResult":
                params = message.get("params", {})
                self.result_count += 1
                filename = params.get("filename", "?")
                line = params.get("line", "?")
                content = params.get("content", "").strip()
                print(f"📄 Result #{self.result_count}: {filename}:{line} - {content}")
        else:
            # レスポンス
            if "result" in message:
                print(f"✅ Response: {message['result']}")
            elif "error" in message:
                print(f"❌ Error: {message['error']}")
                
    def close(self):
        """クライアントを終了"""
        try:
            self.process.stdin.close()
            self.process.terminate()
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()


def main():
    if len(sys.argv) != 2:
        print("Usage: python3 test_client.py <search_query>")
        sys.exit(1)
        
    query = sys.argv[1]
    print(f"🔍 Searching for: '{query}'")
    
    # faeサーバーを起動
    client = JsonRpcClient(["cargo", "run", "--bin", "fae-service", "--", "start", "search:literal", "--log-level", "info"])
    
    # メッセージ読み取りスレッドを開始
    reader_thread = threading.Thread(target=client.read_messages, daemon=True)
    reader_thread.start()
    
    # 少し待ってからテストを開始
    time.sleep(1)
    
    try:
        # ステータスを確認
        print("\n📋 Checking server status...")
        client.send_request("search.status")
        time.sleep(0.5)
        
        # 検索を実行
        print(f"\n🚀 Starting search for '{query}'...")
        client.send_notification("updateQuery", {"query": query})
        
        # 結果を待機
        print("\n⏳ Waiting for search results...")
        time.sleep(3)  # 検索完了を待機
        
        print(f"\n✨ Search completed! Found {client.result_count} results.")
        
    except KeyboardInterrupt:
        print("\n🛑 Interrupted by user")
    finally:
        client.close()


if __name__ == "__main__":
    main()