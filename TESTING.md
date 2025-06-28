# Testing Agent Process Manager

## Manual Testing Steps

### Terminal 1: Start the Daemon

```bash
# Build the project
cargo build

# Start the daemon (it will create apm.db automatically)
./target/debug/apm start
```

You should see output like:
```
[INFO] Starting Agent Process Manager v0.1.0
[INFO] API server listening on http://0.0.0.0:7337
```

### Terminal 2: Spawn and Manage Processes

```bash
# 1. List processes (should be empty)
./target/debug/apm list

# 2. Spawn a simple test process
./target/debug/apm spawn "counter" bash -c "i=0; while true; do echo Count: \$i; i=\$((i+1)); sleep 1; done" --pty

# 3. List processes again
./target/debug/apm list

# 4. View logs (live)
./target/debug/apm logs counter

# 5. Stop the process
./target/debug/apm stop counter
```

### Testing with a Real Dev Server

If you have a Node.js project:

```bash
# Navigate to a Node.js project
cd /path/to/node/project

# Spawn the dev server through APM
./path/to/apm spawn "my-app" npm run dev --pty

# In another terminal, check the logs
./path/to/apm logs my-app

# The APM should detect:
# - Port bindings (e.g., "Listening on port 3000")
# - URLs (e.g., "http://localhost:3000")
# - Errors if any occur
```

### Testing the API

```bash
# Get all processes
curl http://localhost:7337/api/processes

# Get a specific process (use the ID from the list)
curl http://localhost:7337/api/processes/{id}

# Get logs with AI-friendly summary
curl "http://localhost:7337/api/processes/{id}/logs?format=summary"

# Get only errors
curl "http://localhost:7337/api/processes/{id}/logs?format=errors"

# Get system summary for AI agents
curl http://localhost:7337/api/agent/summary

# Natural language query (basic implementation)
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{"question": "What ports are in use?"}'
```

### Testing Pattern Detection

Spawn a process that outputs different patterns:

```bash
./target/debug/apm spawn "pattern-test" bash -c '
echo "Server starting..."
echo "Listening on port 8080"
echo "API available at http://localhost:8080/api"
echo "ERROR: Database connection failed"
echo "Loaded config from /etc/app/config.json"
echo "WARN: Deprecation warning"
echo "Build complete in 3.2s"
' --pty

# Then check the logs
./target/debug/apm logs pattern-test

# And the summary
curl "http://localhost:7337/api/processes/{id}/logs?format=summary"
```

### Expected Patterns Detected

The APM should automatically detect:
- **Ports**: 8080
- **URLs**: http://localhost:8080/api
- **Errors**: "ERROR: Database connection failed"
- **Warnings**: "WARN: Deprecation warning"
- **File paths**: /etc/app/config.json
- **Key events**: "Server starting", "Build complete"

### Testing Multiple Processes

```bash
# Spawn multiple processes
./target/debug/apm spawn "web" python -m http.server 8000 --pty
./target/debug/apm spawn "api" python -m http.server 8001 --pty
./target/debug/apm spawn "worker" bash -c "while true; do echo Processing...; sleep 5; done" --pty

# List all
./target/debug/apm list

# Get system-wide summary
curl http://localhost:7337/api/agent/summary
```

### Troubleshooting

1. **Database errors**: The daemon will create `apm.db` in the current directory
2. **Port already in use**: Default port is 7337, set `APM_SERVER_PORT=8080` to change
3. **Process not found**: Use exact name or ID from `apm list`
4. **No logs showing**: Ensure process is spawned with `--pty` flag

### Advanced Testing

Test the WebSocket endpoints (requires a WebSocket client):

```javascript
// In browser console or Node.js with ws package
const ws = new WebSocket('ws://localhost:7337/api/processes/{id}/logs/stream');
ws.onmessage = (event) => console.log('Log:', event.data);
```

### Automated Test Script

Run the included test script:

```bash
./test-apm.sh
```

This will automatically test basic functionality and clean up afterwards.