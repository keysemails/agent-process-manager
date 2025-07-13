# MCP Integration Tests

## Current Status

MCP (Model Context Protocol) integration tests are currently limited to basic unit tests due to the following challenges:

1. **Private APIs**: The `rmcp` crate used for MCP implementation doesn't expose public testing APIs
2. **Protocol Complexity**: MCP requires a full JSON-RPC handshake over TCP/Unix sockets
3. **Daemon Dependency**: Tests require starting the full APM daemon with MCP enabled

## Available Tests

### Unit Tests
- `tests/mcp_test.rs` - Basic MCP server creation test
- `tests/mcp_tcp_test.rs` - Configuration tests for TCP/Unix socket modes

### Manual Testing
We provide example scripts for manual testing:
- `examples/test_mcp_connection.py` - Python script to test MCP protocol
- `examples/test_mcp_connection.js` - Node.js script to test MCP protocol

### Testing Guide
See `docs/MCP_TESTING_GUIDE.md` for comprehensive testing instructions including:
- Using MCP Inspector
- Testing with official MCP SDK clients
- Integration with Claude Code
- Performance testing examples

## Running Manual Tests

1. Start APM with MCP enabled:
```bash
APM_MCP_ENABLED=1 cargo run -- start
```

2. Run test script:
```bash
# Python
python examples/test_mcp_connection.py

# Node.js
node examples/test_mcp_connection.js
```

## Future Improvements

1. Create a dedicated MCP client library for testing
2. Add integration tests using the official MCP SDK
3. Implement mock transport for unit testing
4. Add automated end-to-end tests in CI/CD

## Why Manual Testing?

While automated tests are ideal, MCP's architecture makes it challenging to test without a full client implementation. The manual test scripts provide:
- Verification that MCP protocol works correctly
- Examples for users implementing MCP clients
- Smoke tests that can be run in CI/CD
- Documentation of the expected protocol behavior