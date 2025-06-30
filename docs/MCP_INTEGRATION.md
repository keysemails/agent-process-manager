# MCP Integration Guide for Agent Process Manager

The Agent Process Manager (APM) now supports the Model Context Protocol (MCP), allowing AI agents like Claude Code to seamlessly interact with APM for process management tasks.

## Overview

MCP (Model Context Protocol) is a standardized protocol that enables AI assistants to interact with external tools and services. APM's MCP integration exposes all process management capabilities through MCP tools, resources, and prompts.

## Starting APM as an MCP Server

### Command Line

```bash
# Start APM in MCP server mode
apm start --mcp

# With custom configuration
apm start --mcp --config /path/to/config.yaml

# Enable MCP via environment variable
APM_MCP_ENABLED=1 apm start --mcp
```

### Configuration

Add MCP settings to your `apm.yaml` configuration file:

```yaml
mcp:
  enabled: true
  stdio: true  # Use stdio transport (default)
  # tcp_host: "127.0.0.1"  # Optional: TCP transport
  # tcp_port: 9337         # Optional: TCP port
```

## Available MCP Tools

### 1. `spawn` - Start a new process
```json
{
  "name": "my-server",
  "command": "python",
  "args": ["-m", "http.server", "8080"]
}
```

### 2. `list` - List all processes
```json
{}
```

### 3. `logs` - Get process logs
```json
{
  "process_id": "uuid-here",
  "limit": 100
}
```

### 4. `stop` - Stop a process
```json
{
  "process_id": "uuid-here"
}
```

### 5. `query` - Execute structured queries
```json
{
  "type": "system_overview"
}
```

Query types:
- `system_overview` - Get overview of all processes
- `process_errors` - Find errors across processes
- `port_mapping` - Discover ports in use
- `performance_metrics` - Get CPU/memory usage
- `log_search` - Search logs by pattern
- `event_correlation` - Correlate events timeline

## MCP Resources

APM exposes the following resources:

- `apm://processes` - List of all processes
- `apm://processes/{id}` - Details for a specific process
- `apm://logs/{id}` - Logs for a specific process

## MCP Prompts

Pre-configured prompts for common tasks:

- `analyze-errors` - Analyze error patterns across all processes
- `system-health` - Generate a system health report

## Integration with Claude Code

### 1. Add APM to Claude Code Configuration

Create or edit `~/.config/claude/claude_code_config.json`:

```json
{
  "mcpServers": {
    "apm": {
      "command": "/usr/local/bin/apm",
      "args": ["start", "--mcp"],
      "env": {
        "APM_MCP_ENABLED": "1"
      }
    }
  }
}
```

### 2. Using APM in Claude Code

Once configured, you can use APM tools directly:

```
"Use the apm spawn tool to start a web server on port 8080"
"List all running processes using apm"
"Show me the logs for the web-server process"
"Stop all processes that are using too much CPU"
```

## Example Workflows

### Starting a Development Environment

```
1. "Use apm to spawn a backend server: python app.py"
2. "Spawn a frontend dev server: npm run dev"
3. "List all processes to confirm they're running"
4. "Monitor the logs for any errors"
```

### Debugging Issues

```
1. "Query apm for any process errors in the last 5 minutes"
2. "Show me which ports are in use"
3. "Get the performance metrics for all processes"
4. "Analyze error patterns and suggest fixes"
```

### Process Management

```
1. "List all apm processes"
2. "Stop the process named 'test-server'"
3. "Restart the backend process"
4. "Get system health report from apm"
```

## Advanced Features

### Custom Environment Variables

```json
{
  "name": "my-app",
  "command": "node",
  "args": ["server.js"],
  "env": {
    "NODE_ENV": "development",
    "PORT": "3000"
  }
}
```

### Working Directory

```json
{
  "name": "my-app",
  "command": "python",
  "args": ["app.py"],
  "cwd": "/path/to/project"
}
```

### Process Tags

```json
{
  "name": "api-server",
  "command": "python",
  "args": ["api.py"],
  "tags": ["backend", "api", "production"]
}
```

## Troubleshooting

### MCP Server Not Starting

1. Check if MCP is enabled in configuration
2. Verify APM is installed correctly: `which apm`
3. Check logs: `RUST_LOG=debug apm start --mcp`

### Connection Issues

1. Ensure APM daemon is not already running
2. Check if port 7337 is available (for HTTP mode)
3. Verify stdio communication is working

### Process Management Issues

1. Use `apm list` to verify process status
2. Check tmux sessions: `tmux list-sessions`
3. Review logs for error messages

## Best Practices

1. **Use tmux mode** (default) for better terminal interaction
2. **Set appropriate resource limits** to prevent runaway processes
3. **Tag processes** for better organization
4. **Monitor logs regularly** for early error detection
5. **Use structured queries** for efficient AI agent interaction

## Security Considerations

1. APM MCP server runs with user permissions
2. Process isolation depends on OS-level security
3. Consider using read-only mode for sensitive environments
4. Audit MCP tool usage through APM logs

## Future Enhancements

- Process groups and dependencies
- Advanced restart policies
- Custom pattern detection
- Resource usage alerts
- Integration with more AI platforms