# Process Tags in APM

Tags provide a powerful way to organize, categorize, and filter processes in Agent Process Manager. This guide covers everything you need to know about using tags effectively.

## Overview

Tags are simple text labels that you can attach to processes for categorization and filtering. They enable:
- Logical grouping of related processes
- Efficient filtering in list operations
- Batch operations on tagged processes
- Better organization in large deployments

## Tag Rules and Validation

Tags must follow these rules:
- **Length**: 1-50 characters
- **Characters**: Only alphanumeric (a-z, A-Z, 0-9), dash (-), underscore (_), and dot (.)
- **Case-sensitive**: `Web` and `web` are different tags
- **No spaces**: Use dashes or underscores instead

### Valid Tags Examples
- `web`, `api`, `database`
- `production`, `staging`, `dev`
- `frontend-v2`, `backend_service`
- `node-app`, `python3.11`
- `team.alpha`, `project.x`

### Invalid Tags Examples
- `my tag` (contains space)
- `app@prod` (invalid character @)
- `service!` (invalid character !)
- `` (empty string)
- `this-is-a-very-long-tag-name-that-exceeds-fifty-chars` (too long)

## Adding Tags When Spawning Processes

### CLI
```bash
# Single tag
apm spawn web-server node app.js --tag production

# Multiple tags
apm spawn api-server python api.py --tag backend --tag api --tag v2

# Tags with environment
apm spawn worker node worker.js --tag worker --tag queue-processor
```

### API
```bash
curl -X POST http://localhost:7337/api/processes \
  -H "Content-Type: application/json" \
  -d '{
    "name": "web-app",
    "command": "node",
    "args": ["server.js"],
    "tags": ["web", "frontend", "production"]
  }'
```

### MCP
```javascript
await mcp.call('spawn', {
  name: 'data-processor',
  command: 'python',
  args: ['process.py'],
  tags: ['batch', 'data', 'nightly']
});
```

## Managing Tags on Existing Processes

### Adding Tags

#### CLI
```bash
apm tag add my-server production
apm tag add my-server critical
```

#### API
```bash
curl -X POST http://localhost:7337/api/processes/<id>/tags \
  -H "Content-Type: application/json" \
  -d '{"tag": "high-priority"}'
```

#### MCP
```javascript
await mcp.call('tag', {
  action: 'add',
  process_id: 'abc123',
  tag: 'monitored'
});
```

### Removing Tags

#### CLI
```bash
apm tag remove my-server staging
```

#### API
```bash
curl -X DELETE http://localhost:7337/api/processes/<id>/tags/staging
```

#### MCP
```javascript
await mcp.call('tag', {
  action: 'remove',
  process_id: 'abc123',
  tag: 'deprecated'
});
```

### Listing Tags

#### CLI
```bash
# List tags for a specific process
apm tag list my-server

# List all unique tags across all processes
apm tag all
```

#### API
```bash
# Get tags for a specific process
curl http://localhost:7337/api/processes/<id>/tags

# Get all unique tags
curl http://localhost:7337/api/tags
```

#### MCP
```javascript
// List all unique tags
const allTags = await mcp.call('tag', {
  action: 'list'
});
```

## Filtering Processes by Tags

APM supports two filtering modes:
- **OR logic**: Show processes that have ANY of the specified tags
- **AND logic**: Show processes that have ALL of the specified tags

### OR Logic (Any Tags)

#### CLI
```bash
# Using --tag (can specify multiple)
apm list --tag web --tag api

# Using --tags-any with comma-separated list
apm list --tags-any "frontend,backend"
```

#### API
```bash
# URL parameter with comma-separated tags
curl "http://localhost:7337/api/processes?tags=web,api,worker"
```

#### MCP
```javascript
const processes = await mcp.call('list', {
  tags: ['web', 'api', 'worker']
});
```

### AND Logic (All Tags)

#### CLI
```bash
# Processes must have ALL specified tags
apm list --tags-all "web,production"
```

#### API
```bash
# URL parameter for AND logic
curl "http://localhost:7337/api/processes?all_tags=web,production,critical"
```

#### MCP
```javascript
const processes = await mcp.call('list', {
  all_tags: ['web', 'production', 'critical']
});
```

