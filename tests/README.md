# APM Test Suite

This directory contains all tests for the Agent Process Manager, organized by type and technology.

## 📁 Directory Structure

### 🧪 [unit/](unit/)
**Rust unit tests** - Fast tests that test individual components:
- `log_patterns_test.rs` - Pattern detection and matching
- `log_storage_test.rs` - SQLite storage operations
- `log_summarizer_test.rs` - Log analysis and summarization

### 🔧 [integration/](integration/)
**Integration tests** - Test component interactions:

#### [rust/](integration/rust/) - Rust integration tests
- `agent_api_test.rs` - AI Agent API testing
- `api_integration_test.rs` - HTTP API testing
- `mcp_*.rs` - MCP protocol testing
- `process_supervisor_test.rs` - Process management
- `search_test.rs` - Full-text search
- `terminal_attachment_test.rs` - Terminal features

#### [python/](integration/python/) - Python-based tests
- `test_mcp_*.py` - MCP protocol testing
- `test_auto_cleanup.py` - Cleanup functionality
- `test_list_display.py` - CLI display testing
- Uses pytest with fixtures in `conftest.py`

#### [shell/](integration/shell/) - Shell script tests
- `test_persistence.sh` - Data persistence
- `test_hierarchical_access.sh` - Access control
- `test_mcp*.sh` - MCP shell testing

### 🌍 [e2e/](e2e/)
**End-to-end tests** - Full workflow testing:
- `test_full_workflow.py` - Complete user workflows
- `cli_e2e_test.rs` - CLI end-to-end testing

### 📋 [fixtures/](fixtures/)
**Test data and configuration**:
- `configs/` - Test configuration files
- `data/` - Test data and regression files

### 🔧 [scripts/](scripts/)
**Test runner scripts**:
- `run_all.sh` - Run complete test suite
- `quick_test.sh` - Fast subset of tests
- `test_apm.sh` - APM-specific tests

## 🚀 Running Tests

### All Tests
```bash
# Run complete test suite
./tests/scripts/run_all.sh

# Quick test subset
./tests/scripts/quick_test.sh
```

### By Category
```bash
# Unit tests only
cargo test --lib

# Integration tests
cargo test --test "*"

# Python tests
cd tests/integration/python
python -m pytest

# Shell tests
./tests/integration/shell/test_*.sh
```

### Specific Tests
```bash
# Specific Rust test
cargo test --test agent_api_test

# Specific Python test
python -m pytest tests/integration/python/test_mcp_basic.py

# MCP tests only
cargo test mcp
python -m pytest tests/integration/python/test_mcp_*.py
```

## 🔧 Test Configuration

### Prerequisites
- Rust toolchain
- Python 3.7+ with pytest
- APM binary built (`cargo build`)

### Environment Variables
- `RUST_LOG=debug` - Enable debug logging
- `APM_TEST_TIMEOUT=30` - Test timeout in seconds

### Test Data
- Fixtures in `tests/fixtures/`
- Temporary workspaces created per test
- Automatic cleanup after tests

## 📚 Writing Tests

### Rust Tests
- Unit tests: Test individual functions/modules
- Integration tests: Test component interactions
- Use `#[tokio::test]` for async tests
- See existing tests for patterns

### Python Tests
- Use pytest fixtures from `conftest.py`
- Test MCP protocol and CLI interactions
- Automatic APM daemon management
- Temporary workspace per test

### Shell Tests
- Test CLI behavior and output
- Use common test utilities
- Proper cleanup and error handling

## 🔍 Test Categories

### By Speed
- **Fast**: Unit tests (`cargo test --lib`)
- **Medium**: Integration tests (`cargo test --test`)
- **Slow**: E2E tests and Python tests

### By Component
- **Core**: Process management, logging
- **API**: HTTP API, WebSocket, AI Agent API
- **MCP**: Model Context Protocol
- **CLI**: Command-line interface
- **Search**: Full-text search functionality

### By Platform
- **Cross-platform**: Most tests
- **Unix-only**: tmux integration tests

## 🧪 Test Utilities

Use the test utilities in `src/test_utils.rs`:
```rust
use agent_process_manager::test_utils::test_utils::*;

// Create test database
let (storage, _temp_dir) = create_test_storage().await?;

// Create test process info
let process = create_test_process_info("my-process");

// Create test log entries
let logs = create_test_log_entries("process-id", 10);
```

## ✅ Best Practices

1. Use `#[serial]` for tests that interact with the daemon
2. Clean up processes and resources after tests
3. Use descriptive test names
4. Test both success and failure cases
5. Use property-based testing for complex inputs
6. Keep tests isolated and independent

## 📊 Coverage

To generate test coverage reports:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## ⚠️ Known Issues

1. WebSocket tests require a running server (currently commented out)
2. Some timing-sensitive tests may occasionally fail on slow systems
3. CLI tests require the binary to be built first