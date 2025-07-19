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
POST   /api/processes              # Spawn new process (with optional tags)
GET    /api/processes              # List all processes (supports tag filtering)
GET    /api/processes/:id          # Get process details
DELETE /api/processes/:id          # Stop process
POST   /api/processes/:id/restart  # Restart process
GET    /api/processes/:id/health   # Get health metrics

# Tag Management
POST   /api/processes/:id/tags     # Add tag to process
DELETE /api/processes/:id/tags/:tag # Remove tag from process
GET    /api/processes/:id/tags     # Get tags for process
GET    /api/tags                   # Get all unique tags

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
APM runs an MCP server by default that AI assistants can connect to:
- **TCP Mode**: Listens on `tcp://127.0.0.1:7338` by default
- **Unix Socket Mode**: Listens on `/tmp/apm.sock` by default
- **Transport**: TCP or Unix socket (no longer uses stdio)

#### MCP Tools Available:

1. **spawn** - Start a new process
   - `name`: Process name (required)
   - `command`: Command to execute (required)
   - `args`: Command arguments (optional)
   - `tags`: List of tags for categorization (optional)

2. **list** - List processes with health metrics
   - `current_dir`: Filter to current directory only (optional, default: false)
   - `tags`: List of tags to filter by (OR logic) (optional)
   - `all_tags`: List of tags that process must have all of (AND logic) (optional)
   - Returns: id, name, command, status, session_pid (tmux session), process_pid (application process), process_name (application name), cpu_percent, memory_mb, detected_ports, tags
   - **NEW**: `process_pid` and `process_name` show the real running application inside tmux sessions (e.g., 'node', 'python') rather than just the shell

3. **logs** - Get process logs with filtering
   - `process_id`: Process ID (required)
   - `limit`: Max log entries (optional, default: 100)
   - `search`: Text pattern filter (optional)
   - `level`: Log level filter - debug/info/warn/error (optional)
   - `since`: RFC3339 timestamp for time filtering (optional)

4. **kill** - Kill a single process (terminate tmux session)
   - `process_id`: Process ID (required)

5. **restart** - Restart a process
   - `process_id`: Process ID (required)
   - Returns: new process_id, restart_count

6. **kill_multiple** - Kill multiple processes
   - `current_dir`: Only kill processes from current directory (optional)
   - `names`: List of process names to kill (optional)
   - `force`: Skip confirmation (optional)
   - Returns: killed count, failed count, errors

7. **clean** - Clean stopped processes
   - `older_than`: Hours threshold (optional)
   - `keep_logs`: Preserve logs when cleaning (optional, default: false)
   - `current_dir`: Only clean from current directory (optional)
   - Returns: cleaned count, process names

8. **query** - Structured queries for AI agents
   - `type`: Query type (required) - one of:
     - `system_overview`: Overall system status
     - `process_errors`: Error analysis across processes
     - `port_mapping`: Port and URL usage mapping
     - `performance_metrics`: CPU/memory metrics and alerts
     - `log_search`: Search logs across processes (simple pattern matching)
     - `event_correlation`: Correlate events across processes
   - Common parameters:
     - `current_dir`: Filter to current directory (optional)
   - Query-specific parameters:
     - `time_window`: For time-based queries (e.g., '5m', '1h')
     - `process_filter`: Filter by process names
     - `include_urls`: Include URLs in port_mapping
     - `metrics`: Metrics to include ['cpu', 'memory']
     - `pattern`: Search pattern for log_search (required for log_search)
     - `limit`: Result limit for log_search
     - `event_types`: Event types for correlation (required for event_correlation)
     - `min_severity`: Minimum severity for process_errors

9. **tag** - Manage process tags
   - `action`: Action to perform - "add", "remove", or "list" (required)
   - `process_id`: Process ID (required for add/remove, optional for list)
   - `tag`: Tag to add/remove (required for add/remove)
   - Returns: Updated tags list or all unique tags across processes

