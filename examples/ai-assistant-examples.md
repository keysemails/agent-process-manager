# AI Assistant Examples - Real-World Scenarios

This document provides real-world examples of how AI assistants should use APM instead of getting blocked on long-running commands.

## Common Scenarios

### 1. Starting a React Development Server

**User Request**: "Can you start the development server for my React app?"

**❌ WRONG - This will block the AI assistant:**
```bash
npm run dev
# AI is now stuck waiting for this to end...
```

**✅ RIGHT - Using APM:**
```bash
# Start the dev server in background
apm spawn react-dev npm run dev

# Check it started successfully
apm logs react-dev | tail -20

# Confirm it's running
apm list
```

**AI Response**: "I've started the React development server. It's running on port 3000. You can view the logs anytime with `apm logs react-dev`."

### 2. Running Multiple Services

**User Request**: "I need to run both the frontend and backend servers"

**❌ WRONG - Sequential blocking:**
```bash
# Start backend
npm run backend
# Never reaches this line...
npm run frontend
```

**❌ WRONG - Using & is problematic:**
```bash
npm run backend &
npm run frontend &
# No way to manage these processes later
```

**✅ RIGHT - Using APM:**
```bash
# Start backend API
apm spawn backend npm run backend

# Start frontend dev server  
apm spawn frontend npm run frontend

# Check both are running
apm list

# View any errors
apm logs backend | grep -i error
apm logs frontend | grep -i error
```

### 3. Python Django Development

**User Request**: "Start the Django development server"

**❌ WRONG:**
```bash
python manage.py runserver
# Blocked forever...
```

**✅ RIGHT:**
```bash
# Start Django server
apm spawn django python manage.py runserver

# Check it's running
apm logs django | grep "Starting development server"

# If you need to run on a different port
apm spawn django python manage.py runserver 0.0.0.0:8080
```

### 4. Running Database Services

**User Request**: "Can you start PostgreSQL for development?"

**❌ WRONG:**
```bash
postgres -D /usr/local/var/postgres
# Blocked...
```

**✅ RIGHT:**
```bash
# Start PostgreSQL
apm spawn postgres postgres -D /usr/local/var/postgres

# Or using pg_ctl
apm spawn postgres pg_ctl -D /usr/local/var/postgres start

# Check it's ready
apm logs postgres | grep "database system is ready"
```

### 5. Running Build Watchers

**User Request**: "Set up TypeScript compilation with watch mode"

**❌ WRONG:**
```bash
tsc --watch
# Can't do anything else...
```

**✅ RIGHT:**
```bash
# Start TypeScript compiler in watch mode
apm spawn tsc-watch tsc --watch

# Also start the dev server
apm spawn dev-server npm run dev

# Monitor both
apm list
```

### 6. Jupyter Notebook

**User Request**: "Launch Jupyter notebook for data analysis"

**❌ WRONG:**
```bash
jupyter notebook
# Browser opens, but AI is stuck...
```

**✅ RIGHT:**
```bash
# Start Jupyter
apm spawn jupyter jupyter notebook --no-browser

# Get the access token
apm logs jupyter | grep -E "http://.*token="

# The user can now access Jupyter while AI continues working
```

### 7. Running Tests in Watch Mode

**User Request**: "Run tests in watch mode while I work"

**❌ WRONG:**
```bash
jest --watch
# Interactive mode blocks AI...
```

**✅ RIGHT:**
```bash
# Run tests in watch mode
apm spawn test-watch jest --watch --no-interactive

# Check test results
apm logs test-watch | tail -30
```

### 8. Docker Compose Services

**User Request**: "Start all the services defined in docker-compose.yml"

**❌ WRONG:**
```bash
docker-compose up
# Logs stream forever...
```

**✅ RIGHT:**
```bash
# Start services in background
apm spawn docker-services docker-compose up

# Check which services started
apm logs docker-services | grep "done"

# Alternative: use docker-compose -d, but APM gives better control
```

