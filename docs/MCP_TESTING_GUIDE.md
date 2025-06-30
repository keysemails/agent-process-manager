# MCP Testing Guide

This guide covers various methods to test the MCP integration in APM.

## 1. Unit Tests

Run the MCP integration tests:

```bash
# Run all MCP tests
cargo test mcp_test

# Run with output
cargo test mcp_test -- --nocapture

# Run specific test
cargo test test_spawn_process -- --nocapture
```

## 2. Manual Testing with MCP Inspector

The MCP Inspector is a tool for testing MCP servers interactively.

### Install MCP Inspector

```bash
npm install -g @modelcontextprotocol/inspector
```

### Test APM MCP Server

```bash
# Start APM in MCP mode
APM_MCP_ENABLED=1 cargo run -- start --mcp

# In another terminal, use the inspector
mcp-inspector stdio -- cargo run -- start --mcp
```

## 3. Testing with Node.js MCP Client

Create a test script to interact with APM:

```javascript
// test-apm-mcp.js
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

async function testAPM() {
  const transport = new StdioClientTransport({
    command: 'cargo',
    args: ['run', '--', 'start', '--mcp'],
    env: { ...process.env, APM_MCP_ENABLED: '1' }
  });

  const client = new Client({
    name: 'test-client',
    version: '1.0.0'
  }, {
    capabilities: {}
  });

  await client.connect(transport);

  // List available tools
  const tools = await client.listTools();
  console.log('Available tools:', tools);

  // Spawn a process
  const spawnResult = await client.callTool('spawn', {
    name: 'test-server',
    command: 'python',
    args: ['-m', 'http.server', '8080']
  });
  console.log('Spawn result:', spawnResult);

  // List processes
  const listResult = await client.callTool('list', {});
  console.log('Processes:', listResult);

  // Get logs
  const processes = JSON.parse(listResult.content[0].text);
  if (processes.length > 0) {
    const logsResult = await client.callTool('logs', {
      process_id: processes[0].id,
      limit: 10
    });
    console.log('Logs:', logsResult);
  }

  await client.close();
}

testAPM().catch(console.error);
```

Run the test:

```bash
node test-apm-mcp.js
```

## 4. Testing with Python MCP Client

```python
# test_apm_mcp.py
import asyncio
import json
from mcp import Client, StdioTransport

async def test_apm():
    async with StdioTransport(
        command=['cargo', 'run', '--', 'start', '--mcp'],
        env={'APM_MCP_ENABLED': '1'}
    ) as transport:
        async with Client('test-client', '1.0.0') as client:
            await client.connect(transport)
            
            # Initialize
            await client.initialize()
            
            # List tools
            tools = await client.list_tools()
            print(f"Available tools: {[t.name for t in tools.tools]}")
            
            # Spawn a process
            result = await client.call_tool('spawn', {
                'name': 'test-echo',
                'command': 'echo',
                'args': ['Hello from Python MCP client']
            })
            print(f"Spawn result: {result}")
            
            # Query system overview
            overview = await client.call_tool('query', {
                'type': 'system_overview'
            })
            print(f"System overview: {json.loads(overview.content[0].text)}")

if __name__ == '__main__':
    asyncio.run(test_apm())
```

## 5. Testing with Claude Code

### Setup

1. Build and install APM:
```bash
cargo build --release
sudo cp target/release/apm /usr/local/bin/
```

2. Configure Claude Code:
```bash
mkdir -p ~/.config/claude
cat > ~/.config/claude/claude_code_config.json << EOF
{
  "mcpServers": {
    "apm": {
      "command": "/usr/local/bin/apm",
      "args": ["start", "--mcp"],
      "env": {
        "APM_MCP_ENABLED": "1",
        "RUST_LOG": "agent_process_manager=debug"
      }
    }
  }
}
EOF
```

3. Test in Claude Code:
```
"Use the apm tool to spawn a test web server on port 8080"
"List all processes managed by apm"
"Show me the logs from the web server"
```

## 6. Direct CLI Testing

Test basic MCP functionality:

```bash
# Test that MCP mode starts correctly
APM_MCP_ENABLED=1 cargo run -- start --mcp

# In debug mode
RUST_LOG=debug APM_MCP_ENABLED=1 cargo run -- start --mcp

# Test with configuration file
cargo run -- start --mcp --config examples/mcp-config.yaml
```

## 7. Integration Test Script

Create a comprehensive test script:

```bash
#!/bin/bash
# test-mcp-integration.sh

echo "Building APM..."
cargo build --release

echo "Starting APM daemon (HTTP mode)..."
./target/release/apm start &
DAEMON_PID=$!
sleep 2

echo "Testing daemon health..."
curl -s http://localhost:7337/health | jq .

echo "Spawning test process via HTTP API..."
curl -X POST http://localhost:7337/api/processes \
  -H "Content-Type: application/json" \
  -d '{
    "name": "test-http-server",
    "command": "python",
    "args": ["-m", "http.server", "9090"]
  }' | jq .

echo "Listing processes..."
curl -s http://localhost:7337/api/processes | jq .

echo "Stopping daemon..."
kill $DAEMON_PID

echo "Testing MCP mode..."
timeout 10s ./target/release/apm start --mcp <<EOF
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "capabilities": {}
  },
  "id": 1
}
{
  "jsonrpc": "2.0",
  "method": "tools/list",
  "params": {},
  "id": 2
}
EOF

echo "Test complete!"
```

## 8. Debugging MCP Issues

### Enable Debug Logging

```bash
RUST_LOG=agent_process_manager=debug,rust_mcp_sdk=debug cargo run -- start --mcp
```

### Test Individual Components

```rust
// Add this test to verify MCP server creation
#[tokio::test]
async fn test_mcp_server_creation() {
    let config = Config::default();
    let log_storage = Arc::new(LogStorage::new(":memory:").await.unwrap());
    let (log_tx, _) = tokio::sync::mpsc::channel(100);
    let process_manager = Arc::new(ProcessManager::new(log_tx));
    
    let server = McpServer::new(process_manager, log_storage).await;
    assert!(server.is_ok());
}
```

### Common Issues and Solutions

1. **MCP not enabled error**
   - Set `APM_MCP_ENABLED=1` environment variable
   - Or add `mcp.enabled: true` to config file

2. **Connection timeout**
   - Check if APM binary is in PATH
   - Verify no other process is using stdio
   - Check logs for startup errors

3. **Tool not found**
   - Verify tool name spelling
   - Check `list_tools()` output
   - Ensure MCP server initialized properly

4. **Process spawn failures**
   - Check command exists in PATH
   - Verify working directory permissions
   - Look for tmux session conflicts

## 9. Performance Testing

```javascript
// perf-test-mcp.js
async function perfTest() {
  const client = await connectToAPM();
  
  console.time('spawn-100-processes');
  for (let i = 0; i < 100; i++) {
    await client.callTool('spawn', {
      name: `test-${i}`,
      command: 'sleep',
      args: ['10']
    });
  }
  console.timeEnd('spawn-100-processes');
  
  console.time('list-processes');
  await client.callTool('list', {});
  console.timeEnd('list-processes');
  
  console.time('query-errors');
  await client.callTool('query', {
    type: 'process_errors',
    time_window: '5m'
  });
  console.timeEnd('query-errors');
}
```

## 10. Continuous Testing

Add to CI/CD pipeline:

```yaml
# .github/workflows/test.yml
- name: Test MCP Integration
  run: |
    cargo test mcp_test
    cargo build --release
    ./scripts/test-mcp-integration.sh
```