#!/bin/bash

echo "Building JSON-RPC stdio example..."
cargo build --example jsonrpc_stdio_example

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

echo "Starting JSON-RPC stdio server in background..."
cargo run --example jsonrpc_stdio_example &
SERVER_PID=$!

# サーバーが起動するまで少し待つ
sleep 1

echo "Testing JSON-RPC requests..."

# ping リクエスト
echo "Sending ping request..."
echo -e 'Content-Length: 53\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"ping"}' | nc localhost 0 2>/dev/null || {
    # nc が使えない場合は直接stdio経由でテスト
    echo 'Content-Length: 53'
    echo ''
    echo '{"jsonrpc":"2.0","id":1,"method":"ping"}'
} | timeout 2s cargo run --example jsonrpc_stdio_example

# echo リクエスト
echo "Sending echo request..."
echo -e 'Content-Length: 88\r\n\r\n{"jsonrpc":"2.0","id":2,"method":"echo","params":"Hello World"}' | timeout 2s cargo run --example jsonrpc_stdio_example

# reverse リクエスト
echo "Sending reverse request..."
echo -e 'Content-Length: 90\r\n\r\n{"jsonrpc":"2.0","id":3,"method":"reverse","params":"Hello"}' | timeout 2s cargo run --example jsonrpc_stdio_example

# log 通知
echo "Sending log notification..."
echo -e 'Content-Length: 77\r\n\r\n{"jsonrpc":"2.0","method":"log","params":"Test notification"}' | timeout 2s cargo run --example jsonrpc_stdio_example

echo "Test completed!"

# サーバープロセスを終了
if kill -0 $SERVER_PID 2>/dev/null; then
    kill $SERVER_PID 2>/dev/null
fi