### 9. Real-time Log Monitoring

**User Request**: "I need to monitor application logs"

**❌ WRONG:**
```bash
tail -f app.log
# Stuck in tail forever...
```

**✅ RIGHT:**
```bash
# If monitoring a running APM process
apm logs app-server -f  # Built-in following

# For external log files, spawn a monitor
apm spawn log-monitor tail -f /var/log/app.log

# Check recent entries
apm logs log-monitor | tail -50
```

### 10. Running Multiple Microservices

**User Request**: "Start all microservices for local development"

**✅ RIGHT - Batch spawn approach:**
```bash
# Start all services
apm spawn auth-service npm run start:auth
apm spawn user-service npm run start:users  
apm spawn payment-service npm run start:payments
apm spawn gateway npm run start:gateway

# Check all are running
apm list

# Get a system overview
apm list | grep Running

# Check for port conflicts
apm logs auth-service | grep -i "address in use"
apm logs user-service | grep -i "address in use"
```

## MCP Tool Examples

When using MCP tools in Claude Code:

### Starting a Process
```typescript
// Instead of: await bash("npm run dev")
await mcp__agent_process_manager__spawn({
  name: "dev-server",
  command: "npm",
  args: ["run", "dev"]
})
```

### Checking Status
```typescript
// List all processes in current directory
const processes = await mcp__agent_process_manager__list({
  current_dir: true
})

// Find our dev server
const devServer = processes.find(p => p.name === "dev-server")
if (devServer && devServer.status === "Running") {
  console.log(`Dev server running on PID ${devServer.pid}`)
}
```

### Viewing Logs
```typescript
// Get recent logs
const logs = await mcp__agent_process_manager__logs({
  process_id: devServer.id,
  limit: 50
})

// Check for errors
const hasErrors = logs.some(line => line.toLowerCase().includes('error'))
```

### System Queries
```typescript
// Get system overview
const overview = await mcp__agent_process_manager__query({
  type: "system_overview",
  current_dir: true
})

// Check for port usage
const ports = await mcp__agent_process_manager__query({
  type: "port_mapping"
})
```

## Recovery Scenarios

### When AI Gets Blocked

If an AI assistant gets blocked on a long-running command:

1. **User sees no response**
2. **User interrupts/restarts session**
3. **On resume, AI should check for orphaned processes:**

```bash
# Check APM status
apm status

# List all processes
apm list --all

# Look for processes that might be from previous session
apm list | grep -E "(node|python|npm|yarn)"

# Stop any orphaned processes
apm stop-all --current-dir
```

### When APM Isn't Installed

```bash
# Check if APM is available
which apm || echo "APM not installed"

# If not available, inform user:
echo "I notice you're trying to run a development server that won't terminate."
echo "Would you like me to help install APM (Agent Process Manager)?"
echo "It prevents me from getting blocked on long-running commands."

# Quick install if user agrees:
cargo install agent-process-manager
# or
curl -sSL https://install.apm.dev | sh  # If install script is provided
```

## Best Practices Summary

1. **Always use APM for**:
   - Development servers (`npm run dev`, `yarn start`)
   - API servers (`python app.py`, `node server.js`)
   - Database processes
   - Build watchers (`--watch` flags)
   - Any process that runs indefinitely

2. **Name processes clearly**:
   - `frontend`, `backend`, `api`, `web`
   - `postgres`, `redis`, `mongo`
   - `build-watch`, `test-watch`

3. **Check logs after spawning**:
   - Wait 2-3 seconds for startup
   - Check for port binding errors
   - Verify successful initialization

4. **Clean up when done**:
   - `apm stop <name>` for specific processes
   - `apm stop-all --current-dir` for all in directory
   - `apm clean` to remove stopped process records

5. **Monitor resources**:
   - `apm list` shows CPU and memory usage
   - Watch for high resource consumption
   - Stop runaway processes promptly