# Agent Process Manager - Development Tasks

This document outlines planned features and improvements for the Agent Process Manager project, organized by priority.

## High Priority

### 1. Complete Terminal Attachment Feature
**Status**: ✅ Completed - Implemented via tmux integration  
**Complexity**: High  
**Location**: `src/tmux.rs`, `src/main.rs`

- [x] Implemented tmux-based terminal attachment
- [x] Native terminal experience via `tmux attach-session`
- [x] Process persistence across APM restarts
- [x] CLI command `apm attach <name>` with read-only mode support
- [x] All terminal features work out-of-box (resize, colors, etc.)

**Technical Notes**:
- Replaced custom WebSocket terminal with tmux integration
- Uses tmux control mode for process management
- Automatic session cleanup on process termination

### 2. Fix Pattern Detection - Timestamp Components Mislabeled as Ports
**Status**: Bug identified  
**Complexity**: Low  
**Location**: `src/logs/patterns.rs`

- [ ] Fix regex pattern that incorrectly identifies timestamp components as ports
- [ ] Ensure port detection only matches actual port numbers in appropriate contexts
- [ ] Add tests to prevent regression

**Technical Notes**:
- Current issue: timestamps like `[11:41:22]` have `41` and `22` detected as ports
- Need to refine port regex to require proper context (e.g., "port 8080", ":8080")

### 3. Enhanced AI Agent API
**Status**: Basic implementation exists  
**Complexity**: High  
**Location**: `src/api/handlers.rs`

- [ ] Implement proper NLP query parsing (consider using a simple rule engine)
- [ ] Add contextual log analysis beyond keyword matching
- [ ] Create structured response formats optimized for LLM consumption
- [ ] Implement query history and context management
- [ ] Add configurable log summarization levels
- [ ] Support for custom query templates
- [ ] Add process correlation (e.g., "show me errors from all web servers")

**Technical Notes**:
- Consider integrating with local LLMs for query understanding
- Design a query DSL for precise searches
- Cache query results for performance

## Medium Priority

### 3. Configuration File Support
**Status**: TODO in codebase  
**Complexity**: Medium  
**Location**: `src/config.rs`, `src/main.rs`

- [ ] Implement YAML/TOML configuration parser
- [ ] Support configuration hierarchy (default -> system -> user -> CLI args)
- [ ] Per-process configuration profiles
- [ ] Environment variable expansion in configs
- [ ] Configuration validation and schema
- [ ] Hot-reload configuration changes
- [ ] Export current configuration

**Example Config Structure**:
```yaml
daemon:
  host: 0.0.0.0
  port: 7337
  log_level: info

defaults:
  restart_policy:
    enabled: true
    max_retries: 3
  resource_limits:
    max_memory_mb: 1024

processes:
  - name: web-server
    command: python -m http.server
    restart_policy:
      max_retries: 5
```

### 4. Advanced Restart Policies
**Status**: Basic implementation exists  
**Complexity**: Medium  
**Location**: `src/process/supervisor.rs`

- [ ] Exponential backoff with jitter
- [ ] Restart based on exit codes (e.g., restart on 1, stop on 0)
- [ ] Signal-based restart conditions
- [ ] Health check-based restarts
- [ ] Time-based restart windows (e.g., only restart during business hours)
- [ ] Crash loop detection and backoff
- [ ] Restart rate limiting
- [ ] Dependency-aware restarts

**Technical Notes**:
- Consider implementing a state machine for restart logic
- Add metrics for restart attempts and success rates

### 5. Log Management Features
**Status**: Basic SQLite storage exists  
**Complexity**: Medium  
**Location**: `src/logs/storage.rs`

- [ ] Log rotation by size and time
- [ ] Compression for archived logs (gzip/zstd)
- [ ] Configurable retention policies
- [ ] Export to external systems (S3, syslog, Elasticsearch)
- [ ] Full-text search indexing
- [ ] Log shipping/forwarding
- [ ] Structured logging support (JSON logs)
- [ ] Log sampling for high-volume processes

**Technical Notes**:
- Consider using `tantivy` for full-text search
- Implement background workers for log rotation/compression

### 6. Tmux Integration and Terminal Multiplexer Support
**Status**: Not implemented  
**Complexity**: Medium  
**Location**: `src/cli/`, new integration module

- [ ] Create `apm tmux` subcommand for seamless tmux integration
- [ ] Auto-generate tmux configuration for APM sessions
- [ ] Support for `tmux new-window -n "process-name" "apm attach process-name"`
- [ ] Helper scripts for common tmux+APM workflows
- [ ] Named pipe support for streaming logs to tmux panes
- [ ] Tmux status line integration showing APM process health
- [ ] Session management commands (list all APM processes in tmux windows)
- [ ] Automatic window naming based on process names

**Example workflows**:
```bash
# Quick attach all APM processes to tmux windows
apm tmux attach-all

# Create tmux session with all running processes
apm tmux session mysession

# Stream logs to current tmux pane
apm logs myapp --follow | tmux loadb -
```

**Technical Notes**:
- Use tmux control mode for programmatic interaction
- Support both attached and detached session creation
- Handle tmux not being installed gracefully
- Consider integration with other terminal multiplexers (screen, zellij)

## Low Priority

### 7. Process Groups and Dependencies
**Status**: Not implemented  
**Complexity**: High  
**Location**: New module needed

- [ ] Define process groups with metadata
- [ ] Startup/shutdown ordering
- [ ] Dependency DAG resolution
- [ ] Group-level operations (start/stop/restart all)
- [ ] Health checks for group readiness
- [ ] Resource limits per group
- [ ] Group-level log aggregation

**Example API**:
```rust
ProcessGroup {
    name: "web-stack",
    processes: ["nginx", "app-server", "redis"],
    dependencies: {
        "app-server": ["redis"],
        "nginx": ["app-server"]
    }
}
```

### 8. Security Enhancements
**Status**: No authentication currently  
**Complexity**: Medium  
**Location**: `src/api/`, new auth module

- [ ] API key authentication
- [ ] JWT token support
- [ ] Role-based access control (view/control/admin)
- [ ] Process isolation (user/group IDs)
- [ ] Resource limit enforcement (cgroups on Linux)
- [ ] Audit logging for all operations
- [ ] TLS support for API and WebSocket
- [ ] Secure configuration storage

### 9. Monitoring and Metrics
**Status**: Basic health metrics exist  
**Complexity**: Medium  
**Location**: `src/process/health.rs`

- [ ] Prometheus metrics endpoint
- [ ] Custom metric collection from processes
- [ ] Historical metrics storage
- [ ] Alerting rules engine
- [ ] Performance profiling
- [ ] Distributed tracing support
- [ ] Dashboard UI (separate project?)

### 10. Plugin System
**Status**: Not implemented  
**Complexity**: High  
**Location**: New module needed

- [ ] Plugin API for custom patterns
- [ ] Custom log processors
- [ ] External storage backends
- [ ] Custom health checks
- [ ] Event hooks (pre/post start/stop)
- [ ] WASM plugin support for sandboxing

## Implementation Order Recommendation

1. **Fix Pattern Detection** - Quick bug fix, improves AI accuracy
2. **Configuration File Support** - Enables many other features
3. **Enhanced AI Agent API** - Core value proposition
4. **Advanced Restart Policies** - Improves reliability
5. **Log Management** - Necessary for production use

## Contributing

When implementing any of these features:
1. Add comprehensive tests
2. Update documentation
3. Follow existing code patterns
4. Consider backwards compatibility
5. Add feature flags for experimental features