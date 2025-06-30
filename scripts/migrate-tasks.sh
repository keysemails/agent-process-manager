#!/bin/bash

# Task Migration Script for Agent Process Manager
# Converts TASKS.md content to GitHub Issues with proper labels

set -e

echo "📋 Migrating tasks from TASKS.md to GitHub Issues..."

# Check if gh CLI is installed and authenticated
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI (gh) is not installed. Please install it first."
    exit 1
fi

if ! gh repo view &> /dev/null; then
    echo "❌ Not in a GitHub repository or not authenticated with gh CLI"
    exit 1
fi

echo "✅ GitHub CLI detected and authenticated"

# Function to create an issue with error handling
create_issue() {
    local title="$1"
    local body="$2"
    local labels="$3"
    
    echo "Creating issue: $title"
    
    # Create a temporary file for the issue body
    local temp_file=$(mktemp)
    echo "$body" > "$temp_file"
    
    if gh issue create --title "$title" --body-file "$temp_file" --label "$labels" 2>/dev/null; then
        echo "  ✅ Created: $title"
    else
        echo "  ❌ Failed to create: $title"
        echo "  Body preview:"
        echo "$body" | head -5
        echo "  ..."
    fi
    
    rm "$temp_file"
    sleep 1  # Rate limiting
}

echo ""
echo "🚀 Creating GitHub Issues from TASKS.md..."

# High Priority Task: Enhanced AI Agent API
create_issue "Complete Enhanced AI Agent API" "## Summary
Implement a comprehensive AI Agent API with structured query parsing and contextual log analysis.

