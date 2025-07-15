# CLAUDE.md - Agent Process Manager (APM)

This file provides guidance to Claude Code when working with the Agent Process Manager codebase.

## Project Overview

Agent Process Manager (APM) is a standalone Rust service designed for AI-native process management. It dramatically reduces context usage for AI agents (by ~90%) while maintaining full human access to logs. APM runs background processes with intelligent log analysis and pattern detection.

## Key Features

- **Process Management**: Spawn, monitor, and control long-running processes with PTY support
- **Dual Log Storage**: Raw logs for humans, structured summaries for AI agents
- **Pattern Detection**: Automatically detects ports, URLs, errors, file paths, and key events with deduplication
- **Resource Monitoring**: Real-time CPU and memory usage tracking via sysinfo
- **Enhanced AI Agent API**: Structured query system for efficient AI interaction
- **Intelligent Summarization**: Error pattern analysis, time-based metrics, and recommendations
- **REST API**: Full control via HTTP endpoints (Axum framework)
- **WebSocket Streaming**: Real-time log streaming for live monitoring
- **CLI Interface**: Human-friendly command-line tool

## Architecture

### Core Components

1. **Process Supervisor** (`src/process/supervisor.rs`)
   - Manages process lifecycle with tmux integration (default) or PTY support
   - Handles process spawning, monitoring, and termination
   - Integrates with HealthMonitor for resource tracking
   - Uses tmux sessions for robust terminal management

2. **Log Storage** (`src/logs/storage.rs`)
   - SQLite-based persistence with SQLx
   - Dual storage: raw lines + structured data
   - Efficient querying and summarization

3. **Pattern Detection** (`src/logs/patterns.rs`)
   - Regex-based detection of important patterns
   - Extracts ports, URLs, errors, file paths
   - Reduces noise for AI consumption

4. **Health Monitoring** (`src/process/health.rs`)
   - Uses sysinfo crate for system metrics
   - Tracks CPU percentage and memory usage
   - Updates every 2 seconds per process

5. **API Server** (`src/api/`)
   - Axum-based REST API
   - WebSocket support for real-time streaming
   - JSON responses with consistent error handling

6. **tmux Integration** (`src/tmux.rs`)
   - Default backend for process management
   - Eliminates terminal emulation complexity
   - Provides native terminal attachment
   - Enables persistent sessions

7. **Log Summarization** (`src/logs/summary.rs`)
   - Real-time data integration with ProcessInfo
   - Error pattern normalization and grouping
   - Time-based metrics (error rates, log velocity)
   - Intelligent recommendation generation
   - Resource usage analysis

## API Endpoints

### HTTP API (Port 7337)
```bash
# Process Management
POST   /api/processes              # Spawn new process
GET    /api/processes              # List all processes
GET    /api/processes/:id          # Get process details
DELETE /api/processes/:id          # Stop process
POST   /api/processes/:id/restart  # Restart process
GET    /api/processes/:id/health   # Get health metrics

# Log Access
GET    /api/logs/:id               # Get logs (with filters)
GET    /api/logs/:id/stream        # WebSocket log streaming
GET    /api/logs/:id/raw           # Raw log output

# AI Agent Endpoints
POST   /api/agent/query            # Structured query system for AI agents
GET    /api/agent/summary          # Get system summary
GET    /api/agent/query-schema     # Get available query types and parameters
GET    /api/agent/capabilities     # Get API capabilities and features

# System
GET    /health                     # Health check
```

### MCP Server (Port 7338 or Unix Socket)
When `mcp.enabled=true`, the daemon also runs an MCP server that AI assistants can connect to:
- **TCP Mode**: Listens on `tcp://127.0.0.1:7338` by default
- **Unix Socket Mode**: Listens on `/tmp/apm.sock` by default
- **Tools Available**: spawn, list, logs, stop, query
- **Transport**: TCP or Unix socket (no longer uses stdio)

## CLI Commands

