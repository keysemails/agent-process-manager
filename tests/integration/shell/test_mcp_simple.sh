#!/bin/bash

echo "Testing MCP server functionality..."
echo "===================================="

# Build APM first
cd ../../../ && cargo build --bin apm && cd tests/integration/shell

# Create a test config with MCP enabled
cat > test_mcp_config.yaml << 'EOF'
api:
  host: "127.0.0.1"
  port: 7337

mcp:
  enabled: true
  transport: "tcp"
  tcp_host: "127.0.0.1"
  tcp_port: 7338

cleanup:
  auto_clean_on_startup: false
  retention_hours: 24
  keep_logs: true

access_control:
  mode: "open"
EOF

# Stop any existing daemon
../../../target/debug/apm stop-all --force 2>/dev/null || true
sleep 1

# Start daemon with MCP enabled
echo "Starting APM daemon with MCP enabled..."
../../../target/debug/apm start --config test_mcp_config.yaml &
DAEMON_PID=$!
sleep 3

# Test if MCP port is listening
if nc -z 127.0.0.1 7338 2>/dev/null; then
    echo "✅ MCP server is listening on port 7338"
    
    # Test MCP bridge command
    echo "Testing MCP bridge..."
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}' | timeout 5 ../../../target/debug/apm mcp-bridge 2>&1 | head -3
    
    echo "✅ MCP bridge test completed"
else
    echo "❌ MCP server is not listening on port 7338"
fi

# Clean up
../../../target/debug/apm stop-all --force 2>/dev/null || true
kill $DAEMON_PID 2>/dev/null || true
rm -f test_mcp_config.yaml

echo "MCP test completed"