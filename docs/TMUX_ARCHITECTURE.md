# tmux-Based Architecture

As of v0.2.0, Agent Process Manager (APM) uses tmux as its backend for process management and terminal attachment. This provides a more robust and feature-rich solution compared to implementing our own terminal emulator.

## Why tmux?

1. **Battle-tested**: tmux has been around for years and handles all edge cases in terminal emulation
2. **No character dropping**: Eliminates the issues we faced with custom WebSocket implementations
3. **Persistence**: Processes continue running even if APM daemon crashes
4. **Native attachment**: Users can attach directly using tmux if needed
5. **Rich features**: Split panes, windows, scrollback, copy mode, etc.

## Architecture Overview

```
┌─────────────┐     ┌─────────────┐     ┌──────────────┐
│   APM CLI   │────▶│ APM Daemon  │────▶│ tmux Session │
└─────────────┘     └─────────────┘     └──────────────┘
                           │                     │
                           ▼                     ▼
                    ┌─────────────┐       ┌──────────┐
                    │  Log Store  │◀──────│ pipe-pane│
                    └─────────────┘       └──────────┘
```

## Process Lifecycle

### 1. Process Spawning

When a process is spawned with APM:

```bash
apm spawn my-app python app.py
```

APM creates a tmux session named `apm-<process-id>` and runs the command inside it:

```bash
tmux new-session -d -s apm-<uuid> python app.py
```

### 2. Log Collection

APM uses tmux's `pipe-pane` command to capture all output:

```bash
tmux pipe-pane -t apm-<uuid> 'cat >> /tmp/apm-<uuid>.log'
```

A background task monitors this file and stores logs in the SQLite database with pattern detection.

### 3. Terminal Attachment

When attaching to a process:

```bash
apm attach my-app
```

APM executes:

```bash
tmux attach-session -t apm-<uuid>
```

This gives users a native tmux experience with all standard tmux keybindings.

### 4. Process Monitoring

APM monitors tmux sessions to detect when processes exit:

- Checks session existence every 2 seconds
- Updates process status when session ends
- Cleans up orphaned sessions

## Configuration

### Enabling tmux Mode

By default, all processes use tmux. You can disable it per-process:

```json
{
  "name": "my-app",
  "command": "python",
  "args": ["app.py"],
  "use_tmux": false,  // Falls back to direct PTY
  "pty": true
}
```

### tmux Requirements

- tmux 2.0 or later must be installed
- APM checks for tmux availability on startup
- Falls back to PTY mode if tmux is not available

## API Changes

### Process Info

Process information now includes tmux session details:

```json
{
  "id": "uuid",
  "name": "my-app",
  "status": "running",
  "tmux_session": "apm-uuid",
  "pid": 12345
}
```

### WebSocket Attachment

The WebSocket attachment endpoint (`/api/attach/:id`) now:

1. Detects if process uses tmux
2. For tmux processes, provides a relay that:
   - Captures pane content periodically
   - Forwards input using `tmux send-keys`
   - Handles basic terminal operations

Note: Direct tmux attachment via CLI is recommended for better performance.

## Advantages

1. **Reliability**: No more character dropping or terminal emulation bugs
2. **Performance**: Native C implementation is faster than our Rust PTY handling
3. **Features**: Users get tmux's scrollback, copy mode, etc. for free
4. **Debugging**: Can attach to processes using standard tmux commands
5. **Persistence**: Processes survive APM restarts

## Limitations

1. **Read-only mode**: Not fully supported with tmux attachment (tmux limitation)
2. **WebSocket performance**: Slightly higher latency due to capture-pane polling
3. **Platform support**: Requires tmux installation (not available on Windows without WSL)

## Migration Guide

For existing APM users:

1. Install tmux: `brew install tmux` (macOS) or `apt install tmux` (Linux)
2. Update APM to v0.2.0 or later
3. Existing processes will continue using PTY mode
4. New processes will use tmux by default

## Advanced Usage

### Direct tmux Access

You can access APM-managed processes directly with tmux:

```bash
# List APM sessions
tmux list-sessions | grep apm-

# Attach directly
tmux attach -t apm-<uuid>

# Send commands
tmux send-keys -t apm-<uuid> "echo hello" Enter
```

### Custom tmux Configuration

APM respects your `~/.tmux.conf` settings. Useful configurations:

```bash
# Better colors
set -g default-terminal "screen-256color"

# Increase scrollback
set -g history-limit 10000

# Mouse support
set -g mouse on
```

## Future Enhancements

1. **tmux control mode**: Use `-CC` flag for programmatic control
2. **Session sharing**: Multiple users attaching to same session
3. **Window management**: Support multiple windows per process
4. **Recording**: Integration with asciinema or similar tools
5. **Distributed tmux**: Support for remote tmux sessions