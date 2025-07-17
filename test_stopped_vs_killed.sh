#!/bin/bash
# Test script for Stopped vs Killed status distinction

set -e

echo "Testing APM Stopped vs Killed status distinction..."

# Build the project
echo "Building APM..."
cargo build --release

# Start APM daemon if not running
echo "Checking APM daemon..."
if ! ./target/release/apm status > /dev/null 2>&1; then
    echo "Starting APM daemon..."
    ./target/release/apm start &
    sleep 2
fi

# Test 1: Natural exit (should be Stopped)
echo -e "\n=== Test 1: Natural Process Exit ==="
echo "Spawning process that exits naturally..."
./target/release/apm spawn test-natural "bash" "-c" "echo 'Running...'; sleep 2; echo 'Exiting naturally'; exit 0"
sleep 3

STATUS=$(./target/release/apm list | grep test-natural | awk '{print $3}')
echo "Status: $STATUS"
if [ "$STATUS" = "Stopped" ]; then
    echo "✓ Process correctly marked as Stopped"
else
    echo "✗ Expected Stopped, got $STATUS"
fi

# Test 2: Forced kill (should be Killed)
echo -e "\n=== Test 2: Forced Process Kill ==="
echo "Spawning long-running process..."
./target/release/apm spawn test-killed "bash" "-c" "echo 'Running forever...'; while true; do sleep 1; done"
sleep 2

echo "Killing process..."
./target/release/apm kill test-killed
sleep 1

STATUS=$(./target/release/apm list | grep test-killed | awk '{print $3}')
echo "Status: $STATUS"
if [ "$STATUS" = "Killed" ]; then
    echo "✓ Process correctly marked as Killed"
else
    echo "✗ Expected Killed, got $STATUS"
fi

# Test 3: Failed process (non-zero exit, should be Failed)
echo -e "\n=== Test 3: Failed Process Exit ==="
echo "Spawning process that fails..."
./target/release/apm spawn test-failed "bash" "-c" "echo 'Running...'; sleep 2; echo 'Failing...'; exit 1"
sleep 3

STATUS=$(./target/release/apm list | grep test-failed | awk '{print $3}')
echo "Status: $STATUS"
if [ "$STATUS" = "Failed" ]; then
    echo "✓ Process correctly marked as Failed"
else
    echo "✗ Expected Failed, got $STATUS"
fi

# Clean up
echo -e "\n=== Cleaning up ==="
./target/release/apm clean --force

echo -e "\nAll tests completed!"