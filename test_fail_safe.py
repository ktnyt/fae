#!/usr/bin/env python3
import subprocess
import json
import time

def test_stdio_failsafe():
    """stdio終了時の自動シャットダウンをテスト"""
    print("Testing stdio fail-safe shutdown mechanism...")
    
    # プロセスを起動
    process = subprocess.Popen(
        ["cargo", "run", "--example", "jsonrpc_stdio_example"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    try:
        # 少し待ってプロセスが起動することを確認
        time.sleep(0.5)
        
        # プロセスがまだ実行中かを確認
        if process.poll() is not None:
            print(f"Process exited early with code: {process.returncode}")
            return
            
        print("Process started successfully")
        
        # stdinを閉じてEOFを送信（fail-safe trigger）
        print("Closing stdin to trigger EOF...")
        if process.stdin:
            process.stdin.close()
        
        # プロセスが自動的に終了するのを待つ（最大3秒）
        try:
            stdout, stderr = process.communicate(timeout=3)
            print(f"Process exited with code: {process.returncode}")
            
            # ログからfail-safe動作を確認
            if "stdin EOF reached, triggering automatic shutdown" in stderr:
                print("✅ EOF detection successful")
            else:
                print("❌ EOF detection not found in logs")
                
            if "stdio terminated, shutting down engine" in stderr:
                print("✅ Automatic shutdown triggered")
            else:
                print("❌ Automatic shutdown not found in logs")
                
            print("\n--- STDERR ---")
            print(stderr)
            
        except subprocess.TimeoutExpired:
            print("❌ Process did not exit within timeout")
            process.kill()
            
    except Exception as e:
        print(f"Test error: {e}")
        process.kill()

if __name__ == "__main__":
    test_stdio_failsafe()