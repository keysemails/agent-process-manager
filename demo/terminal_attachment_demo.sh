#!/bin/bash
# Terminal Attachment Demo Script

echo "=== Agent Process Manager - Terminal Attachment Demo ==="
echo

# Check if APM daemon is running
if ! curl -s http://localhost:7337/health > /dev/null 2>&1; then
    echo "Starting APM daemon..."
    ./target/debug/apm start &
    sleep 2
fi

echo "1. Spawning an interactive shell process..."
./target/debug/apm spawn demo-shell bash

echo
echo "2. Listing processes to see the shell running:"
./target/debug/apm list

echo
echo "3. You can now attach to the shell with:"
echo "   ./target/debug/apm attach demo-shell"
echo
echo "   Or in read-only mode:"
echo "   ./target/debug/apm attach demo-shell --read-only"
echo
echo "4. While attached:"
echo "   - Type commands like you would in a normal shell"
echo "   - Press Ctrl+Q, D to detach"
echo "   - Terminal resize is automatically handled"
echo
echo "5. The shell keeps running after you detach!"
echo
echo "Try it now!"