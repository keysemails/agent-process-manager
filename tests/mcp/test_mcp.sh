#!/bin/bash

# Test MCP server with initialize request
echo "Testing MCP server initialization..."

# Send initialize request
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol_version":"2025-03-26","capabilities":{},"client_info":{"name":"test-client","version":"1.0.0"}}}' | cargo run --bin apm -- mcp 2>&1 | head -50