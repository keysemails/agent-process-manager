# Agent Process Manager (APM)

A standalone service for AI-native process management that reduces context usage by ~90% while maintaining full human access to logs.

## Features

- 🚀 **Process Management**: Spawn and control long-running processes with PTY support
- 📊 **Resource Monitoring**: Real-time CPU and memory usage tracking
- 🔍 **Intelligent Pattern Detection**: Automatically extracts ports, URLs, errors, and key events
- 💾 **Dual Log Storage**: Raw logs for humans, structured summaries for AI agents
- 🌐 **REST API**: Full programmatic control
- 📡 **WebSocket Streaming**: Real-time log monitoring
- 🖥️ **CLI Interface**: Human-friendly command-line tool

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
```

## API Usage

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

## Architecture

APM consists of:
- **Daemon**: Background service managing processes
- **CLI**: Command-line interface for human interaction
- **API**: REST endpoints for programmatic control
- **Storage**: SQLite database for log persistence

## Testing

The project includes a comprehensive test suite covering unit tests, integration tests, and end-to-end tests.

### Test Categories

- **Unit Tests**: Core logic for process management, log storage, and pattern detection
- **Integration Tests**: API endpoints and database operations
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
cargo test --test cli_e2e_test                     # CLI commands

# Run benchmarks
cargo bench
```

See `test_summary.md` for detailed test documentation.

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
cargo test --test cli_e2e_test -- --test-threads=1

# Format code
cargo fmt

# Check lints
cargo clippy
```

## License

This project is part of the Vibe codebase but designed to be used independently.