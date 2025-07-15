#!/bin/bash

echo "Starting APM daemon in background..."
./target/debug/apm start &
DAEMON_PID=$!

# Wait for daemon to start
sleep 2

# Check if it started
if ps -p $DAEMON_PID > /dev/null; then
    echo "✅ Daemon started successfully (PID: $DAEMON_PID)"
    echo ""
    echo "Testing API health endpoint:"
    curl -s http://localhost:7337/api/health || echo "API not responding yet"
    echo ""
    echo ""
    echo "Now you can run in another terminal:"
    echo "  ./target/debug/apm spawn test-app echo 'Hello World' --pty"
    echo "  ./target/debug/apm list"
    echo "  ./target/debug/apm logs test-app"
    echo ""
    echo "To stop the daemon: kill $DAEMON_PID"
else
    echo "❌ Failed to start daemon"
fi