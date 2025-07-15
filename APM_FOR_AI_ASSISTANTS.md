# APM for AI Assistants - Critical Instructions

## 🚨 CRITICAL: Never Block on Long-Running Commands

**NEVER run development servers or long-running processes directly with Bash.**
**ALWAYS use APM for any command that doesn't complete within 30 seconds.**

### ❌ Commands That Will Block You (NEVER RUN DIRECTLY)

```bash
# Web Development Servers
npm run dev
npm start
yarn dev
yarn start
pnpm dev
bun dev

# Python Servers
python manage.py runserver
python -m http.server
flask run
uvicorn main:app
streamlit run app.py
jupyter notebook
jupyter lab

# Other Language Servers
node server.js
node app.js
deno run --allow-net server.ts
cargo run (when it's a server)
cargo watch
go run main.go (when it's a server)
rails server
rails s
php -S localhost:8000
php artisan serve

# Build Watchers
npm run watch
webpack --watch
tsc --watch
nodemon
pm2 start

# Any command that shows:
# "Listening on port..."
# "Server running at..."
# "Press Ctrl+C to stop"
# "Watching for changes..."
```

### ✅ Always Use APM Instead

```bash
# WRONG - This blocks your execution:
npm run dev

# RIGHT - This runs in background:
apm spawn dev-server npm run dev

# WRONG - Even with & this is problematic:
python manage.py runserver &

# RIGHT - Proper process management:
apm spawn django-server python manage.py runserver
```

## 🎯 Detection Patterns

### Recognize Long-Running Commands By:

1. **Command Keywords**:
   - Contains: `serve`, `server`, `watch`, `dev`, `start`, `run`
   - Starts with: `nodemon`, `pm2`, `webpack-dev-server`
   - Ends with: `--watch`, `--serve`, `:dev`

2. **Port Binding Indicators**:
   - Contains port numbers: `:3000`, `:8080`, `:5000`, `:8000`
   - Has flags: `--port`, `-p`, `--host`, `-h`
   - Uses `localhost`, `127.0.0.1`, `0.0.0.0`

3. **Framework-Specific Patterns**:
   ```javascript
   // Next.js / React
   "next dev", "react-scripts start", "vite", "vite preview"
   
   // Node.js
   "express", "fastify", "koa", "hapi"
   
   // Python
   "django", "flask", "fastapi", "tornado", "aiohttp"
   
   // Ruby
   "sinatra", "puma", "unicorn", "thin"
   ```