10. **search** - Full-text search across all process logs (if search is enabled)
   - `query`: Lucene-compatible search query (required)
     - Simple: `"error"`, `"database connection"`
     - Boolean: `"error AND timeout"`, `"database OR cache"`, `"error NOT retry"`
     - Fuzzy: `"databse~"` (finds "database"), `"conection~2"` (2 edits allowed)
     - Wildcards: `"time*"`, `"*base"`, `"dat?base"`
     - Phrases: `"connection timeout"` (exact phrase)
     - Field search: `"level:error"`, `"process_id:12345"`
   - `process_id`: Filter by specific process ID (optional)
   - `level`: Filter by log level - debug/info/warn/error (optional)
   - `since`: RFC3339 timestamp for start of time range (optional)
   - `until`: RFC3339 timestamp for end of time range (optional)
   - `limit`: Max results, default 50, max 1000 (optional)
   - `offset`: For pagination (optional)
   - `highlight`: Include highlighted snippets (optional)
   - Returns: Relevance-ranked results with scores, timestamps, and patterns
   
   **When to use search vs logs vs query:**
   - Use `search` for: Complex queries, cross-process searches, fuzzy matching, relevance ranking
   - Use `logs` for: Simple filtering within one process, recent logs only
   - Use `query` with `log_search` for: Pattern matching when full-text search is disabled

## CLI Commands

```bash
# Daemon Management
apm start                          # Start the daemon
apm status                         # Check daemon status

# Process Management
apm spawn <name> <command> [args]  # Start a process (uses tmux by default)
apm spawn <name> <command> --tag tag1 --tag tag2  # Start with tags
apm list                          # List processes visible from current directory
apm list --all                    # List all processes regardless of directory
apm list --tag web --tag api      # List processes with web OR api tags
apm list --tags-any "backend,db"  # List processes with backend OR db tags
apm list --tags-all "web,prod"    # List processes with web AND prod tags
apm logs <name>                   # View process logs (from current directory)
apm logs <name> --all             # View logs of any process
apm kill <name>                   # Kill a process (from current directory)
apm kill <name> --all             # Kill any process
apm kill-all                      # Kill all processes (with confirmation)
apm kill-all --force              # Kill all processes without confirmation
apm kill-all --current-dir        # Kill only processes from current directory
apm restart <name>                # Restart a process (from current directory)
apm restart <name> --all          # Restart any process
apm clean                         # Clean all stopped processes (with confirmation)
apm clean --force                 # Clean without confirmation
apm clean --older-than 24         # Clean processes stopped >24 hours ago
apm clean --current-dir           # Clean only from current directory
apm clean --keep-logs             # Clean but preserve log data

# Tag Management
apm tag add <name> <tag>          # Add tag to process
apm tag remove <name> <tag>       # Remove tag from process
apm tag list <name>               # List tags for process
apm tag all                       # List all unique tags

# Configuration Management
apm config init                   # Create user config file with defaults
apm config init --force           # Overwrite existing config file
apm config path                   # Show config and data directory paths
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

APM uses a global configuration system following platform-specific conventions for config and data directories. 

### Directory Structure

**Configuration Files:**
- **User config**: `~/.config/apm/config.yaml` (Linux/Unix) or `~/Library/Application Support/apm/config.yaml` (macOS)
- **System config**: `/etc/apm/config.yaml`

**Data Files:**
- **User data**: `~/.local/share/apm/` (Linux/Unix) or `~/Library/Application Support/apm/data/` (macOS)
  - `apm.db`, `apm.db-shm`, `apm.db-wal` - SQLite database files
  - `search_index/` - Full-text search index directory
  - `logs/` - Additional log storage (if configured)

### Configuration Precedence

Configuration is loaded in order of precedence:
1. **Environment variables**: `APM_*` prefixed variables (highest precedence)
2. **User config**: Platform-specific user config file
3. **System config**: `/etc/apm/config.yaml` (lowest precedence)

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
  database_url: "sqlite:~/.local/share/apm/apm.db"  # Linux
  # database_url: "sqlite:~/Library/Application Support/apm/data/apm.db"  # macOS
  log_retention_days: 7
  max_log_size_mb: 1000

ui:
  theme: "dark"
  dashboard_auth: "none"

mcp:
  enabled: true
  transport: "tcp"
  tcp_host: "127.0.0.1"
  tcp_port: 7338
  unix_socket: "/tmp/apm.sock"

access_control:
  mode: "open"  # open, strict, or unrestricted

search:
  enabled: true
  index_path: "~/.local/share/apm/search_index"  # Linux
  # index_path: "~/Library/Application Support/apm/data/search_index"  # macOS  
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

### Enhanced MCP Capabilities for AI Agents

The MCP interface now provides comprehensive process management capabilities optimized for AI agent usage:

1. **Efficient Context Usage**: All queries return structured, summarized data to minimize token consumption
2. **Advanced Log Filtering**: Search, filter by level, and time-based queries reduce noise
3. **Batch Operations**: `stop_multiple` and `clean` tools enable efficient bulk management
4. **Health Monitoring**: CPU and memory metrics included in list output for proactive monitoring
5. **Structured Queries**: The `query` tool provides 6 specialized query types for different analysis needs
6. **Pattern Detection**: Automatic extraction of ports, URLs, errors, and key events

#### Example MCP Usage for AI Agents:

```javascript
// List processes with actual command information
const processes = await mcp.call('list', {
  current_dir: false
});

