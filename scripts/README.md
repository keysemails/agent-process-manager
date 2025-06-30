# GitHub Issues Automation Scripts

This directory contains automation scripts for managing GitHub Issues workflow in the Agent Process Manager project.

## 🚀 Quick Start

### Complete Setup
```bash
# Set up everything from scratch
./scripts/github-setup.sh
```

### Daily Usage
```bash
# Find issues to work on
./scripts/query-issues.sh ready

# Check project status
./scripts/query-issues.sh stats

# Backup labels
./scripts/manage-labels.sh backup
```

## 📜 Script Reference

### `github-setup.sh`
**Complete GitHub Issues setup script**

Performs full setup including:
- Creates comprehensive label system
- Verifies issue templates
- Sets up automation scripts
- Creates GitHub CLI aliases
- Provides verification and next steps

```bash
./scripts/github-setup.sh
```

### `create-labels.sh`
**Label creation script**

Creates the complete label system for issue organization:
- Priority levels (critical, high, medium, low)
- Component labels (api, cli, logs, process, tmux, websocket, ai-agent, config)
- Status indicators (needs-research, blocked, ready, in-progress, needs-review)
- Type labels (bug, feature, enhancement, documentation, refactor, chore)
- Special categories (good-first-issue, breaking-change, security, performance, architecture)

```bash
./scripts/create-labels.sh
```

### `migrate-tasks.sh`
**Task migration script**

Converts tasks from TASKS.md to GitHub Issues with proper labels and formatting.

```bash
./scripts/migrate-tasks.sh
```

### `query-issues.sh`
**Issue query helper**

Provides quick filters for common issue queries:

```bash
# Status filters
./scripts/query-issues.sh ready           # Ready for implementation
./scripts/query-issues.sh in-progress     # Currently being worked on
./scripts/query-issues.sh needs-review    # Needing review
./scripts/query-issues.sh blocked         # Blocked issues

# Priority filters
./scripts/query-issues.sh high-priority   # High priority issues

# Component filters
./scripts/query-issues.sh api             # API-related issues
./scripts/query-issues.sh cli             # CLI-related issues
./scripts/query-issues.sh logs            # Log management issues
./scripts/query-issues.sh process         # Process management issues
./scripts/query-issues.sh tmux            # tmux integration issues
./scripts/query-issues.sh ai-agent        # AI agent issues

# Type filters
./scripts/query-issues.sh bugs            # Bug reports
./scripts/query-issues.sh features        # Feature requests

# Personal
./scripts/query-issues.sh mine            # Your assigned issues
./scripts/query-issues.sh good-first-issue # Newcomer-friendly issues

# Overview
./scripts/query-issues.sh all             # All open issues
./scripts/query-issues.sh stats           # Comprehensive statistics
```

### `manage-labels.sh`
**Label management utilities**

Provides comprehensive label management:

```bash
# View and analyze
./scripts/manage-labels.sh list           # List all labels with descriptions
./scripts/manage-labels.sh stats          # Show label statistics
./scripts/manage-labels.sh missing        # Check for missing standard labels

# Backup and restore
./scripts/manage-labels.sh backup         # Create timestamped backup
./scripts/manage-labels.sh restore file.json # Restore from backup

# Maintenance
./scripts/manage-labels.sh sync           # Create missing standard labels
./scripts/manage-labels.sh clean          # Delete ALL labels (dangerous!)
```

## 🎯 Common Workflows

### For Contributors

1. **Find work to do:**
   ```bash
   ./scripts/query-issues.sh ready
   ./scripts/query-issues.sh good-first-issue
   ```

2. **Check component-specific tasks:**
   ```bash
   ./scripts/query-issues.sh api
   ./scripts/query-issues.sh cli
   ```

3. **Monitor your work:**
   ```bash
   ./scripts/query-issues.sh mine
   ```

### For Maintainers

1. **Project overview:**
   ```bash
   ./scripts/query-issues.sh stats
   ```

2. **Review workflow:**
   ```bash
   ./scripts/query-issues.sh needs-review
   ./scripts/query-issues.sh in-progress
   ```