```bash
# Daemon Management
apm start                          # Start the daemon
apm status                         # Check daemon status

# Process Management
apm spawn <name> <command> [args]  # Start a process (uses tmux by default)
apm list                          # List processes visible from current directory
apm list --all                    # List all processes regardless of directory
apm logs <name>                   # View process logs (from current directory)
apm logs <name> --all             # View logs of any process
apm stop <name>                   # Stop a process (from current directory)
apm stop <name> --all             # Stop any process
apm stop-all                      # Stop all processes (with confirmation)
apm stop-all --force              # Stop all processes without confirmation
apm stop-all --current-dir        # Stop only processes from current directory
apm restart <name>                # Restart a process (from current directory)
apm restart <name> --all          # Restart any process
apm clean                         # Clean all stopped processes (with confirmation)
apm clean --force                 # Clean without confirmation
apm clean --older-than 24         # Clean processes stopped >24 hours ago
apm clean --current-dir           # Clean only from current directory
apm clean --keep-logs             # Clean but preserve log data

# Configuration Management
apm config init                   # Create user config file with defaults
apm config init --force           # Overwrite existing config file
apm config path                   # Show which config file is active
apm config edit                   # Edit config file in $EDITOR

# Interactive
apm attach <name>                 # Attach to process (uses tmux attach)
```

## Working Directory-Based Process Isolation

APM uses a simplified authentication model based on working directories with hierarchical access:
- Processes are automatically tagged with the directory they were spawned from
- Parent directories can see and manage processes from their subdirectories
- Use the `--all` flag to bypass this isolation and access all processes
- MCP clients also respect this isolation based on their working directory

### Hierarchical Access Model
- **Parent directories** can access processes from their subdirectories
- **Sibling directories** cannot access each other's processes
- **Child directories** cannot access parent directory processes

### Access Groups
- Each process has an `access_group` based on the canonical path: `dir:/absolute/path/to/directory`
- Processes without an access_group are globally accessible (legacy/recovered processes)
- The access group is determined at spawn time and cannot be changed

### Examples
```bash
# Project structure:
# /home/user/myproject/
# ├── backend/
# └── frontend/

# In /home/user/myproject/backend
apm spawn api python api.py       # Tagged with dir:/home/user/myproject/backend

# In /home/user/myproject/frontend  
apm spawn web npm start           # Tagged with dir:/home/user/myproject/frontend
apm list                          # Shows only 'web' (frontend process)
apm list --all                    # Shows all processes

# In /home/user/myproject (parent directory)
apm list                          # Shows both 'api' and 'web' (hierarchical access!)
apm stop api                      # Can stop the backend API
apm logs web                      # Can view frontend logs

# In /home/user (grandparent)
apm list                          # Shows all processes under /home/user
```

### Use Cases
- **Monorepo Management**: From the project root, manage all service processes
- **Service Isolation**: Each service directory sees only its own processes
- **Development Workflow**: Work in subdirectories while monitoring from project root

### Configurable Access Control Modes

APM supports three access control modes that can be configured in your config file:

1. **Open Mode** (`mode: "open"`) - Default and recommended for most users
   - **Read operations** (list, logs): Can see all processes regardless of directory
   - **Write operations** (stop, restart, attach): Require hierarchical access
   - Best for collaborative development and debugging
   - Better observability - agents can see what ports are in use system-wide

2. **Strict Mode** (`mode: "strict"`) - For high-security environments
   - Both read and write operations require hierarchical access
   - Provides complete isolation between directory contexts

3. **Unrestricted Mode** (`mode: "unrestricted"`) - For admin environments
   - Full read/write access to all processes regardless of directory structure
   - Bypasses all hierarchical access controls
   - Useful for administrative tools and monitoring systems

#### Configuration Examples

```yaml
# Default open mode (recommended)
access_control:
  mode: "open"

# Strict security mode
access_control:
  mode: "strict"

# Unrestricted admin mode
access_control:
  mode: "unrestricted"
```

The `--all` flag bypasses access control modes for superuser access in CLI commands.