// Filter processes by tags (OR logic)
const webProcesses = await mcp.call('list', {
  tags: ['web', 'frontend']
});

// Filter processes by tags (AND logic)
const prodWebProcesses = await mcp.call('list', {
  all_tags: ['web', 'production']
});
// Returns processes with process_pid and process_name showing real applications:
// [
//   {
//     "id": "abc123",
//     "name": "web-server",
//     "command": "node",
//     "session_pid": 1234,     // Tmux session PID
//     "process_pid": 1456,     // Real Node.js application PID
//     "process_name": "node",  // Real application name
//     "cpu_percent": 15.8,     // Node.js CPU usage (not shell)
//     "memory_mb": 256,        // Node.js memory usage (not shell)
//     "detected_ports": [8080]
//   }
// ]

// Find all processes using high CPU
const result = await mcp.call('query', {
  type: 'performance_metrics',
  metrics: ['cpu'],
  current_dir: false
});

// Search for specific errors across all processes (simple pattern matching)
const errors = await mcp.call('query', {
  type: 'log_search',
  pattern: 'connection refused',
  limit: 20
});

// Full-text search with advanced query syntax (if search is enabled)
const searchResults = await mcp.call('search', {
  query: 'error AND (timeout OR refused) NOT retry',
  level: 'error',
  since: '2024-01-01T10:00:00Z',
  limit: 50
});

// Fuzzy search to handle typos
const fuzzyResults = await mcp.call('search', {
  query: 'databse~ OR conection~',  // Finds "database" and "connection"
  highlight: true
});

// Get filtered logs with level and time constraints
const logs = await mcp.call('logs', {
  process_id: '12345',
  level: 'error',
  since: '2024-01-01T00:00:00Z',
  limit: 50
});

// Clean up old stopped processes
const cleaned = await mcp.call('clean', {
  older_than: 24,  // hours
  keep_logs: false
});

// Spawn process with tags
const proc = await mcp.call('spawn', {
  name: 'web-server',
  command: 'node',
  args: ['app.js'],
  tags: ['web', 'production', 'frontend']
});

// Manage tags
const updatedTags = await mcp.call('tag', {
  action: 'add',
  process_id: proc.id,
  tag: 'critical'
});

const allTags = await mcp.call('tag', {
  action: 'list'
});
```

#### AI Agent Best Practices for Process Management:

```javascript
// 1. Detect resource-intensive processes by application type
const processes = await mcp.call('list', {});
const highCpuProcesses = processes.filter(p => 
  p.cpu_percent > 80 && p.process_name // Only actual running applications
);

// 2. Identify process types for targeted troubleshooting
const nodeProcesses = processes.filter(p => p.process_name === 'node');
const pythonProcesses = processes.filter(p => p.process_name === 'python');

