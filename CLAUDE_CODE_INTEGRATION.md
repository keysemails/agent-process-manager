# Claude Code Integration Guide

This guide shows how to integrate the Agent Process Manager (APM) with Claude Code, enabling AI assistants to manage background processes efficiently.

## Overview

APM provides an MCP (Model Context Protocol) server that Claude Code agents can connect to for:
- **Process Management**: Spawn, monitor, and control background processes
- **Intelligent Log Access**: Get AI-optimized process logs and summaries
- **Working Directory Isolation**: Automatic process filtering based on current directory
- **Resource Monitoring**: Real-time CPU and memory usage tracking

## Quick Setup

### 1. Install and Start APM

```bash
# Clone and build APM
git clone https://github.com/sunnya97/agent-process-manager.git
cd agent-process-manager
cargo build --release

# Install globally (optional)
sudo cp target/release/apm /usr/local/bin/

# Start the daemon with MCP enabled
apm start
```

APM will automatically start the MCP server on port 7338 (configurable in `apm.yaml`).

### 2. Configure Claude Code

**One command setup:**
```bash
claude mcp add agent-process-manager apm mcp-bridge -e RUST_LOG=warn
```

That's it! APM is now available in all Claude Code sessions.

<details>
<summary>Alternative: Manual configuration</summary>

Create or edit your Claude Code configuration file at:
```
~/.config/claude/claude_code_config.json
```

Add the APM MCP server:

```json
{
  "mcpServers": {
    "agent-process-manager": {
      "command": "apm",
      "args": ["mcp-bridge"],
      "env": {
        "RUST_LOG": "warn"
      }
    }
  }
}
```
</details>

### 3. Test the Integration

Start a new Claude Code session and verify APM tools are available:

```bash
# In Claude Code, you should now have access to these tools:
# - spawn: Start new processes
# - list: List running processes 
# - logs: View process logs
# - stop: Stop processes
# - query: Get AI-optimized system data
```

## Configuration Options

### APM Configuration (apm.yaml)

```yaml
# MCP Server Configuration
mcp:
  enabled: true
  transport: "tcp"           # TCP transport for MCP
  tcp_host: "127.0.0.1"      # Bind to localhost only
  tcp_port: 7338             # MCP server port

# Access Control
access_control:
  mode: "open"               # Recommended for development
  # "open": Read access is system-wide, write requires directory access
  # "strict": Both read and write require directory access
  # "unrestricted": Full access regardless of directory
```

### Alternative Claude Code Configurations

#### Direct TCP Connection (via netcat)
```json
{
  "mcpServers": {
    "apm": {
      "command": "nc",
      "args": ["localhost", "7338"]
    }
  }
}
```

#### Custom APM Path
```json
{
  "mcpServers": {
    "agent-process-manager": {
      "command": "/path/to/apm",
      "args": ["mcp-bridge", "--host", "127.0.0.1", "--port", "7338"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
```

## Available MCP Tools

### Process Management

#### `spawn` - Start New Process
```json
{
  "name": "spawn",
  "arguments": {
    "name": "web-server",
    "command": "python3",
    "args": ["-m", "http.server", "8080"]
  }
}
```

#### `list` - List Processes
```json
{
  "name": "list",
  "arguments": {
    "current_dir": true  // Filter to current directory only
  }
}
```

#### `stop` - Stop Process
```json
{
  "name": "stop", 
  "arguments": {
    "process_id": "12345"
  }
}
```

#### `logs` - Get Process Logs
```json
{
  "name": "logs",
  "arguments": {
    "process_id": "12345",
    "limit": 100
  }
}
```

### AI-Optimized Queries

#### `query` - Structured Data Queries
```json
{
  "name": "query",
  "arguments": {
    "type": "system_overview",
    "current_dir": true
  }
}
```

Available query types:
- `system_overview`: Process counts, status summary
- `process_errors`: Recent errors across processes

## Working Directory Isolation

APM automatically provides directory-based access control:

- **Hierarchical Access**: Parent directories can manage subdirectory processes
- **Current Directory Filtering**: Use `current_dir: true` to scope operations
- **Automatic Tagging**: Processes tagged with spawn directory

### Example Workflow

```bash
# In /project/backend
claude: spawn api python app.py

# In /project/frontend  
claude: spawn web npm start

# In /project (parent directory)
claude: list all processes        # Shows both api and web
claude: list current_dir=true     # Shows both (hierarchical access)

# In /project/backend
claude: list current_dir=true     # Shows only api process
```

## Troubleshooting

### Common Issues

#### APM daemon not running
```bash
# Check status
apm status

# Start if needed
apm start
```

#### MCP connection failed
```bash
# Test the bridge manually
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | apm mcp-bridge

# Check MCP server logs
tail -f apm.log
```

#### Tools not appearing in Claude Code
1. Verify configuration file location: `~/.config/claude/claude_code_config.json`
2. Check JSON syntax is valid
3. Restart Claude Code after configuration changes
4. Ensure `apm` command is in PATH

### Debug Mode

Enable detailed logging:

```json
{
  "mcpServers": {
    "agent-process-manager": {
      "command": "apm",
      "args": ["mcp-bridge"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

## Benefits for AI Assistants

### Context Efficiency
- **90% reduction** in context usage vs raw logs
- Pattern detection for ports, URLs, errors
- Structured summaries instead of raw text

### Intelligent Process Management
- Automatic resource monitoring
- Directory-based isolation
- Real-time log analysis

### Enhanced Development Workflow
- Start development servers with one command
- Monitor multiple services simultaneously  
- Get AI-friendly error summaries
- Manage processes across project directories

## Security Considerations

- APM binds MCP server to localhost only by default
- Directory-based access control prevents cross-project interference
- Process isolation ensures clean separation
- No network exposure unless explicitly configured

## Next Steps

1. **Try the Examples**: Use the configuration above to connect Claude Code to APM
2. **Explore Query Types**: Experiment with different query types for system insights
3. **Customize Access Control**: Adjust `access_control.mode` based on your security needs
4. **Monitor Performance**: Use APM's resource monitoring to track process health

For more details, see the [main README](README.md) and [CLAUDE.md](CLAUDE.md) documentation.