## Development Guidelines

### Building and Testing

```bash
# Build
cargo build

# Run all tests
cargo test

# Run specific test suites
cargo test --test agent_api_test        # AI Agent API tests
cargo test --test log_summarizer_test   # LogSummarizer tests
cargo test --test log_patterns_test     # Pattern detection tests

# Start daemon for development
RUST_LOG=agent_process_manager=debug ./target/debug/apm start

# Start daemon with MCP enabled
APM_MCP_ENABLED=1 ./target/debug/apm start

# Test with example processes
./target/debug/apm spawn test-server python3 -- -m http.server 8080
./target/debug/apm spawn test-app node -- app.js

# Test AI Agent API
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"type": "system_overview"}'

# Test MCP connection (using netcat)
nc localhost 7338
```

## Configuration Management

APM uses a global configuration system following platform-specific conventions for config directories. Configuration files are searched in order of precedence:

1. **User config**: `~/.config/apm/config.yaml` (Linux/Unix) or `~/Library/Application Support/apm/config.yaml` (macOS)
2. **System config**: `/etc/apm/config.yaml` 
3. **Environment variables**: `APM_*` prefixed variables (highest precedence)

### Configuration Commands

```bash
# Initialize user config directory and create default config file
apm config init

# Show which config file is currently active
apm config path

# Edit the active config file in $EDITOR
apm config edit

# Force overwrite existing config during init
apm config init --force
```

### Configuration File Format

The config file uses YAML format with all APM settings:

```yaml
# APM Configuration File
server:
  host: "0.0.0.0"
  port: 7337

storage:
  database_url: "sqlite:apm.db"
  log_retention_days: 7
  max_log_size_mb: 1000

ui:
  theme: "dark"
  dashboard_auth: "none"

mcp:
  enabled: false
  transport: "tcp"
  tcp_host: "127.0.0.1"
  tcp_port: 7338
  unix_socket: "/tmp/apm.sock"

access_control:
  mode: "open"  # open, strict, or unrestricted

search:
  enabled: true
  index_path: "./apm_search_index"
  commit_interval_seconds: 5
  buffer_size_mb: 50

cleanup:
  auto_clean_on_startup: false
  retention_hours: 168  # 7 days
  keep_logs: false
  keep_failed: true
```

### Environment Variable Overrides

Any configuration setting can be overridden using environment variables with the `APM_` prefix:

```bash
# Override MCP settings
APM_MCP_ENABLED=true
APM_MCP_TCP_PORT=9999

# Override server settings
APM_SERVER_PORT=8080
APM_SERVER_HOST=127.0.0.1

# Override storage settings  
APM_STORAGE_DATABASE_URL="sqlite:custom.db"
```

### MCP Configuration

To enable MCP server alongside HTTP API, add to your config file:

```yaml
mcp:
  enabled: true
  transport: "tcp"       # or "unix_socket"
  tcp_host: "127.0.0.1"
  tcp_port: 7338
  unix_socket: "/tmp/apm.sock"
```

Or use environment variables:
```bash
APM_MCP_ENABLED=1
APM_MCP_TRANSPORT=tcp
APM_MCP_TCP_PORT=7338
```

### Cleanup Configuration

APM supports automatic cleanup of stopped processes on daemon startup:

```yaml
cleanup:
  auto_clean_on_startup: true  # Enable auto-cleanup on daemon start
  retention_hours: 168         # Keep stopped processes for 7 days (0 = keep forever)
  keep_logs: false             # Delete logs when cleaning processes
  keep_failed: true            # Keep processes that failed (non-zero exit)
```

Or use environment variables:
```bash
APM_CLEANUP_AUTO_CLEAN_ON_STARTUP=true
APM_CLEANUP_RETENTION_HOURS=24
APM_CLEANUP_KEEP_LOGS=false
APM_CLEANUP_KEEP_FAILED=true
```

### MCP Working Directory Isolation