3. **Label maintenance:**
   ```bash
   ./scripts/manage-labels.sh backup
   ./scripts/manage-labels.sh missing
   ```

### For New Repositories

1. **Complete setup:**
   ```bash
   ./scripts/github-setup.sh
   ```

2. **Verify setup:**
   ```bash
   ./scripts/manage-labels.sh stats
   ./scripts/query-issues.sh stats
   ```

## 🏷️ Label System

### Priority Labels
- ![#d73a4a](https://via.placeholder.com/15/d73a4a/000000?text=+) `priority: critical` - Urgent issues affecting core functionality
- ![#ff9500](https://via.placeholder.com/15/ff9500/000000?text=+) `priority: high` - Important features for next release
- ![#fbca04](https://via.placeholder.com/15/fbca04/000000?text=+) `priority: medium` - Nice-to-have improvements
- ![#0075ca](https://via.placeholder.com/15/0075ca/000000?text=+) `priority: low` - Future enhancements

### Component Labels
- `component: api` - REST API and handlers
- `component: cli` - Command line interface
- `component: logs` - Log storage and pattern detection
- `component: process` - Process management and supervision
- `component: tmux` - Terminal integration
- `component: websocket` - Real-time streaming
- `component: ai-agent` - AI agent API features
- `component: config` - Configuration management

### Status Labels
- `status: needs-research` - Requires investigation
- `status: blocked` - Cannot proceed due to dependencies
- `status: ready` - Ready for implementation
- `status: in-progress` - Currently being worked on
- `status: needs-review` - Awaiting code review

### Type Labels
- `type: bug` - Something isn't working
- `type: feature` - New feature request
- `type: enhancement` - Improvement to existing feature
- `type: documentation` - Documentation updates
- `type: refactor` - Code structure improvements
- `type: chore` - Maintenance tasks

### Special Labels
- `good-first-issue` - Good for newcomers
- `breaking-change` - Will require version bump
- `security` - Security-related improvements
- `performance` - Performance optimization
- `architecture` - Architectural decisions

## 🔧 GitHub CLI Aliases

The setup script creates convenient aliases:

```bash
gh issues-ready    # Show ready issues
gh issues-mine     # Show your assigned issues
gh issues-high     # Show high priority issues
```

## 📊 Monitoring Project Health

### Regular Health Checks
```bash
# Get overview
./scripts/query-issues.sh stats

# Check label system
./scripts/manage-labels.sh missing

# Review work distribution
./scripts/query-issues.sh ready
./scripts/query-issues.sh in-progress
```

### Weekly Maintenance
```bash
# Backup labels
./scripts/manage-labels.sh backup

# Review high priority items
./scripts/query-issues.sh high-priority

# Check for blocked items
./scripts/query-issues.sh blocked
```

## 🛠️ Customization

### Adding New Labels
Edit `create-labels.sh` to add new labels to the standard set.

### Custom Queries
Extend `query-issues.sh` with new filter combinations for your workflow.

### Backup Automation
Consider setting up automated backups:
```bash
# Add to cron for weekly backups
0 0 * * 1 cd /path/to/repo && ./scripts/manage-labels.sh backup
```

## 🔍 Troubleshooting

### GitHub CLI Issues
```bash
# Check authentication
gh auth status

# Re-authenticate if needed
gh auth login

# Verify repository access
gh repo view
```

### Permission Issues
- Ensure you have admin access for label management
- Check if repository is public or private
- Verify GitHub CLI has necessary scopes

### Script Issues
- Ensure scripts are executable: `chmod +x scripts/*.sh`
- Check for required dependencies: `jq`, `gh`
- Run scripts from repository root directory

## 📚 Additional Resources

- [GitHub Issues Documentation](https://docs.github.com/en/issues)
- [GitHub CLI Documentation](https://cli.github.com/manual/)
- [Project Contributing Guide](../CONTRIBUTING.md)
- [GitHub Labels Best Practices](https://github.com/yoshuawuyts/github-standard-labels)