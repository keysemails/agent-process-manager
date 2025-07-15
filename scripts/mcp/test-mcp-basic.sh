#!/bin/bash
# Basic MCP testing script that sends JSON-RPC messages to APM

set -e

echo "=== APM MCP Basic Test ==="
echo

# Create a temporary file for MCP communication
TEMP_FILE=$(mktemp)

# Function to send JSON-RPC request and get response
send_request() {
    local request=$1
    echo "Request: $request"
    echo "$request" | APM_MCP_ENABLED=1 timeout 5s cargo run --quiet -- start --mcp 2>/dev/null || true
    echo
}

# Test 1: Initialize
echo "1. Testing initialization..."
send_request '{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "0.1.0",
    "capabilities": {}
  },
  "id": 1
}'

# Test 2: List tools
echo "2. Testing list tools..."
cat << 'EOF' | APM_MCP_ENABLED=1 timeout 5s cargo run --quiet -- start --mcp 2>/dev/null || true
{"jsonrpc": "2.0", "method": "initialize", "params": {"protocolVersion": "0.1.0", "capabilities": {}}, "id": 1}
{"jsonrpc": "2.0", "method": "tools/list", "params": {}, "id": 2}
EOF
echo

# Test 3: Call spawn tool
echo "3. Testing spawn tool..."
cat << 'EOF' | APM_MCP_ENABLED=1 timeout 5s cargo run --quiet -- start --mcp 2>/dev/null || true
{"jsonrpc": "2.0", "method": "initialize", "params": {"protocolVersion": "0.1.0", "capabilities": {}}, "id": 1}
{"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "spawn", "arguments": {"name": "test-echo", "command": "echo", "args": ["Hello MCP"]}}, "id": 2}
EOF
echo

# Test 4: List resources
echo "4. Testing list resources..."
cat << 'EOF' | APM_MCP_ENABLED=1 timeout 5s cargo run --quiet -- start --mcp 2>/dev/null || true
{"jsonrpc": "2.0", "method": "initialize", "params": {"protocolVersion": "0.1.0", "capabilities": {}}, "id": 1}
{"jsonrpc": "2.0", "method": "resources/list", "params": {}, "id": 2}
EOF
echo

# Test 5: System query
echo "5. Testing system overview query..."
cat << 'EOF' | APM_MCP_ENABLED=1 timeout 5s cargo run --quiet -- start --mcp 2>/dev/null || true
{"jsonrpc": "2.0", "method": "initialize", "params": {"protocolVersion": "0.1.0", "capabilities": {}}, "id": 1}
{"jsonrpc": "2.0", "method": "tools/call", "params": {"name": "query", "arguments": {"type": "system_overview"}}, "id": 2}
EOF
echo

# Clean up
rm -f "$TEMP_FILE"

echo "=== Basic MCP tests completed ==="
echo
echo "To run more comprehensive tests:"
echo "1. Install MCP Inspector: npm install -g @modelcontextprotocol/inspector"
echo "2. Run: mcp-inspector stdio -- cargo run -- start --mcp"
echo
echo "Or run the Rust integration tests:"
echo "cargo test mcp_test"