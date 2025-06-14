#!/usr/bin/env python3
import subprocess
import json
import re

def test_jsonrpc_stdio():
    test_cases = [
        # ping test
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "ping"
        },
        # echo test
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "echo",
            "params": "Hello World"
        },
        # reverse test
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "reverse",
            "params": "Hello"
        },
        # shutdown test
        {
            "jsonrpc": "2.0",
            "id": 99,
            "method": "shutdown"
        }
    ]
    
    for i, request in enumerate(test_cases, 1):
        print(f"\n=== Test {i}: {request['method']} ===")
        
        request_json = json.dumps(request)
        content_length = len(request_json)
        message = f"Content-Length: {content_length}\r\n\r\n{request_json}"
        
        print(f"Sending: {request_json}")
        
        # プロセスを起動
        process = subprocess.Popen(
            ["cargo", "run", "--example", "jsonrpc_stdio_example"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        try:
            # shutdownの場合はより長いタイムアウトは不要
            timeout = 1 if request['method'] == 'shutdown' else 3
            # メッセージを送信
            stdout, stderr = process.communicate(input=message, timeout=timeout)
            
            # レスポンスを解析
            if stdout:
                # Content-Lengthヘッダーを探す
                lines = stdout.strip().split('\n')
                for line in lines:
                    if line.startswith('{') and line.endswith('}'):
                        try:
                            response = json.loads(line)
                            print(f"Response: {response}")
                            if 'result' in response:
                                print(f"Result: {response['result']}")
                            if 'error' in response:
                                print(f"Error: {response['error']}")
                        except json.JSONDecodeError:
                            print(f"Failed to parse JSON: {line}")
            else:
                print("No stdout received")
            
        except subprocess.TimeoutExpired:
            process.kill()
            print("Process timed out")

if __name__ == "__main__":
    test_jsonrpc_stdio()