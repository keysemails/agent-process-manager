# Agent Process Manager (APM)

A standalone service for AI-native process management that reduces context usage by ~90% while maintaining full human access to logs.

## Features

- 🚀 **Process Management**: Spawn and control long-running processes with PTY support
- 📊 **Resource Monitoring**: Real-time CPU and memory usage tracking
- 🔍 **Intelligent Pattern Detection**: Automatically extracts ports, URLs, errors, and key events with deduplication
- 💾 **Dual Log Storage**: Raw logs for humans, structured summaries for AI agents
- 🤖 **Enhanced AI Agent API**: Structured query system for efficient AI interaction
- 📈 **Intelligent Summarization**: Error pattern analysis, time-based metrics, and recommendations
- 🌐 **REST API**: Full programmatic control
- 📡 **WebSocket Streaming**: Real-time log monitoring
- 🖥️ **CLI Interface**: Human-friendly command-line tool
- 🔐 **Configurable Access Control**: Flexible read/write access modes with hierarchical directory-based isolation

## Installation

```bash
# Clone and build
git clone <repository>
cd agent-process-manager
cargo build --release

# Add to PATH (optional)
sudo cp target/release/apm /usr/local/bin/
```

## Quick Start

```bash
# Start the daemon
apm start

# Spawn a process
apm spawn my-server python -m http.server 8080

# List processes
apm list

# View logs
apm logs my-server

# Stop a process
apm stop my-server

# Stop all processes (with confirmation)
apm stop-all

# Stop all processes without confirmation
apm stop-all --force

# Clean up stopped processes
apm clean                          # Clean all stopped processes
apm clean --older-than 24          # Clean processes stopped >24 hours ago
apm clean --current-dir            # Clean only from current directory
apm clean --keep-logs              # Clean but preserve log data
```

## API Usage

### Process Management
```bash
# Spawn a process via API
curl -X POST http://localhost:7337/api/processes \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-app",
    "command": "node",
    "args": ["app.js"],
    "pty": true
  }'

# Get process health
curl http://localhost:7337/api/processes/<id>/health

# Stream logs via WebSocket
wscat -c ws://localhost:7337/api/logs/<id>/stream
```

### AI Agent API
```bash
# System overview query
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"type": "system_overview"}'

# Find errors in last 5 minutes
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "process_errors",
    "time_window": "5m"
  }'

# Discover port mappings
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "port_mapping",
    "include_urls": true
  }'

# Get performance metrics
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "performance_metrics",
    "metrics": ["cpu", "memory"]
  }'

# Search logs
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "log_search",
    "pattern": "error",
    "limit": 10
  }'

# Get query schema
curl http://localhost:7337/api/agent/query-schema

# Get API capabilities
curl http://localhost:7337/api/agent/capabilities
```

## AI Agent Benefits

APM dramatically reduces context usage for AI agents while maintaining full functionality:

### Context Reduction
- **~90% reduction** in token usage compared to raw log streaming
- Structured summaries replace verbose log dumps
- Pattern detection extracts only relevant information
- Time-based metrics provide insights without raw data

### Intelligent Analysis
- **Error Pattern Grouping**: Similar errors are normalized and counted
- **Resource Alerts**: Automatic detection of high CPU/memory usage
- **Port Conflict Detection**: Identifies processes competing for ports
- **Recommendation Engine**: Suggests actions based on system state

### Query Efficiency
- **6 Specialized Query Types**: System overview, error analysis, port mapping, performance metrics, log search, event correlation
- **Time Window Filtering**: Focus on recent events (5m, 1h, 24h, etc.)
- **Process Filtering**: Target specific applications
- **Structured Responses**: Consistent JSON format with metadata

### Example: Traditional vs APM Approach

**Traditional**: AI agent requests last 1000 log lines (~50KB of text)
```bash
# Returns massive text dump
curl http://server/logs?limit=1000
```

**APM**: AI agent gets structured summary (~2KB of JSON)
```bash
# Returns focused insights
curl -X POST http://localhost:7337/api/agent/query \
  -d '{"type": "system_overview"}'
```

## Architecture

