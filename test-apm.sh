#!/bin/bash

# Test script for Agent Process Manager

echo "=== Agent Process Manager Test Script ==="
echo

# Kill any existing APM daemon
echo "Stopping any existing APM daemon..."
pkill -f "apm start" 2>/dev/null || true
sleep 1

# Start the daemon in the background
echo "Starting APM daemon in background..."
./target/debug/apm start &
DAEMON_PID=$!
sleep 2

# Check if daemon started
if ! ps -p $DAEMON_PID > /dev/null; then
    echo "❌ Failed to start APM daemon"
    exit 1
fi

echo "✅ APM daemon started (PID: $DAEMON_PID)"
echo

# Test 1: List processes (should be empty)
echo "Test 1: Listing processes (should be empty)..."
./target/debug/apm list
echo

# Test 2: Spawn a simple process
echo "Test 2: Spawning a test process..."
./target/debug/apm spawn "test-echo" "bash" "-c" "while true; do echo 'Hello from test process'; sleep 2; done" --pty
sleep 3

# Test 3: List processes again
echo "Test 3: Listing processes (should show test-echo)..."
./target/debug/apm list
echo

# Test 4: View logs
echo "Test 4: Viewing logs..."
./target/debug/apm logs test-echo
echo

# Test 5: Test the API
echo "Test 5: Testing API endpoints..."
echo "- Getting process list via API:"
curl -s http://localhost:7337/api/processes | jq '.' 2>/dev/null || echo "Install jq for pretty JSON output"
echo

echo "- Getting agent summary:"
curl -s http://localhost:7337/api/agent/summary | jq '.' 2>/dev/null || echo "Install jq for pretty JSON output"
echo

# Test 6: Stop the process
echo "Test 6: Stopping test process..."
./target/debug/apm stop test-echo
sleep 1

# Test 7: Final list (should be empty again)
echo "Test 7: Final process list (should be empty)..."
./target/debug/apm list
echo

# Cleanup
echo "Cleaning up..."
kill $DAEMON_PID 2>/dev/null
wait $DAEMON_PID 2>/dev/null

echo "✅ All tests completed!"
echo
echo "You can also test:"
echo "  - Start daemon: ./target/debug/apm start"
echo "  - Spawn a dev server: ./target/debug/apm spawn 'my-server' 'npm' 'run' 'dev' --pty"
echo "  - Attach to process: ./target/debug/apm attach my-server"
echo "  - View errors only: ./target/debug/apm logs my-server --errors"
echo "  - API docs at: http://localhost:7337/api"