// 3. Monitor memory usage by application process, not session
const memoryHogs = processes.filter(p => 
  p.process_pid && p.memory_mb > 500 // Only check actual applications
);

// 4. Provide technology-specific advice
function getProcessAdvice(process) {
  if (process.process_name === 'node' && process.memory_mb > 1000) {
    return "High memory usage detected in Node.js process. Consider checking for memory leaks.";
  }
  if (process.process_name === 'python' && process.cpu_percent > 90) {
    return "High CPU usage in Python process. Check for infinite loops or heavy computations.";
  }
  return "Process running normally.";
}
```

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

## Full-Text Search Best Practices

When using APM's search functionality:

### Query Syntax Tips
- **Use quotes for exact phrases**: `"connection timeout"` finds the exact phrase
- **Boolean operators must be uppercase**: `AND`, `OR`, `NOT`
- **Parentheses for complex queries**: `(error OR warn) AND database`
- **Field-specific search**: `level:error`, `process_id:backend-api`
- **Fuzzy search for typos**: `databse~` or `conection~2` (allows 2 edits)
- **Wildcards**: `time*` matches "timeout", "timestamp", etc.

### Performance Optimization
- **Use specific queries**: `"database connection error"` is faster than just `error`
- **Add filters when possible**: Combine query with level, process_id, or time range
- **Limit results appropriately**: Default 50 is usually sufficient
- **Use pagination for large results**: Set offset for subsequent queries

### When to Use Each Tool
1. **Use `search` when**:
   - Searching across multiple processes
   - Need fuzzy matching or typo tolerance
   - Complex boolean queries required
   - Want relevance-ranked results
   - Historical search beyond recent logs

2. **Use `logs` when**:
   - Filtering within a single known process
   - Need real-time log streaming
   - Simple text filtering is sufficient
   - Want raw log output

3. **Use `query` with `log_search` when**:
   - Full-text search is not enabled
   - Simple pattern matching is adequate
   - Need to correlate with other metrics

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

## Changelog

### v0.3.0 (2025-07-17)

**Breaking Changes:**
- Renamed all "stop" commands to "kill" to accurately reflect that they forcefully terminate processes
- Added new `ProcessStatus::Killed` state to distinguish forced termination from natural exits
- MCP tool renamed: `stop` → `kill`, `stop_multiple` → `kill_multiple`

**New Features:**
- **Instant Process Exit Detection**: Implemented file marker approach for immediate detection of process exits
  - Process exits are now detected in <100ms instead of up to 30 seconds
  - Exit codes are captured and stored via `/tmp/apm-{session}.exit-code` files
  - Distinguishes between natural exits (Stopped) and forced termination (Killed)
- **SIGCHLD-based Monitoring**: Added real-time process exit detection for PTY processes
  - Direct child processes report exit immediately via SIGCHLD signal
  - ProcessExitMonitor handles signal events and process reaping

**Technical Improvements:**
- Added `src/process/exit_monitor.rs` for centralized exit event handling
- Modified tmux wrapper commands to create exit marker files on process completion
- Updated monitoring loop to check file markers before expensive tmux operations
- Reduced polling overhead by prioritizing file-based detection

**Status Distinctions:**
- `Stopped`: Process exited naturally (exit code available)
- `Killed`: Process/session was forcefully terminated
- `Failed`: Process exited with non-zero exit code

## Process Management with APM

This project uses Agent Process Manager (APM) for all background processes.
**NEVER run development servers or long-running processes directly**.

### Critical Rules

- NEVER run `npm run dev`, `npm start`, `yarn dev` directly
- NEVER run `python manage.py runserver`, `flask run` directly  
- NEVER run commands that don't terminate naturally

### Use APM Instead

- cargo run → apm spawn app cargo run
- cargo watch -x run → apm spawn dev cargo watch -x run

### Common Commands

- Check running processes: `apm list`
- View logs: `apm logs <name>`
- Stop processes: `apm stop <name>`
- Stop all: `apm stop-all --current-dir`

See: https://github.com/sunnya97/agent-process-manager/blob/main/APM_FOR_AI_ASSISTANTS.md
