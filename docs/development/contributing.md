# Contributing to Agent Process Manager

Thank you for your interest in contributing to Agent Process Manager! This document provides guidelines for contributing to the project.

## 🚀 Getting Started

### Prerequisites
- Rust 1.70+ 
- Git
- GitHub CLI (`gh`) - recommended for issue management
- tmux (for terminal integration features)

### Development Setup
```bash
# Clone the repository
git clone https://github.com/sunnya97/agent-process-manager.git
cd agent-process-manager

# Build the project
cargo build

# Run tests
cargo test

# Start development daemon
RUST_LOG=agent_process_manager=debug cargo run -- start
```

## 📋 Issue Management

We use GitHub Issues as our single source of truth for project management. All tasks, bugs, and feature requests are tracked through GitHub Issues with a comprehensive labeling system.

### Issue Types
- **🐛 Bug Report**: Something isn't working correctly
- **✨ Feature Request**: New functionality or enhancements  
- **📋 Development Task**: Maintenance, refactoring, or general improvements

### Priority Levels
- ![#d73a4a](https://via.placeholder.com/15/d73a4a/000000?text=+) **Critical**: Urgent issues affecting core functionality
- ![#ff9500](https://via.placeholder.com/15/ff9500/000000?text=+) **High**: Important features for next release
- ![#fbca04](https://via.placeholder.com/15/fbca04/000000?text=+) **Medium**: Nice-to-have improvements
- ![#0075ca](https://via.placeholder.com/15/0075ca/000000?text=+) **Low**: Future enhancements

### Component Labels
- `component: api` - REST API and handlers
- `component: cli` - Command line interface
- `component: logs` - Log storage and pattern detection
- `component: process` - Process management and supervision
- `component: tmux` - Terminal integration
- `component: websocket` - Real-time streaming
- `component: ai-agent` - AI agent API features
- `component: config` - Configuration management

### Finding Issues to Work On
- Browse [open issues](https://github.com/sunnya97/agent-process-manager/issues)
- Filter by [`good-first-issue`](https://github.com/sunnya97/agent-process-manager/labels/good-first-issue) for newcomer-friendly tasks
- Check [`status: ready`](https://github.com/sunnya97/agent-process-manager/labels/status%3A%20ready) for issues ready for implementation

## 🔄 Development Workflow

### 1. Pick an Issue
- Comment on the issue to let others know you're working on it
- Ask questions if anything is unclear
- Check for dependencies or blockers

### 2. Create a Branch
```bash
git checkout -b feature/issue-number-short-description
# Example: git checkout -b feature/123-add-config-support
```

### 3. Implement Changes
- Follow existing code patterns and conventions
- Add tests for new functionality
- Update documentation as needed
- Ensure all tests pass: `cargo test`

### 4. Submit a Pull Request
- Reference the issue number in your PR description
- Provide a clear description of changes
- Include screenshots/examples for UI changes
- Ensure CI passes

### 5. Code Review
- Address review feedback promptly
- Keep discussions focused and constructive
- Update PR based on feedback

## 🧪 Testing Guidelines

We maintain comprehensive test coverage across multiple categories:

### Test Categories
```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test agent_api_test           # AI Agent API
cargo test --test log_summarizer_test      # Log summarization
cargo test --test log_patterns_test        # Pattern detection
cargo test --test api_integration_test     # API endpoints

# End-to-end tests  
cargo test --test cli_e2e_test            # CLI commands

# Run all tests
cargo test
```

### Writing Tests
- Add unit tests for new functions/modules
- Include integration tests for API endpoints
- Test error conditions and edge cases
- Update existing tests when modifying functionality

## 📝 Code Standards

### Rust Guidelines
- Follow standard Rust conventions (`cargo fmt`)
- Use `cargo clippy` for linting
- Add documentation for public APIs
- Handle errors appropriately (avoid `unwrap()` in production code)

### Code Organization
- Keep modules focused and cohesive
- Use descriptive variable and function names
- Add inline comments for complex logic
- Follow existing project structure

### Commit Messages
- Use conventional commit format
- Reference issue numbers: `feat: add config validation (#123)`
- Keep first line under 50 characters
- Provide details in commit body if needed

## 🐛 Reporting Bugs

Use our [Bug Report template](https://github.com/sunnya97/agent-process-manager/issues/new?template=bug_report.yml) which includes:

- Clear description of the issue
- Steps to reproduce
- Expected vs actual behavior
- Environment information
- Relevant logs or error messages

## ✨ Requesting Features

Use our [Feature Request template](https://github.com/sunnya97/agent-process-manager/issues/new?template=feature_request.yml) which includes:

- Problem statement and use case
- Proposed solution
- Alternative approaches considered
- Usage examples
- Implementation suggestions

## 📚 Documentation

### What to Document
- New features and APIs
- Breaking changes
- Configuration options
- Deployment guides
- Troubleshooting tips

### Documentation Locations
- `README.md` - Project overview and quick start
- `CLAUDE.md` - Detailed development guidance
- Inline code documentation
- GitHub Issues for feature specifications

## 🏗️ Architecture Decisions

For significant architectural changes:

1. Open a GitHub Discussion for initial proposal
2. Create an issue with `architecture` label
3. Provide detailed technical design
4. Consider backward compatibility
5. Plan migration strategy if needed

## 🤝 Community Guidelines

### Code of Conduct
- Be respectful and inclusive
- Focus on constructive feedback
- Help newcomers learn and contribute
- Maintain professional communication

### Getting Help
- Check existing documentation first
- Search closed issues for solutions
- Ask questions in GitHub Discussions
- Tag maintainers for urgent issues

## 📊 Project Management

### Issue Lifecycle
1. **Needs Research** → Investigation required
2. **Ready** → Available for implementation  
3. **In Progress** → Currently being worked on
4. **Needs Review** → Pull request submitted
5. **Completed** → Merged and deployed

### Release Process
- Features are merged to `main` branch
- Releases are tagged with semantic versioning
- Release notes highlight new features and breaking changes
- Critical fixes may trigger patch releases

## 🛠️ Development Tools

### Recommended VS Code Extensions
- `rust-analyzer` - Rust language support
- `CodeLLDB` - Debugging support
- `Even Better TOML` - Configuration file support

### GitHub CLI Usage
```bash
# View repository issues
gh issue list

# Create new issue
gh issue create --template feature_request.yml

# View issue details
gh issue view 123

# Assign issue to yourself
gh issue edit 123 --add-assignee @me
```

## 📞 Contact

- **Issues**: [GitHub Issues](https://github.com/sunnya97/agent-process-manager/issues)
- **Discussions**: [GitHub Discussions](https://github.com/sunnya97/agent-process-manager/discussions)
- **Security**: Report via GitHub Security Advisories

---

Thank you for contributing to Agent Process Manager! 🚀