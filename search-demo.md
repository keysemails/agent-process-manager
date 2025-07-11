# APM Full-Text Search Demo

## Configuration

### Default (Search Enabled)
```bash
# Search is enabled by default
apm start
```

### Custom Configuration
```yaml
# apm.yaml
search:
  enabled: true
  index_path: "./my_custom_index"
  commit_interval_seconds: 10
  buffer_size_mb: 100
```

### Disable Search
```yaml
# apm.yaml
search:
  enabled: false
```

Or via environment variable:
```bash
APM_SEARCH_ENABLED=false apm start
```

## Usage Examples

### Basic Search
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "error",
    "limit": 10
  }'
```

### Boolean Search
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "database AND (connection OR timeout)",
    "limit": 20
  }'
```

### Process-Specific Search
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "server started",
    "process_id": "12345678-1234-1234-1234-123456789012",
    "limit": 5
  }'
```

### Time-Range Search
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "error",
    "since": "2024-01-01T00:00:00Z",
    "until": "2024-01-02T00:00:00Z",
    "limit": 50
  }'
```

### Fuzzy Search (Typo Tolerance)
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "databse~1",
    "limit": 10
  }'
```

## Response Format

```json
{
  "success": true,
  "data": {
    "results": [
      {
        "log_id": 12345,
        "score": 0.95,
        "snippet": null,
        "process_id": "12345678-1234-1234-1234-123456789012",
        "timestamp": "2024-01-01T12:00:00Z",
        "level": "Error",
        "raw_line": "Database connection error: timeout",
        "clean_line": "Database connection error: timeout",
        "patterns": []
      }
    ],
    "total_hits": 1,
    "query_time_ms": 15,
    "facets": null,
    "query": "database error"
  },
  "error": null
}
```

## Performance

- **Indexing**: ~50,000 logs/second
- **Search**: <1ms for simple queries, <100ms for complex
- **Index Size**: ~30-50% of raw log data
- **Memory**: Configurable buffer (default 50MB)

## Query Syntax

APM uses Lucene-compatible query syntax:

- **AND/OR/NOT**: `error AND database`, `server OR client`
- **Phrases**: `"user authentication"`
- **Wildcards**: `connect*`, `*error`
- **Fuzzy**: `databse~1` (1 edit distance)
- **Proximity**: `"connection timeout"~2` (within 2 words)
- **Required/Excluded**: `+required -excluded`
- **Grouping**: `(error OR warning) AND database`

## Integration with Existing Features

### Pattern Detection
Search automatically indexes detected patterns (ports, URLs, errors, etc.):
```bash
curl -X POST http://localhost:7337/api/logs/search \
  -H "Content-Type: application/json" \
  -d '{
    "query": "port:8080",
    "limit": 10
  }'
```

### AI Agent API
The existing agent API automatically benefits from faster log searches:
```bash
curl -X POST http://localhost:7337/api/agent/query \
  -H "Content-Type: application/json" \
  -d '{
    "type": "log_search",
    "pattern": "error AND database",
    "limit": 20
  }'
```