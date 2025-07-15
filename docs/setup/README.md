# APM Setup Documentation

This directory contains setup and configuration documentation for APM.

## 📄 Documents

### [ai-assistants.md](ai-assistants.md)
**Critical instructions for AI assistants (Claude, ChatGPT, etc.)**

⚠️ **IMPORTANT**: If you're an AI assistant, read this first! Contains critical rules to prevent getting blocked on long-running commands.

- Commands that will block you (NEVER run directly)
- APM alternatives for each command type
- Framework-specific transformation rules
- Recovery procedures when blocked

### [claude-code.md](claude-code.md)
**Complete Claude Code integration guide**

Step-by-step setup for Claude Code users:
- One-command setup: `claude mcp add agent-process-manager apm mcp-bridge`
- Manual configuration options
- MCP tool configuration
- Troubleshooting guide

### [claude-template.md](claude-template.md)
**Template for project-specific Claude configuration**

Copy this template to your project's `CLAUDE.md` file and customize:
- Project-specific long-running commands
- Custom process names and conventions
- Team-specific workflows
- Port assignments and configurations

## 🚀 Quick Start

1. **For AI Assistants**: Start with [ai-assistants.md](ai-assistants.md)
2. **For Claude Code Users**: Follow [claude-code.md](claude-code.md)
3. **For Project Setup**: Use [claude-template.md](claude-template.md)

## 🔧 Setup Commands

```bash
# Interactive local setup
apm setup-claude

# Automated global setup
apm setup-claude --global -y

# Check configuration status
apm setup-claude --check
```