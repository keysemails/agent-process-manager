# APM Examples

This directory contains example configurations, scripts, and demonstrations for the Agent Process Manager.

## 📁 Directory Structure

### 🔧 [configs/](configs/)
**Configuration examples** for different use cases:
- `basic.yaml` - Basic APM configuration
- `advanced.yaml` - Advanced configuration with all options
- `claude-code.json` - Claude Code MCP configuration
- `claude-mcp-tcp.json` - Claude MCP TCP configuration
- `claude_desktop.json` - Claude Desktop configuration
- `mcp.yaml` - MCP-specific configuration

### 📄 [scripts/](scripts/)
**Example scripts and usage guides**:
- `ai-assistant-examples.md` - Real-world scenarios for AI assistants
- `mcp-usage-examples.md` - MCP protocol usage examples

### 🎬 [demo/](demo/)
**Demonstration scripts**:
- `demo.sh` - Main APM demonstration
- `terminal_attachment_demo.sh` - Terminal attachment features

## 🚀 Quick Start

### Basic Configuration
```bash
# Copy basic config
cp examples/configs/basic.yaml apm.yaml

# Start APM with custom config
apm start --config apm.yaml
```

### Claude Code Setup
```bash
# Use the Claude Code configuration
cp examples/configs/claude-code.json ~/.config/claude/claude_code_config.json

# Or use the one-command setup
claude mcp add agent-process-manager apm mcp-bridge -e RUST_LOG=warn
```

### Run Demos
```bash
# Basic demonstration
./examples/demo/demo.sh

# Terminal attachment demo
./examples/demo/terminal_attachment_demo.sh
```

## 📋 Configuration Options

### Basic Configuration (basic.yaml)
- Default API settings
- Basic MCP configuration
- Standard logging
- Open access control mode

### Advanced Configuration (advanced.yaml)
- All available options
- Advanced MCP settings
- Custom cleanup policies
- Detailed logging configuration

### Claude Code Integration
- MCP server configuration
- Tool registration
- Environment variables
- Connection settings

## 💡 Usage Examples

### Development Workflow
```bash
# 1. Start APM
apm start

# 2. Spawn development processes
apm spawn frontend npm run dev
apm spawn backend python manage.py runserver
apm spawn db postgres -D ./data

# 3. Monitor processes
apm list
apm logs frontend

# 4. Stop when done
apm stop-all --current-dir
```

### AI Assistant Integration
```bash
# Set up for Claude Code
claude mcp add agent-process-manager apm mcp-bridge

# AI can now use APM tools:
# - spawn: Start processes
# - list: List running processes
# - logs: View process logs
# - stop: Stop processes
# - query: Get structured data
```

## 🔧 Customization

### Creating Your Own Config
1. Start with `examples/configs/basic.yaml`
2. Modify settings for your needs
3. Save as `apm.yaml` in your project
4. Test with `apm start --config apm.yaml`

### Project-Specific Setup
1. Copy `examples/configs/` files to your project
2. Add project-specific process definitions
3. Customize port mappings and directories
4. Create project-specific demo scripts

## 📚 Documentation

For more detailed information:
- [Main Documentation](../docs/) - Complete documentation
- [AI Assistant Guide](../docs/setup/ai-assistants.md) - AI integration
- [MCP Integration](../docs/integration/mcp.md) - MCP protocol details