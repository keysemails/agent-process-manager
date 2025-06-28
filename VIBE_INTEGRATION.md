# Integrating Agent Process Manager with Vibe

## Overview

Agent Process Manager (APM) will replace the current tmux-based approach in Vibe machines, providing a more efficient interface for Claude while maintaining human access.

## Integration Steps

### 1. Update Docker Image

Add APM to the vibe-devcontainer:

```dockerfile
# Install APM
RUN curl -L https://github.com/agent-process-manager/apm/releases/latest/download/apm-linux-x64 -o /usr/local/bin/apm \
    && chmod +x /usr/local/bin/apm

# Start APM daemon on machine boot
RUN echo "apm start --daemon &" >> /etc/profile.d/apm.sh
```

### 2. Update CLAUDE.md

Replace tmux instructions with APM:

```markdown
## Process Management

Use APM for all background processes:

### Starting services
```bash
# Instead of: tmux new-window -d -n "dev" "npm run dev"
apm start dev "npm run dev"
```

### Checking status efficiently
```bash
# Instead of: tmux capture-pane -t dev -p | tail -50
apm status dev  # Returns only relevant info
```

### Getting specific information
```bash
# Just errors (no noise)
apm logs dev --errors

# Find all ports/URLs
apm info
```
```

### 3. Update vibe-bridge

Modify vibe-bridge to use APM instead of tmux:

```javascript
// Before
const createTmuxWindow = async (name, command) => {
  await execAsync(`tmux new-window -t ${TMUX_SESSION} -n "${name}" "${command}"`);
};

// After
const createApmProcess = async (name, command) => {
  const response = await fetch('http://localhost:7337/api/processes', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      name,
      command: command.split(' ')[0],
      args: command.split(' ').slice(1),
      pty: true,
      tags: ['vibe', 'user-process']
    })
  });
  return response.json();
};
```

### 4. Benefits for Vibe Users

1. **Better Claude Performance**
   - 90% less context used for process monitoring
   - Claude can check on processes without flooding context
   - Smarter responses based on actual process state

2. **Enhanced Developer Experience**
   - `apm attach` works like tmux but with more features
   - Web dashboard at http://localhost:7337/dashboard
   - Export logs in various formats
   - Search and filter capabilities

3. **Improved Reliability**
   - Automatic process restart policies
   - Resource limit enforcement
   - Better error detection and reporting

## Example Workflow

### Current (tmux) Approach
```bash
# Claude starts a dev server
tmux new-window -d -n "frontend" "npm run dev"

# Later, Claude checks status (uses lots of context)
tmux capture-pane -t frontend -p | tail -100
# Returns 100 lines of raw output
```

### New (APM) Approach
```bash
# Claude starts a dev server
apm start frontend "npm run dev"

# Later, Claude checks status (minimal context)
curl localhost:7337/api/processes/frontend/logs?format=summary
# Returns structured summary with key info only
```

## Migration Path

1. **Phase 1**: Include APM in Docker image, keep tmux as fallback
2. **Phase 2**: Update CLAUDE.md to prefer APM
3. **Phase 3**: Migrate vibe-bridge to use APM APIs
4. **Phase 4**: Remove tmux dependency

## API Examples for Claude

```bash
# Get all running services and their URLs
curl localhost:7337/api/agent/summary

# Check for errors in the last 5 minutes
curl -X POST localhost:7337/api/agent/query \
  -d '{"question": "What errors occurred recently?"}'

# Get specific process info
curl localhost:7337/api/processes/frontend/logs?format=summary
```

This integration will make Claude significantly more effective at managing development processes while preserving the full terminal experience for human users.