MCP clients automatically inherit working directory-based isolation with hierarchical access:
- The MCP server determines the client's working directory at runtime
- All MCP operations (spawn, list, logs, stop) respect hierarchical access rules
- Processes spawned via MCP are tagged with the server's working directory
- MCP clients in parent directories can manage subdirectory processes
- Example: An MCP client in `/project` can manage processes from `/project/backend`

### Adding New Features

1. **Pattern Detection**: Add new patterns in `src/logs/patterns.rs`
2. **API Endpoints**: Add routes in `src/api/mod.rs` and handlers in `src/api/handlers.rs`
3. **CLI Commands**: Add commands in `src/main.rs` using clap
4. **Process Features**: Extend `ProcessConfig` in `src/process/supervisor.rs`
5. **Agent Queries**: Add new query types in `AgentQuery` enum and implement handlers
6. **Log Analysis**: Extend `LogSummarizer` with new metrics and recommendation logic

### Database Schema

```sql
-- Log entries table
CREATE TABLE log_entries (
    id INTEGER PRIMARY KEY,
    process_id TEXT NOT NULL,
    timestamp REAL NOT NULL,
    raw_line TEXT NOT NULL,
    clean_line TEXT NOT NULL,
    level TEXT,
    patterns TEXT  -- JSON array of detected patterns
);
```

### WebSocket Protocol

Log streaming sends JSON messages:
```json
{
  "type": "log",
  "id": 123,
  "timestamp": 1234567890.123,
  "line": "Server started on port 8080",
  "level": "info",
  "patterns": [
    {"type": "port", "value": "8080", "context": "Server started on port 8080"}
  ]
}
```

### Resource Monitoring

Health metrics are automatically collected:
- CPU usage: Percentage of CPU used by process
- Memory usage: RSS memory in megabytes
- Updates every 2 seconds while process is running
- Exposed in all process info endpoints

## Enhanced AI Agent API

The structured query system provides 6 specialized query types:

### System Overview
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"type": "system_overview"}'
```

### Process Errors
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "process_errors",
    "time_window": "5m",
    "min_severity": "error"
  }'
```

### Port Mapping
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "port_mapping",
    "include_urls": true
  }'
```

### Performance Metrics
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "performance_metrics",
    "metrics": ["cpu", "memory"]
  }'
```

### Log Search
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "log_search",
    "pattern": "error",
    "limit": 10
  }'
```

### Event Correlation
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "event_correlation",
    "event_types": ["error", "key_event"],
    "time_window": "1h"
  }'
```

### Query Schema and Capabilities
```bash
# Get all available query types and their parameters
curl http://localhost:7337/api/agent/query-schema

# Get API capabilities
curl http://localhost:7337/api/agent/capabilities
```

## Enhanced Log Summarization

The LogSummarizer now provides real-time data analysis:

### Error Pattern Analysis
- Normalizes similar errors into patterns
- Tracks occurrence counts and timestamps
- Groups related errors for better insights

### Time-based Metrics
- Error rate (errors per minute)
- Warning rate (warnings per minute) 
- Log velocity (logs per minute)
- Time span analysis

### Intelligent Recommendations
- High error rate detection
- Resource usage alerts (CPU > 80%, memory > 1GB)
- Port conflict detection
- Log volume analysis

### Sample ProcessSummary Response
```json
{
  "status": "Running",
  "uptime": "2h 15m",
  "key_events": ["Server started", "Database connected"],
  "recent_errors": ["Connection timeout", "Auth failed"],
  "detected_urls": ["http://localhost:8080"],
  "detected_ports": [8080, 5432],
  "resource_usage": {
    "cpu_percent": "15.2%",
    "memory_mb": "256.8MB"
  },
  "error_patterns": [
    {
      "pattern": "connection [path] failed",
      "count": 5,
      "first_seen": "2023-01-01T10:00:00Z",
      "last_seen": "2023-01-01T10:15:00Z"
    }
  ],
  "metrics": {
    "total_logs": 1520,
    "error_rate": 2.3,
    "warning_rate": 0.8,
    "log_velocity": 12.5,
    "time_span_minutes": 135.0
  }
}
```