APM consists of:
- **Daemon**: Background service managing processes with tmux integration
- **CLI**: Command-line interface for human interaction
- **HTTP API**: REST endpoints for programmatic control (port 7337)
- **MCP Server**: Model Context Protocol server for AI assistants (port 7338)
- **Storage**: SQLite database for log persistence and pattern detection
- **AI Agent API**: Structured query system for efficient AI interaction
- **Log Summarization**: Real-time analysis with error patterns and metrics
- **Pattern Detection**: Intelligent extraction of ports, URLs, errors, and events

## MCP (Model Context Protocol) Integration

APM includes built-in MCP server support, allowing AI assistants like Claude to directly manage processes:

### Enabling MCP
```yaml
# In apm.yaml
mcp:
  enabled: true
  transport: "tcp"      # or "unix_socket"
  tcp_port: 7338       # Default port
```

### Available MCP Tools
- `spawn`: Start new processes
- `list`: List all processes with status
- `logs`: Retrieve process logs
- `stop`: Stop processes
- `query`: Execute structured queries (same as AI Agent API)

The MCP server runs alongside the HTTP API when enabled, sharing the same process manager and log storage.

## Testing

The project includes a comprehensive test suite covering unit tests, integration tests, and end-to-end tests.

### Test Categories

- **Unit Tests**: Core logic for process management, log storage, and pattern detection
- **Integration Tests**: API endpoints and database operations
- **AI Agent API Tests**: Structured query system and response validation
- **Log Summarizer Tests**: Real-time data integration and error pattern analysis
- **End-to-End Tests**: CLI commands and full system workflows
- **Property Tests**: Fuzz testing for pattern detection
- **Performance Tests**: Benchmarks for critical paths

### Running Tests

```bash
# Run all tests
./run_tests.sh

# Run specific test categories
cargo test --lib                                    # Unit tests only
cargo test --test process_supervisor_test          # Process management
cargo test --test log_storage_test                 # Log storage
cargo test --test log_patterns_test                # Pattern detection
cargo test --test api_integration_test             # API endpoints
cargo test --test agent_api_test                   # AI Agent API
cargo test --test log_summarizer_test              # Log summarization
cargo test --test cli_e2e_test                     # CLI commands

# Run benchmarks
cargo bench
```

See `test_summary.md` for detailed test documentation.

## Contributing

We welcome contributions! Agent Process Manager uses GitHub Issues for project management with a comprehensive labeling system.

### Quick Start
1. **Find an Issue**: Browse [open issues](https://github.com/sunnya97/agent-process-manager/issues) or check [`good-first-issue`](https://github.com/sunnya97/agent-process-manager/labels/good-first-issue) for newcomer-friendly tasks
2. **Create a Branch**: `git checkout -b feature/issue-number-description`
3. **Make Changes**: Follow existing code patterns and add tests
4. **Submit PR**: Reference the issue number and provide clear description

### Issue Management
- 🐛 **Bug Reports**: Use our [bug report template](https://github.com/sunnya97/agent-process-manager/issues/new?template=bug_report.yml)
- ✨ **Feature Requests**: Use our [feature request template](https://github.com/sunnya97/agent-process-manager/issues/new?template=feature_request.yml)
- 📋 **Development Tasks**: General improvements and maintenance

### Priority & Component System
Issues are organized with priority levels (critical, high, medium, low) and component labels (api, cli, logs, process, tmux, websocket, ai-agent, config) for easy filtering and organization.

For detailed guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Development

```bash
# Run with debug logging
RUST_LOG=agent_process_manager=debug cargo run -- start

# Run tests
cargo test

# Run full test suite with all categories
./run_tests.sh

# Run specific test suites
cargo test --test process_supervisor_test -- --test-threads=1
cargo test --test log_storage_test -- --test-threads=1
cargo test --test log_patterns_test
cargo test --test api_integration_test -- --test-threads=1
cargo test --test agent_api_test                   # AI Agent API tests
cargo test --test log_summarizer_test              # Log summarization tests
cargo test --test cli_e2e_test -- --test-threads=1

# Format code
cargo fmt

# Check lints
cargo clippy
```

## License

This project is part of the Vibe codebase but designed to be used independently.