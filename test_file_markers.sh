#!/bin/bash
# Test script for file marker system

set -e

echo "Testing APM file marker system..."

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

# Spawn a test process that exits quickly
echo "Spawning test process..."
PROCESS_ID=$(./target/release/apm spawn test-exit "bash" "-c" "echo 'Hello from test process'; sleep 2; echo 'Process exiting normally'; exit 0" | grep -oP 'Process ID: \K[^ ]+')
echo "Process ID: $PROCESS_ID"

# Monitor for exit markers
echo "Monitoring for exit markers..."
SESSION_NAME="apm-$PROCESS_ID"
MARKER_FILE="/tmp/$SESSION_NAME.exited"
EXIT_CODE_FILE="/tmp/$SESSION_NAME.exit-code"

# Wait for up to 10 seconds for the marker to appear
for i in {1..100}; do
    if [ -f "$MARKER_FILE" ]; then
        echo "✓ Exit marker detected after ~${i}00ms"
        
        if [ -f "$EXIT_CODE_FILE" ]; then
            EXIT_CODE=$(cat "$EXIT_CODE_FILE")
            echo "✓ Exit code captured: $EXIT_CODE"
        else
            echo "✗ Exit code file not found"
        fi
        
        # Check process status
        sleep 1  # Give APM time to update status
        STATUS=$(./target/release/apm list | grep test-exit | awk '{print $3}')
        echo "Process status: $STATUS"
        
        if [ "$STATUS" = "Stopped" ]; then
            echo "✓ Process correctly marked as Stopped (natural exit)"
        else
            echo "✗ Process status is $STATUS, expected Stopped"
        fi
        
        exit 0
    fi
    sleep 0.1
done

echo "✗ Exit marker not detected within 10 seconds"
exit 1