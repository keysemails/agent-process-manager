# APM Test Suite

Comprehensive test suite for the Agent Process Manager covering unit tests, integration tests, and end-to-end tests.

## Test Structure

### Unit Tests
Located in individual test modules within the source files and in the `tests/` directory:

- **Process Supervisor Tests** (`tests/process_supervisor_test.rs`)
  - Process spawning with/without PTY
  - Process lifecycle management (start, stop, restart)
  - Restart policies and failure handling
  - Environment variables and working directory
  - Concurrent process operations
  - Health metric collection

- **Log Storage Tests** (`tests/log_storage_test.rs`)
  - Log entry persistence and retrieval
  - Query filtering (by level, time, patterns)
  - Pagination support
  - Log summarization
  - Concurrent write operations
  - ANSI escape code handling

- **Pattern Detection Tests** (`tests/log_patterns_test.rs`)
  - Port number detection
  - URL extraction
  - Error keyword matching
  - File path detection
  - Property-based testing
  - Performance validation

### Integration Tests
- **API Integration Tests** (`tests/api_integration_test.rs`)
  - All REST endpoints
  - Error handling
  - Request/response validation
  - Health checks
  - Agent-specific endpoints

### End-to-End Tests
- **CLI E2E Tests** (`tests/cli_e2e_test.rs`)
  - Full command-line interface testing
  - Daemon lifecycle
  - Process management via CLI
  - Log viewing
  - Error scenarios

## Running Tests

### Run All Tests
```bash
./run_tests.sh
```

### Run Specific Test Categories
```bash
# Unit tests only
cargo test --lib

# Process supervisor tests
cargo test --test process_supervisor_test

# Log storage tests
cargo test --test log_storage_test

# Pattern detection tests
cargo test --test log_patterns_test

# API integration tests
cargo test --test api_integration_test

# CLI end-to-end tests
cargo test --test cli_e2e_test
```

### Run with Logging
```bash
RUST_LOG=agent_process_manager=debug cargo test -- --nocapture
```

### Run Benchmarks
```bash
cargo bench
```

## Test Dependencies

The test suite uses several additional dependencies:
- `tempfile` - Temporary file/directory creation
- `mockall` - Mocking framework (ready for use)
- `proptest` - Property-based testing
- `criterion` - Benchmarking
- `test-case` - Parameterized tests
- `wiremock` - HTTP mocking
- `serial_test` - Sequential test execution

## Writing New Tests

### Test Utilities
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

### Best Practices
1. Use `#[serial]` for tests that interact with the daemon
2. Clean up processes and resources after tests
3. Use descriptive test names
4. Test both success and failure cases
5. Use property-based testing for complex inputs
6. Keep tests isolated and independent

## Continuous Integration

The test suite is designed to run in CI environments:
- Tests automatically kill any existing daemons
- Serial execution prevents race conditions
- Temporary directories isolate file operations
- All tests clean up after themselves

## Coverage

To generate test coverage reports:
```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

## Known Issues

1. WebSocket tests require a running server (currently commented out)
2. Some timing-sensitive tests may occasionally fail on slow systems
3. CLI tests require the binary to be built first