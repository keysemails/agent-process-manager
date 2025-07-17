#!/bin/bash

# Test script to spawn a PTY process and test SIGCHLD monitoring

# Create a test config that forces PTY usage
cat > /tmp/test-pty.json << 'EOF'
{
  "name": "test-pty-process",
  "command": "sleep",
  "args": ["3"],
  "use_tmux": false,
  "pty": true
}
EOF

# Use the API to spawn the process
curl -X POST http://localhost:7337/api/processes \
  -H "Content-Type: application/json" \
  -d @/tmp/test-pty.json

echo "Process spawned, waiting for exit..."
sleep 5

# Check status
curl -X GET http://localhost:7337/api/processes

# Clean up
rm -f /tmp/test-pty.json