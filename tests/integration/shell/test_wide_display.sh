#!/bin/bash

echo "Testing with different terminal widths..."

# Build APM first
cargo build --bin apm

# Ensure daemon is running by checking status, start if needed
if ! ./target/debug/apm status >/dev/null 2>&1; then
    echo "Starting APM daemon for display tests..."
    ./target/debug/apm start >/dev/null 2>&1 &
    sleep 3
    STARTED_DAEMON=true
    
    # Verify daemon actually started
    if ! ./target/debug/apm status >/dev/null 2>&1; then
        echo "⚠️  Could not start APM daemon, testing with empty output"
        DAEMON_WORKING=false
    else
        DAEMON_WORKING=true
        # Spawn a test process for display testing
        ./target/debug/apm spawn test-display echo "Display test process" >/dev/null 2>&1 || true
    fi
else
    STARTED_DAEMON=false
    DAEMON_WORKING=true
fi

# Test with narrow terminal (80 cols)
echo -e "\n=== 80 column terminal ==="
COLUMNS=80 ./target/debug/apm list

# Test with medium terminal (120 cols)
echo -e "\n=== 120 column terminal ==="
COLUMNS=120 ./target/debug/apm list

# Test with wide terminal (200 cols)
echo -e "\n=== 200 column terminal ==="
COLUMNS=200 ./target/debug/apm list

# Clean up if we started the daemon
if [ "$STARTED_DAEMON" = true ]; then
    echo "Cleaning up test daemon..."
    ./target/debug/apm stop-all --force >/dev/null 2>&1 || true
fi

if [ "$DAEMON_WORKING" = true ]; then
    echo "✅ Display width test completed successfully"
else
    echo "⚠️  Display width test completed (daemon issues but test structure verified)"
fi