# MCP Usage Examples for APM

## Basic Process Management

### Starting a Simple Web Server
```typescript
// Using MCP tools in an AI agent
const result = await mcp.callTool('apm', 'spawn', {
  name: 'web-server',
  command: 'python',
  args: ['-m', 'http.server', '8080']
});
```

### Starting a Node.js Application
```typescript
const result = await mcp.callTool('apm', 'spawn', {
  name: 'node-app',
  command: 'node',
  args: ['server.js'],
  env: {
    NODE_ENV: 'development',
    PORT: '3000'
  }
});
```

## Querying Process Information

### List All Processes
```typescript
const processes = await mcp.callTool('apm', 'list', {});
console.log(processes);
// Returns array of process info with status, CPU, memory usage
```

### Get Process Logs
```typescript
const logs = await mcp.callTool('apm', 'logs', {
  process_id: 'uuid-here',
  limit: 50
});
```

## Advanced Queries

### System Overview
```typescript
const overview = await mcp.callTool('apm', 'query', {
  type: 'system_overview'
});
// Returns total processes, running/stopped counts, and details
```

### Find Process Errors
```typescript
const errors = await mcp.callTool('apm', 'query', {
  type: 'process_errors',
  time_window: '10m'
});
// Returns all errors from last 10 minutes
```

### Port Mapping Discovery
```typescript
const ports = await mcp.callTool('apm', 'query', {
  type: 'port_mapping',
  include_urls: true
});
// Returns which processes are using which ports
```

## Resource Access

### Read All Processes Resource
```typescript
const resource = await mcp.readResource('apm://processes');
// Returns JSON with all process information
```

### Read Specific Process Logs
```typescript
const logs = await mcp.readResource('apm://logs/process-uuid');
// Returns plain text logs for the process
```

## Using Prompts

### Analyze System Errors
```typescript
const prompt = await mcp.getPrompt('apm', 'analyze-errors', {});
// Returns a prompt with all recent errors for AI analysis
```

### Generate Health Report
```typescript
const prompt = await mcp.getPrompt('apm', 'system-health', {});
// Returns a prompt with system metrics for AI insights
```

## Complete Workflow Examples

### Development Environment Setup
```javascript
// 1. Start backend API
await mcp.callTool('apm', 'spawn', {
  name: 'backend-api',
  command: 'python',
  args: ['manage.py', 'runserver'],
  env: { DJANGO_SETTINGS_MODULE: 'myapp.settings.dev' }
});

// 2. Start frontend dev server
await mcp.callTool('apm', 'spawn', {
  name: 'frontend',
  command: 'npm',
  args: ['run', 'dev'],
  cwd: '/path/to/frontend'
});

// 3. Start database
await mcp.callTool('apm', 'spawn', {
  name: 'postgres',
  command: 'postgres',
  args: ['-D', '/usr/local/var/postgres']
});

// 4. Verify all running
const processes = await mcp.callTool('apm', 'list', {});
console.log(`Started ${processes.length} processes`);
```

### Debugging Production Issue
```javascript
// 1. Get system overview
const overview = await mcp.callTool('apm', 'query', {
  type: 'system_overview'
});

// 2. Find recent errors
const errors = await mcp.callTool('apm', 'query', {
  type: 'process_errors',
  time_window: '1h'
});

// 3. Check performance metrics
const metrics = await mcp.callTool('apm', 'query', {
  type: 'performance_metrics',
  metrics: ['cpu', 'memory']
});

// 4. Analyze patterns
const prompt = await mcp.getPrompt('apm', 'analyze-errors', {});
// Send to AI for analysis

// 5. Stop problematic process
if (needsRestart) {
  await mcp.callTool('apm', 'stop', {
    process_id: problematicProcessId
  });
}
```

### Monitoring Long-Running Tasks
```javascript
// Start a long-running data processing job
const job = await mcp.callTool('apm', 'spawn', {
  name: 'data-processor',
  command: 'python',
  args: ['process_data.py', '--input', 'large_dataset.csv']
});

// Monitor progress by searching logs
const progress = await mcp.callTool('apm', 'query', {
  type: 'log_search',
  pattern: 'Processing.*complete|Progress:',
  process_id: job.process_id,
  limit: 10
});

// Check resource usage
const metrics = await mcp.callTool('apm', 'query', {
  type: 'performance_metrics',
  process_id: job.process_id
});

// Get completion status
const logs = await mcp.callTool('apm', 'logs', {
  process_id: job.process_id,
  limit: 20
});
```

## Error Handling

```javascript
try {
  const result = await mcp.callTool('apm', 'spawn', {
    name: 'my-service',
    command: 'node',
    args: ['service.js']
  });
} catch (error) {
  if (error.message.includes('already exists')) {
    // Process with same name already running
    const processes = await mcp.callTool('apm', 'list', {});
    const existing = processes.find(p => p.name === 'my-service');
    console.log('Process already running:', existing);
  }
}
```

## Best Practices

1. **Always name your processes** for easy identification
2. **Use tags** to group related processes
3. **Set working directory** when needed
4. **Monitor logs** after spawning for startup errors
5. **Check system overview** before spawning resource-intensive processes
6. **Use structured queries** instead of parsing raw logs