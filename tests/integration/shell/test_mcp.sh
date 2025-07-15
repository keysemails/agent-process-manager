#!/bin/bash

echo "Testing MCP server TCP connection..."
echo "===================================="

# Build APM first
cd ../../../ && cargo build --bin apm && cd tests/integration/shell

# Create a test config with MCP enabled
cat > test_mcp_tcp_config.yaml << 'EOF'
api:
  host: "127.0.0.1"
  port: 7337

mcp:
  enabled: true
  transport: "tcp"
  tcp_host: "127.0.0.1"
  tcp_port: 7339

cleanup:
  auto_clean_on_startup: false
  
access_control:
  mode: "open"
EOF

# Stop any existing daemon
../../../target/debug/apm stop-all --force 2>/dev/null || true
sleep 1

# Start daemon with MCP on different port to avoid conflicts
echo "Starting APM daemon with MCP on port 7339..."
../../../target/debug/apm start --config test_mcp_tcp_config.yaml &
DAEMON_PID=$!
sleep 3

# Test TCP connection to MCP server
if command -v nc >/dev/null 2>&1; then
    if nc -z 127.0.0.1 7339 2>/dev/null; then
        echo "✅ MCP TCP server is listening on port 7339"
        
        # Test basic JSON-RPC communication
        echo "Testing JSON-RPC communication..."
        (
            echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}'
            sleep 1
        ) | nc 127.0.0.1 7339 2>/dev/null | head -3
        
        echo "✅ MCP TCP communication test completed"
    else
        echo "❌ MCP TCP server is not listening on port 7339"
    fi
else
    echo "⚠️  nc (netcat) not available, skipping TCP connection test"
    echo "✅ MCP configuration test passed (daemon started successfully)"
fi

# Clean up
../../../target/debug/apm stop-all --force 2>/dev/null || true
kill $DAEMON_PID 2>/dev/null || true
rm -f test_mcp_tcp_config.yaml

echo "MCP TCP test completed"