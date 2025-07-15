# CLAUDE.md Template for APM Integration

Copy this template to your project's `CLAUDE.md` file and customize it for your specific needs.

---

# CLAUDE.md - [Your Project Name]

## Process Management with APM

This project uses Agent Process Manager (APM) for all background processes. **NEVER run development servers or long-running processes directly**. Always use APM to prevent getting blocked.

### Required Setup

Ensure APM is installed and running:
```bash
# Check if APM is available
apm status

# If not running, start it
apm start
```

### Project-Specific Commands

Replace these commands with APM equivalents:

```bash
# Frontend Development Server
# DON'T: npm run dev
# DO: apm spawn frontend npm run dev

# Backend API Server  
# DON'T: python api/server.py
# DO: apm spawn backend python api/server.py

# Database (if managed locally)
# DON'T: postgres -D ./data
# DO: apm spawn postgres postgres -D ./data

# [Add your project's specific commands here]
# DON'T: [original command]
# DO: apm spawn [process-name] [original command]
```

### Standard Process Names

Use these consistent names for common processes:
- `frontend` - Frontend development server
- `backend` - Backend API server
- `db` or `postgres/mysql/mongo` - Database server
- `redis` - Redis server
- `worker` - Background job processor
- `build-watch` - Build system in watch mode

### Common Tasks

#### Starting Development Environment
```bash
# Start all services
apm spawn frontend npm run dev
apm spawn backend python manage.py runserver
apm spawn redis redis-server

# Verify all are running
apm list
```

#### Checking Logs
```bash
# Check for errors in backend
apm logs backend | grep -i error | tail -20

# Monitor frontend build
apm logs frontend -f
```

#### Stopping Services
```bash
# Stop everything in this project
apm stop-all --current-dir

# Stop specific service
apm stop backend
```

### Port Assignments

Document your project's port usage:
- Frontend: 3000
- Backend API: 8000  
- Database: 5432
- Redis: 6379
- [Add your ports]

### Environment-Specific Commands

#### Development
```bash
apm spawn dev-server npm run dev
```

#### Testing
```bash
# Unit tests (short-lived, OK to run directly)
npm test

# Integration tests with services
apm spawn test-db postgres -D ./test-data
npm run test:integration
apm stop test-db
```

#### Production Build
```bash
# Build commands are usually short-lived, OK to run directly
npm run build
```

### Troubleshooting

#### Process won't start
```bash
# Check if port is already in use
apm list | grep ":3000"

# Check detailed logs
apm logs [process-name] | head -50
```

#### Can't find process
```bash
# List all processes (not just current directory)
apm list --all

# Process might have different name or crashed
apm list | grep -i [partial-name]
```

### Project-Specific Long-Running Commands

List any custom scripts or commands that should use APM:

1. `./scripts/watch-assets.sh` → `apm spawn asset-watch ./scripts/watch-assets.sh`
2. `make serve` → `apm spawn server make serve`
3. [Add your custom commands]

### Notes

- Always check `apm list` after starting services
- Use descriptive process names
- Check logs if a process exits unexpectedly
- Run `apm clean` periodically to remove old stopped processes

---

Remember: When in doubt, use APM! It's better to spawn a short process unnecessarily than to get blocked on a long-running one.