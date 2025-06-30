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

## CLI Commands

```bash
# Daemon Management
apm start                          # Start the daemon
apm status                         # Check daemon status

# Process Management
apm spawn <name> <command> [args]  # Start a process (uses tmux by default)
apm list                          # List all processes
apm logs <name>                   # View process logs
apm stop <name>                   # Stop a process
apm restart <name>                # Restart a process

# Interactive
apm attach <name>                 # Attach to process (uses tmux attach)
```

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

# Test with example processes
./target/debug/apm spawn test-server python3 -- -m http.server 8080
./target/debug/apm spawn test-app node -- app.js

# Test AI Agent API
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"type": "system_overview"}'
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

## Future Enhancements

- Process groups and dependencies
- Advanced restart policies
- Log rotation and archival
- Distributed APM clustering
- Plugin system for custom patterns
- Process communication channels