# Agent Process Manager Test Suite

## Overview

A comprehensive test suite has been created for the Agent Process Manager (APM) project, covering unit tests, integration tests, and end-to-end tests.

## Test Structure

### 1. Test Utilities (`src/test_utils.rs`)
- Helper functions for creating test databases, process configs, and log entries
- Mock utilities for predictable testing scenarios
- Async condition waiting helpers

### 2. Unit Tests

#### Process Supervisor Tests (`tests/process_supervisor_test.rs`)
- ✅ Process spawning (with/without PTY)
- ✅ Process lifecycle management (start, stop, restart)
- ✅ Environment variables and working directory
- ✅ Restart policies and failure handling
- ✅ Concurrent process operations
- ✅ Health metric collection
- ✅ Process not found error handling

#### Log Storage Tests (`tests/log_storage_test.rs`)
- ✅ Store and retrieve logs
- ✅ Query by log level
- ✅ Query by time range
- ✅ Search patterns in logs
- ✅ Pagination with limit/offset
- ✅ Raw log retrieval
- ✅ Delete process logs
- ✅ Log summarization
- ✅ Concurrent log writes
- ✅ ANSI escape code stripping

#### Pattern Detection Tests (`tests/log_patterns_test.rs`)
- ✅ Port number detection
- ✅ URL extraction
- ✅ Error keyword matching
- ✅ File path detection
- ✅ Multiple patterns in single line
- ✅ Key event detection
- ✅ IP address detection
- ✅ Property-based testing with proptest
- ✅ Performance validation

### 3. Integration Tests

#### API Integration Tests (`tests/api_integration_test.rs`)
- ✅ Health endpoint
- ✅ Process spawning via API
- ✅ List processes
- ✅ Get specific process
- ✅ Stop/restart processes
- ✅ Log retrieval with filters
- ✅ Raw log endpoint
- ✅ Process health metrics
- ✅ Error handling (404, invalid JSON, etc.)

### 4. End-to-End Tests

#### CLI E2E Tests (`tests/cli_e2e_test.rs`)
- ✅ Daemon lifecycle
- ✅ Process spawning via CLI
- ✅ Process listing
- ✅ Log viewing
- ✅ Stop/restart processes
- ✅ Process with arguments
- ✅ Invalid command handling
- ✅ Help and version commands

## Test Dependencies Added

```toml
[dev-dependencies]
tempfile = "3.12"          # Temporary file/directory creation
mockall = "0.13"           # Mocking framework
proptest = "1.5"           # Property-based testing
criterion = "0.5"          # Benchmarking
test-case = "3.3"          # Parameterized tests
wiremock = "0.6"           # HTTP mocking
serial_test = "3.1"        # Sequential test execution
hyper = "1.6"              # HTTP body handling
```

## Running Tests

### Run All Tests
```bash
./run_tests.sh
```

### Run Specific Test Suites
```bash
# Unit tests
cargo test --lib

# Process supervisor tests
cargo test --test process_supervisor_test -- --test-threads=1

# Log storage tests  
cargo test --test log_storage_test -- --test-threads=1

# Pattern detection tests
cargo test --test log_patterns_test

# API integration tests
cargo test --test api_integration_test -- --test-threads=1

# CLI end-to-end tests
cargo test --test cli_e2e_test -- --test-threads=1
```

### Run Benchmarks
```bash
cargo bench
```

## Key Features Tested

1. **Process Management**: Full lifecycle testing including spawning, monitoring, stopping, and restarting
2. **Log Handling**: Storage, retrieval, filtering, and pattern detection
3. **API Functionality**: All REST endpoints with proper error handling
4. **CLI Operations**: Complete command-line interface testing
5. **Concurrency**: Parallel operations and race condition testing
6. **Performance**: Benchmarks for critical paths like pattern detection

## Test Coverage Areas

- ✅ Core business logic
- ✅ API contract validation
- ✅ Error scenarios
- ✅ Edge cases
- ✅ Performance characteristics
- ✅ Concurrent operations
- ✅ CLI user workflows

## Notes

- Tests use `serial_test` for operations that interact with the daemon to prevent conflicts
- The test suite is designed to be CI/CD friendly with proper cleanup
- All tests are self-contained and don't require external dependencies
- WebSocket streaming tests are documented but require a running server (commented out)