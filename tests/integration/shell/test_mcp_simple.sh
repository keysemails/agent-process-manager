#!/bin/bash

# Create a test input file with JSON-RPC messages
cat > mcp_test_input.json << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list"}}
EOF

echo "Testing MCP server with test input..."
echo "========================================"

# Set MCP enabled and run the server with test input
APM_MCP_ENABLED=1 timeout 5 cargo run --bin apm -- start --mcp < mcp_test_input.json 2>&1 || true

# Clean up
rm -f mcp_test_input.json