## Current Status
- Basic implementation exists in \`src/api/handlers.rs\`
- Simple keyword matching is implemented
- Need to enhance with proper NLP query parsing and structured responses

## Acceptance Criteria
- [ ] Implement proper NLP query parsing (consider using a simple rule engine)
- [ ] Add contextual log analysis beyond keyword matching
- [ ] Create structured response formats optimized for LLM consumption
- [ ] Implement query history and context management
- [ ] Add configurable log summarization levels
- [ ] Support for custom query templates
- [ ] Add process correlation (e.g., \"show me errors from all web servers\")

## Technical Implementation Notes
- Consider integrating with local LLMs for query understanding
- Design a query DSL for precise searches
- Cache query results for performance
- Files affected: \`src/api/handlers.rs\`

## Definition of Done
- [ ] All acceptance criteria implemented
- [ ] Comprehensive tests added
- [ ] Documentation updated
- [ ] Performance benchmarks meet requirements
- [ ] Code review completed

**Priority**: High
**Complexity**: High
**Component**: AI Agent API" "type: feature,priority: high,component: ai-agent,status: ready"

# Medium Priority Tasks

create_issue "Add Configuration File Support" "## Summary
Implement comprehensive configuration file support with YAML/TOML parsing and hierarchical configuration management.

## Current Status
- TODO marker exists in codebase
- No configuration file support currently implemented

## Acceptance Criteria
- [ ] Implement YAML/TOML configuration parser
- [ ] Support configuration hierarchy (default -> system -> user -> CLI args)
- [ ] Per-process configuration profiles
- [ ] Environment variable expansion in configs
- [ ] Configuration validation and schema
- [ ] Hot-reload configuration changes
- [ ] Export current configuration

## Technical Implementation Notes
- Files affected: \`src/config.rs\`, \`src/main.rs\`
- Use serde for serialization/deserialization
- Implement configuration merging logic

## Example Configuration Structure
\`\`\`yaml
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
\`\`\`

**Priority**: Medium
**Complexity**: Medium
**Component**: Configuration" "type: feature,priority: medium,component: config,status: ready"

create_issue "Implement Advanced Restart Policies" "## Summary
Enhance the basic restart policy implementation with sophisticated restart logic including exponential backoff, health checks, and dependency awareness.

## Current Status
- Basic implementation exists in \`src/process/supervisor.rs\`
- Simple restart counting is implemented
- Need advanced restart strategies

## Acceptance Criteria
- [ ] Exponential backoff with jitter
- [ ] Restart based on exit codes (e.g., restart on 1, stop on 0)
- [ ] Signal-based restart conditions
- [ ] Health check-based restarts
- [ ] Time-based restart windows (e.g., only restart during business hours)
- [ ] Crash loop detection and backoff
- [ ] Restart rate limiting
- [ ] Dependency-aware restarts

## Technical Implementation Notes
- Consider implementing a state machine for restart logic
- Add metrics for restart attempts and success rates
- Files affected: \`src/process/supervisor.rs\`
- Integration with health monitoring system

## Definition of Done
- [ ] All restart policy types implemented
- [ ] State machine design documented
- [ ] Comprehensive tests for all restart scenarios
- [ ] Metrics collection for restart events
- [ ] Configuration schema updated

**Priority**: Medium
**Complexity**: Medium
**Component**: Process Management" "type: enhancement,priority: medium,component: process,status: ready"

create_issue "Add Comprehensive Log Management Features" "## Summary
Enhance the basic SQLite log storage with advanced log management features including rotation, compression, and external system integration.

## Current Status
- Basic SQLite storage exists in \`src/logs/storage.rs\`
- No rotation or archival capabilities
- Limited to local storage only

## Acceptance Criteria
- [ ] Log rotation by size and time
- [ ] Compression for archived logs (gzip/zstd)
- [ ] Configurable retention policies
- [ ] Export to external systems (S3, syslog, Elasticsearch)
- [ ] Full-text search indexing
- [ ] Log shipping/forwarding
- [ ] Structured logging support (JSON logs)
- [ ] Log sampling for high-volume processes

## Technical Implementation Notes
- Consider using \`tantivy\` for full-text search
- Implement background workers for log rotation/compression
- Files affected: \`src/logs/storage.rs\`, new modules for external integrations
- Performance considerations for high-volume logging

## Dependencies
- Configuration system (for retention policies)
- Background task system

**Priority**: Medium
**Complexity**: Medium
**Component**: Log Storage & Patterns" "type: feature,priority: medium,component: logs,status: ready"

create_issue "Enhance tmux Integration with Advanced Features" "## Summary
Expand tmux integration beyond basic terminal attachment to provide comprehensive terminal multiplexer workflows and automation.

## Current Status
- Basic tmux integration exists
- Simple attach functionality implemented
- Missing advanced tmux workflow features

## Acceptance Criteria
- [ ] Create \`apm tmux\` subcommand for seamless tmux integration
- [ ] Auto-generate tmux configuration for APM sessions
- [ ] Support for \`tmux new-window -n \"process-name\" \"apm attach process-name\"\`
- [ ] Helper scripts for common tmux+APM workflows
- [ ] Named pipe support for streaming logs to tmux panes
- [ ] Tmux status line integration showing APM process health
- [ ] Session management commands (list all APM processes in tmux windows)
- [ ] Automatic window naming based on process names

## Example Workflows
\`\`\`bash
# Quick attach all APM processes to tmux windows
apm tmux attach-all

# Create tmux session with all running processes
apm tmux session mysession

# Stream logs to current tmux pane
apm logs myapp --follow | tmux loadb -
\`\`\`

## Technical Implementation Notes
- Use tmux control mode for programmatic interaction
- Support both attached and detached session creation
- Handle tmux not being installed gracefully
- Consider integration with other terminal multiplexers (screen, zellij)
- Files affected: \`src/cli/\`, new integration module

**Priority**: Medium
**Complexity**: Medium
**Component**: tmux Integration" "type: feature,priority: medium,component: tmux,status: ready"

# Low Priority Tasks

create_issue "Implement Process Groups and Dependencies" "## Summary
Add support for process groups with dependency management, startup ordering, and group-level operations.

## Current Status
- Not implemented
- New module needed
- Complex feature requiring architectural planning

## Acceptance Criteria
- [ ] Define process groups with metadata
- [ ] Startup/shutdown ordering
- [ ] Dependency DAG resolution
- [ ] Group-level operations (start/stop/restart all)
- [ ] Health checks for group readiness
- [ ] Resource limits per group
- [ ] Group-level log aggregation

## Technical Implementation Notes
- Requires new module design
- Dependency resolution algorithm needed
- Integration with existing process management

## Example API Structure
\`\`\`rust
ProcessGroup {
    name: \"web-stack\",
    processes: [\"nginx\", \"app-server\", \"redis\"],
    dependencies: {
        \"app-server\": [\"redis\"],
        \"nginx\": [\"app-server\"]
    }
}
\`\`\`

**Priority**: Low
**Complexity**: High
**Component**: Process Management" "type: feature,priority: low,component: process,architecture,status: needs-research"

create_issue "Add Security Enhancements" "## Summary
Implement comprehensive security features including authentication, authorization, process isolation, and audit logging.

## Current Status
- No authentication currently implemented
- No access control mechanisms
- Security considerations not addressed

## Acceptance Criteria
- [ ] API key authentication
- [ ] JWT token support
- [ ] Role-based access control (view/control/admin)
- [ ] Process isolation (user/group IDs)
- [ ] Resource limit enforcement (cgroups on Linux)
- [ ] Audit logging for all operations
- [ ] TLS support for API and WebSocket
- [ ] Secure configuration storage

## Technical Implementation Notes
- Files affected: \`src/api/\`, new auth module
- Consider integration with existing authentication systems
- Platform-specific isolation mechanisms
- Compliance considerations

**Priority**: Low
**Complexity**: Medium
**Component**: API, Security" "type: feature,priority: low,security,component: api,status: needs-research"

create_issue "Add Monitoring and Metrics Collection" "## Summary
Enhance the basic health metrics with comprehensive monitoring, historical metrics storage, and alerting capabilities.

## Current Status
- Basic health metrics exist in \`src/process/health.rs\`
- No historical storage or alerting
- Limited metrics collection

## Acceptance Criteria
- [ ] Prometheus metrics endpoint
- [ ] Custom metric collection from processes
- [ ] Historical metrics storage
- [ ] Alerting rules engine
- [ ] Performance profiling
- [ ] Distributed tracing support
- [ ] Dashboard UI (separate project?)

## Technical Implementation Notes
- Integration with monitoring ecosystem
- Performance impact of metrics collection
- Storage considerations for historical data
- Files affected: \`src/process/health.rs\`, new monitoring modules

**Priority**: Low
**Complexity**: Medium
**Component**: Process Management, Monitoring" "type: feature,priority: low,component: process,performance,status: needs-research"

create_issue "Create Plugin System Architecture" "## Summary
Design and implement a comprehensive plugin system for extensibility including custom patterns, log processors, and external integrations.

## Current Status
- Not implemented
- Requires significant architectural design
- High complexity feature

## Acceptance Criteria
- [ ] Plugin API for custom patterns
- [ ] Custom log processors
- [ ] External storage backends
- [ ] Custom health checks
- [ ] Event hooks (pre/post start/stop)
- [ ] WASM plugin support for sandboxing

## Technical Implementation Notes
- New module needed
- Security considerations for plugin sandboxing
- Plugin discovery and loading mechanisms
- API design for plugin developers
- Documentation for plugin development

**Priority**: Low
**Complexity**: High
**Component**: Architecture, New Component" "type: feature,priority: low,architecture,status: needs-research"

# Technical Task: WebSocket Enhancement
create_issue "Implement tmux pane resize for WebSocket terminal" "## Summary
Complete the TODO item in websocket.rs to implement tmux pane resize functionality for WebSocket terminal connections.

## Current Status
- TODO comment exists in \`src/api/websocket.rs:447\`
- Resize message handling is stubbed out
- Feature not implemented

## Technical Details
**File**: \`src/api/websocket.rs\`
**Line**: 447
**Current Code**:
\`\`\`rust
Some(\"resize\") => {
    // TODO: Implement tmux pane resize if needed
}
\`\`\`

## Acceptance Criteria
- [ ] Implement tmux pane resize functionality
- [ ] Parse resize messages from WebSocket clients
- [ ] Send appropriate tmux commands for pane resizing
- [ ] Handle resize errors gracefully
- [ ] Add tests for resize functionality
- [ ] Update WebSocket protocol documentation

## Technical Implementation Notes
- Use tmux control mode commands for resizing
- Validate resize parameters (width, height)
- Consider rate limiting for resize operations
- Error handling for invalid tmux sessions

**Priority**: Low
**Complexity**: Low
**Component**: WebSocket, tmux Integration" "type: enhancement,priority: low,component: websocket,component: tmux,status: ready"

echo ""
echo "🎉 Task migration complete!"
echo ""
echo "📊 Migration Summary:"
echo "   • High Priority: 1 issue"
echo "   • Medium Priority: 4 issues"
echo "   • Low Priority: 5 issues"
echo "   • Total Issues Created: 10"
echo ""
echo "📝 Next steps:"
echo "   1. Review created issues in your GitHub repository"
echo "   2. Assign issues to team members or yourself"
echo "   3. Update project documentation"
echo "   4. Remove TASKS.md file after verification"
echo ""
echo "💡 View all issues: gh issue list"