## Use Cases and Best Practices

### Environment-Based Tags
```bash
# Development
apm spawn dev-server npm run dev --tag development --tag local

# Staging
apm spawn staging-api node api.js --tag staging --tag testing

# Production
apm spawn prod-web node server.js --tag production --tag critical
```

### Service Type Tags
```bash
# Different service types
apm spawn web-frontend react-scripts start --tag web --tag frontend --tag react
apm spawn api-backend python api.py --tag api --tag backend --tag python
apm spawn db-proxy pgbouncer --tag database --tag proxy
apm spawn cache-server redis-server --tag cache --tag redis
```

### Team/Project Tags
```bash
# Team ownership
apm spawn team-alpha-service node service.js --tag team.alpha --tag project.x

# Department tags
apm spawn analytics-worker python worker.py --tag dept.data --tag analytics
```

### Version Tags
```bash
# Version tracking
apm spawn app-v2 node app.js --tag v2.0.0 --tag latest
apm spawn app-v1 node app.js --tag v1.9.5 --tag stable
```

## Tag Display in Output

Tags are displayed in various APM outputs:

### List Command
```
┌────────┬─────────────┬─────────┬─────────┬──────────┬─────────────┬──────────────────┐
│ NAME   │ ID          │ STATUS  │ PID     │ CPU %    │ MEMORY      │ TAGS             │
├────────┼─────────────┼─────────┼─────────┼──────────┼─────────────┼──────────────────┤
│ web    │ abc123      │ Running │ 1234    │ 15.2%    │ 256.3 MB    │ web,prod,v2      │
│ api    │ def456      │ Running │ 5678    │ 8.1%     │ 128.1 MB    │ api,prod         │
└────────┴─────────────┴─────────┴─────────┴──────────┴─────────────┴──────────────────┘
```

### JSON Output
```json
{
  "id": "abc123",
  "name": "web-server",
  "status": "Running",
  "tags": ["web", "production", "frontend"],
  "cpu_percent": 15.2,
  "memory_mb": 256.3
}
```

## Advanced Tag Patterns

### Hierarchical Tags
Use dots to create hierarchical structures:
```bash
apm spawn service node app.js \
  --tag env.production \
  --tag region.us-east \
  --tag tier.web
```

### Semantic Versioning
```bash
apm spawn app node app.js \
  --tag version.2.1.0 \
  --tag release.stable
```

### Combined Filtering
```bash
# Find all production web services
apm list --tags-all "production,web"

# Find any service that's either critical OR in production
apm list --tags-any "critical,production"
```

## Tag Limits and Performance

- **Tags per process**: No hard limit, but recommend keeping under 10 for clarity
- **Tag length**: Maximum 50 characters per tag
- **Unique tags**: System tracks all unique tags efficiently
- **Performance**: Tag filtering is optimized and performs well even with thousands of processes

## Troubleshooting

### Common Issues

1. **Tag validation errors**
   ```
   Error: Tag can only contain alphanumeric characters, dash, underscore, and dot
   ```
   Solution: Remove spaces and special characters from tags

2. **Tag not found when filtering**
   - Tags are case-sensitive
   - Check exact spelling with `apm tag all`

3. **Process not showing in filtered list**
   - Verify the process has the expected tags with `apm tag list <process-name>`
   - Check if using correct filter logic (OR vs AND)

### Debugging Tag Filters
```bash
# See all processes with their tags
apm list --all --format json | jq '.[] | {name, tags}'

# Check specific process tags
apm tag list my-process

# Verify tag exists in system
apm tag all | grep -i "mytag"
```

## Integration with Other APM Features

### Access Control
Tags work seamlessly with APM's directory-based access control:
- Tags are visible based on process visibility
- Tag management respects access permissions

### Queries and Search
Use tags in AI agent queries:
```javascript
// Find errors in production services
const errors = await mcp.call('query', {
  type: 'process_errors',
  process_filter: ['production'],  // Filters by process names, not tags
  time_window: '1h'
});
```

### Future Features
Planned enhancements for tags:
- Tag-based restart policies
- Automatic tagging based on patterns
- Tag inheritance for restarted processes
- Bulk operations on tagged processes