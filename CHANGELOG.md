# Changelog

All notable changes to Agent Process Manager (APM) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.4] - 2025-07-30

### Fixed
- Linux binaries now built in Debian 12 (bookworm) container for GLIBC 2.36 compatibility
- Resolves runtime errors when using APM in Node:20 or Debian 12 based containers

### Changed
- Release workflow uses rust:1-bookworm container for Linux builds
- Updated documentation to explain GLIBC compatibility requirements

## [0.3.3] - 2025-07-19

### Fixed
- Docker image publishing in release workflow now runs in main release job
- Fixed repository access issues for Docker build

## [0.3.2] - 2025-07-19

### Added
- Docker image publishing to ghcr.io for public binary distribution
- Dockerfile.release for minimal APM binary image
- Support for using APM in other containers via `COPY --from=ghcr.io/sunnya97/apm:TAG`

### Changed
- Release workflow now builds and pushes Docker images alongside binaries
- Updated release documentation to include Docker installation method

## [0.3.1] - 2025-07-19

### Added
- GitHub Actions release workflow improvements
- Release documentation and tooling (RELEASING.md, CHANGELOG.md, install.sh)
- Update script for Vibe Docker integration

### Fixed
- Tombstoned status for killed processes
- Process exit detection with tmux pane status
- Test suite compatibility with tombstoned status changes
- GitHub Actions workflow to use non-deprecated artifact actions

## [0.3.0] - 2025-07-19

### Added
- GitHub Actions release workflow for automated binary releases
- Support for Linux x64, macOS x64, and macOS ARM64 platforms
- Binary distribution through GitHub releases
- RELEASING.md documentation for release process
- Installation script for easy setup

### Breaking Changes
- Renamed all "stop" commands to "kill" to accurately reflect forceful termination
- Added new `ProcessStatus::Killed` state to distinguish forced termination from natural exits
- MCP tool renamed: `stop` → `kill`, `stop_multiple` → `kill_multiple`

### New Features
- **Instant Process Exit Detection**: File marker approach for <100ms exit detection
- **SIGCHLD-based Monitoring**: Real-time process exit detection for PTY processes
- **ProcessExitMonitor**: Centralized exit event handling
- **Exit Code Capture**: Process exit codes now captured and stored
- **Status Distinctions**: Clear differentiation between Stopped/Killed/Failed states

### Technical Improvements
- Added `src/process/exit_monitor.rs` for centralized exit event handling
- Modified tmux wrapper commands to create exit marker files
- Updated monitoring loop to check file markers before expensive tmux operations
- Reduced polling overhead by prioritizing file-based detection

## [0.2.0-alpha.1] - Previous Release

### Added
- Initial alpha release with core functionality
- Process management with tmux integration
- MCP server support
- AI-optimized API endpoints
- Full-text search capabilities
- Tag-based process organization

## [0.1.0] - Initial Development

### Added
- Basic process spawning and management
- Log storage and retrieval
- REST API with Axum framework
- CLI interface
- WebSocket support for real-time logs

[0.3.3]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.3.3
[0.3.2]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.3.2
[0.3.1]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.3.1
[0.3.0]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.3.0
[0.2.0-alpha.1]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.2.0-alpha.1
[0.1.0]: https://github.com/sunnya97/agent-process-manager/releases/tag/v0.1.0