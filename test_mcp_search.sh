#!/bin/bash
# Test script for MCP search functionality

set -e

echo "Testing APM MCP search functionality..."

# Build the project
echo "Building APM..."
cargo build --release

# Kill any existing APM daemon
./target/release/apm shutdown --force 2>/dev/null || true
sleep 2

# Start APM daemon with search enabled
echo "Starting APM daemon with search enabled..."
APM_SEARCH_ENABLED=true ./target/release/apm start &
sleep 3

# Check status
echo "Checking daemon status..."
./target/release/apm status

# Spawn some test processes that generate logs
echo "Spawning test processes..."
./target/release/apm spawn search-test-1 bash -c "for i in {1..10}; do echo 'INFO: Processing item '$i; echo 'DEBUG: Item details for '$i; sleep 1; done; echo 'ERROR: Test error occurred'; echo 'WARN: This is a warning'"
./target/release/apm spawn search-test-2 bash -c "echo 'Starting database connection...'; sleep 2; echo 'ERROR: Database connection timeout'; echo 'INFO: Retrying connection...'; sleep 2; echo 'SUCCESS: Connected to database'"

# Wait for logs to be generated and indexed
echo "Waiting for logs to be generated..."
sleep 5

# Test search via HTTP API first
echo -e "\n=== Testing HTTP API search ==="
echo "Searching for 'error'..."
curl -s -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{"query": "error"}' | jq '.data.total_hits'

# Now test via MCP using a simple Node.js script
echo -e "\n=== Testing MCP search ==="
cat > test_mcp_search.js << 'EOF'
const net = require('net');

// Connect to MCP server
const client = net.createConnection({ port: 7338, host: '127.0.0.1' }, () => {
  console.log('Connected to MCP server');
  
  // Send JSON-RPC request for search
  const searchRequest = {
    jsonrpc: "2.0",
    method: "tools/call",
    params: {
      name: "search",
      arguments: {
        query: "error",
        limit: 10
      }
    },
    id: 1
  };
  
  client.write(JSON.stringify(searchRequest) + '\n');
});

client.on('data', (data) => {
  console.log('Received:', data.toString());
  
  // Send another search with filters
  const advancedSearch = {
    jsonrpc: "2.0",
    method: "tools/call",
    params: {
      name: "search",
      arguments: {
        query: "database AND connection",
        level: "error"
      }
    },
    id: 2
  };
  
  client.write(JSON.stringify(advancedSearch) + '\n');
  
  setTimeout(() => {
    client.end();
  }, 1000);
});

client.on('end', () => {
  console.log('Disconnected from server');
  process.exit(0);
});
EOF

# Run the MCP test
node test_mcp_search.js

# Clean up
echo -e "\n=== Cleaning up ==="
./target/release/apm kill-all --force
./target/release/apm clean --force
rm -f test_mcp_search.js

echo -e "\nMCP search test completed!"