## Common Tasks

### Debugging WebSocket Issues
```bash
# Test WebSocket connection
wscat -c ws://localhost:7337/api/logs/<process-id>/stream

# Check daemon logs with debug enabled
RUST_LOG=agent_process_manager=debug,tower_http=debug ./target/debug/apm start
```

### Managing Process Lifecycle
```bash
# Spawn with tmux support (default)
apm spawn my-app python app.py

# Attach to running process (uses tmux)
apm attach my-app

# Access tmux session directly
tmux attach -t apm-<process-id>

# Monitor resources
curl http://localhost:7337/api/processes/<id>/health

# Get AI-friendly summary
curl http://localhost:7337/api/agent/summary

# Query with structured API
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"type": "system_overview"}'
```

### tmux Integration
```bash
# List APM tmux sessions
tmux list-sessions | grep apm-

# Send commands to a process
tmux send-keys -t apm-<id> "echo hello" Enter

# Capture pane content
tmux capture-pane -t apm-<id> -p

# Kill orphaned sessions
tmux kill-session -t apm-<id>
```

## Design Principles

1. **Context Efficiency**: Minimize token usage for AI agents while preserving full logs
2. **Real-time Monitoring**: Stream logs and metrics as they happen
3. **Pattern Intelligence**: Automatically extract meaningful information
4. **Human-Friendly**: Full access to raw logs when needed
5. **Independent Service**: No dependencies on specific AI platforms

## Task Management and Contributing

### GitHub Issues Workflow
All development tasks are now tracked through GitHub Issues with a comprehensive labeling system:

**Priority Labels**: `priority: critical/high/medium/low`
**Component Labels**: `component: api/cli/logs/process/tmux/websocket/ai-agent/config`
**Status Labels**: `status: needs-research/blocked/ready/in-progress/needs-review`
**Type Labels**: `type: bug/feature/enhancement/documentation/refactor/chore`

### Finding Work
- Browse [open issues](https://github.com/sunnya97/agent-process-manager/issues)
- Filter by component: `label:"component: api"` for API-related tasks
- Check [`good-first-issue`](https://github.com/sunnya97/agent-process-manager/labels/good-first-issue) for newcomer tasks
- Look for [`status: ready`](https://github.com/sunnya97/agent-process-manager/labels/status%3A%20ready) issues

### Development Process
1. **Pick an Issue**: Comment to claim it and ask questions if needed
2. **Create Branch**: `git checkout -b feature/issue-number-description`
3. **Implement**: Follow implementation guidelines
4. **Test**: Ensure all tests pass and add new tests
5. **Submit PR**: Reference issue number and provide clear description

### Implementation Guidelines
When implementing features:
1. Add comprehensive tests (see testing section above)
2. Update documentation (README.md, CLAUDE.md, inline docs)
3. Follow existing code patterns and Rust conventions
4. Consider backwards compatibility
5. Add feature flags for experimental features
6. Update issue status labels as you progress

### Useful GitHub CLI Commands
```bash
# View issues for a component
gh issue list --label "component: api"

# View high priority issues
gh issue list --label "priority: high"

# Create new issue
gh issue create --template feature_request.yml

# Assign issue to yourself
gh issue edit 123 --add-assignee @me
```

## Future Development

Current development priorities are tracked in GitHub Issues. Key areas include:
- Process groups and dependencies ([Issue #6](https://github.com/sunnya97/agent-process-manager/issues/6))
- Advanced restart policies ([Issue #3](https://github.com/sunnya97/agent-process-manager/issues/3))
- Log rotation and archival ([Issue #4](https://github.com/sunnya97/agent-process-manager/issues/4))
- Enhanced AI Agent API ([Issue #1](https://github.com/sunnya97/agent-process-manager/issues/1))
- Plugin system for custom patterns ([Issue #9](https://github.com/sunnya97/agent-process-manager/issues/9))