4. **Output Patterns** (if you see these, you're already blocked!):
   - "Server running at http://..."
   - "Listening on port..."
   - "webpack compiled successfully"
   - "Starting development server..."
   - "[nodemon] watching"

## 🔄 Command Transformation Rules

### Basic Transformations

```bash
# Web Servers
python -m http.server 8080          → apm spawn http-server python -m http.server 8080
php -S localhost:8000               → apm spawn php-server php -S localhost:8000
ruby -run -e httpd . -p 9000        → apm spawn ruby-server ruby -run -e httpd . -p 9000

# Node.js Projects  
npm run dev                         → apm spawn dev-server npm run dev
npm start                           → apm spawn app npm start
node server.js                      → apm spawn node-server node server.js
yarn dev                            → apm spawn dev yarn dev

# Python Projects
python app.py                       → apm spawn app python app.py
flask run                           → apm spawn flask-app flask run
uvicorn main:app --reload           → apm spawn api uvicorn main:app --reload
streamlit run dashboard.py          → apm spawn dashboard streamlit run dashboard.py

# Development Tools
webpack-dev-server                  → apm spawn webpack webpack-dev-server
tsc --watch                         → apm spawn typescript-watch tsc --watch
nodemon server.js                   → apm spawn nodemon nodemon server.js
```

### Naming Conventions

Use descriptive names based on the service type:
- `dev-server` for development servers
- `api` for API servers  
- `web` for web frontends
- `worker` for background workers
- `db` for database processes
- `<framework>-server` for specific frameworks

## 🛠️ Using APM with MCP Tools

If you have MCP access to APM, prefer using MCP tools:

```typescript
// Instead of Bash command:
// ❌ await bash("npm run dev")

// ✅ Use MCP tool:
await mcp__agent_process_manager__spawn({
  name: "dev-server",
  command: "npm",
  args: ["run", "dev"]
})

// Check if it's running
const processes = await mcp__agent_process_manager__list({ current_dir: true })

// View logs if needed
const logs = await mcp__agent_process_manager__logs({ 
  process_id: processes[0].id,
  limit: 50
})
```

## 📋 Complete Workflow Example

When a user asks you to "start the development server":

1. **Identify the command** (e.g., found `npm run dev` in package.json)
2. **Use APM to spawn it**:
   ```bash
   apm spawn dev-server npm run dev
   ```
3. **Confirm it's running**:
   ```bash
   apm list
   ```
4. **Check for startup errors**:
   ```bash
   apm logs dev-server | tail -20
   ```
5. **Report success to user**:
   "Development server started successfully on port 3000. You can view logs with `apm logs dev-server`"

## 🚑 Recovery Procedures

### If You Get Blocked:

1. The user might see you're unresponsive
2. They may need to interrupt/restart the session
3. When you resume, check for APM:
   ```bash
   apm status
   ```
4. List any running processes:
   ```bash
   apm list --all
   ```

### If APM Is Not Available:

1. **Check if APM is installed**:
   ```bash
   which apm || echo "APM not found"
   ```

2. **If not installed, ask the user**:
   "I notice you're trying to run a development server. Would you like me to help you install APM (Agent Process Manager) for better background process management? It prevents me from getting blocked on long-running commands."

3. **Quick install**:
   ```bash
   # If user agrees
   cargo install agent-process-manager
   # or
   git clone https://github.com/sunnya97/agent-process-manager && cd agent-process-manager && cargo build --release && sudo cp target/release/apm /usr/local/bin/
   ```

## 🎓 Best Practices

1. **Always spawn before other operations**:
   ```bash
   # First spawn the server
   apm spawn backend python app.py
   # Then continue with other tasks
   ```

2. **Use descriptive process names**:
   ```bash
   # Good - clear what's running
   apm spawn nextjs-dev npm run dev
   apm spawn django-api python manage.py runserver
   
   # Bad - unclear
   apm spawn thing1 npm run dev
   apm spawn p2 python app.py
   ```

3. **Check logs after spawning**:
   ```bash
   apm spawn api npm start
   sleep 2  # Give it time to start
   apm logs api | tail -20  # Check for errors
   ```

4. **Clean up when done**:
   ```bash
   # Stop specific process
   apm stop dev-server
   
   # Or stop all from current directory
   apm stop-all --current-dir
   ```

## 🎯 Quick Reference Card

| If you would normally run... | Use this instead... |
|------------------------------|---------------------|
| `npm run dev` | `apm spawn dev npm run dev` |
| `python app.py` | `apm spawn app python app.py` |
| `yarn start` | `apm spawn web yarn start` |
| `rails server` | `apm spawn rails rails server` |
| `php artisan serve` | `apm spawn laravel php artisan serve` |
| `cargo watch -x run` | `apm spawn rust-dev cargo watch -x run` |
| `jupyter notebook` | `apm spawn jupyter jupyter notebook` |

## 🔍 Advanced Detection Rules

### Multi-Stage Detection

1. **Pre-execution check** - Before running ANY command:
   ```python
   # Pseudo-code for detection
   if (command.contains(["run", "start", "serve", "watch"]) or
       args.contains(["dev", "development", "--watch", "--reload"]) or  
       likely_port_binding(command, args)):
       use_apm_spawn()
   ```

2. **Script inspection** - If running a script:
   ```bash
   # Check if script starts a server
   grep -E "(listen|serve|watch|app\.run|server\.start)" script.py
   ```

3. **Package.json inspection** - For Node projects:
   ```json
   // If scripts contain these, use APM:
   {
     "scripts": {
       "dev": "next dev",          // ← Long-running
       "build": "next build",      // ← Short-lived, OK to run directly  
       "start": "next start"       // ← Long-running
     }
   }
   ```

## 📚 Framework-Specific Guidelines

### React/Next.js/Vue/Angular
- `dev`, `start` scripts → Always use APM
- `build`, `test` scripts → Usually safe to run directly
- `storybook` → Always use APM

### Django/Flask/FastAPI  
- `runserver`, `run` → Always use APM
- `migrate`, `makemigrations` → Safe to run directly
- `shell` → Safe to run directly

### Rails
- `server`, `s` → Always use APM  
- `console`, `c` → Safe to run directly
- `db:migrate` → Safe to run directly

### PHP/Laravel
- `serve` → Always use APM
- `artisan` commands → Check if they're long-running
- `composer` → Usually safe to run directly

## 🎪 Integration with Claude Code

To ensure Claude Code always has access to APM:

1. **One-time setup** (users should run this):
   ```bash
   claude mcp add agent-process-manager apm mcp-bridge -e RUST_LOG=warn
   ```

2. **In your CLAUDE.md file**, add:
   ```markdown
   ## Process Management
   
   This project uses APM for process management. Always use:
   - `apm spawn dev npm run dev` instead of `npm run dev`
   - `apm spawn api python api.py` instead of `python api.py`
   - Check running processes with `apm list`
   ```

3. **Project-specific rules**:
   ```markdown
   ## Long-running commands in this project:
   - `make server` → `apm spawn backend make server`
   - `./dev.sh` → `apm spawn dev ./dev.sh`
   - Custom commands that run indefinitely
   ```

Remember: **When in doubt, use APM!** It's better to spawn a short-lived process unnecessarily than to get blocked on a long-running one.