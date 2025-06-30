# Terminal Attachment Feature

The Agent Process Manager (APM) supports real-time terminal attachment to running processes, similar to `tmux attach` or `docker attach`. This allows you to interact with processes that were spawned with PTY support.

## Overview

When a process is spawned with PTY enabled (`pty: true` in config or default behavior), APM maintains a pseudo-terminal that can be accessed later. The terminal attachment feature provides:

- **Bidirectional I/O**: Send input to and receive output from the process
- **Terminal control sequences**: Full support for terminal applications (vim, nano, etc.)
- **Resize handling**: Terminal dimensions are synchronized automatically
- **Read-only mode**: Monitor process output without sending input
- **Detach/reattach**: Leave and return to sessions without affecting the process

## Usage

### Basic Attachment

```bash
# Spawn a process with PTY (enabled by default)
apm spawn my-shell bash

# Attach to the running process
apm attach my-shell

# Detach with Ctrl+Q, D
```

### Read-Only Mode

```bash
# Attach in read-only mode (no input sent to process)
apm attach my-process --read-only
```

## WebSocket Protocol

The terminal attachment uses WebSocket for real-time communication. The protocol uses JSON messages with base64-encoded terminal data:

### Message Types

#### Client to Server

```json
// Input data
{
  "type": "input",
  "data": "base64_encoded_bytes"
}

// Terminal resize
{
  "type": "resize",
  "rows": 24,
  "cols": 80
}
```

#### Server to Client

```json
// Output data
{
  "type": "output",
  "data": "base64_encoded_bytes"
}

// Connection established
{
  "type": "connected",
  "process_id": "uuid"
}

// Error
{
  "type": "error",
  "message": "Error description"
}

// Disconnected
{
  "type": "disconnected"
}
```

## Implementation Details

### Server Side (WebSocket Handler)

The WebSocket handler (`src/api/websocket.rs`):
1. Retrieves the PTY master from ProcessManager
2. Creates separate tasks for reading from PTY and writing to WebSocket
3. Handles input from WebSocket and forwards to PTY
4. Manages terminal resize events

### Client Side (CLI)

The CLI implementation (`src/main.rs`):
1. Uses `crossterm` to put the local terminal in raw mode
2. Captures all keyboard input and terminal events
3. Forwards input to WebSocket (unless in read-only mode)
4. Displays output from the process
5. Handles the detach sequence (Ctrl+Q, D)

### PTY Management

- Each process with PTY enabled stores an `Arc<Mutex<Box<dyn MasterPty>>>`
- The `ProcessManager::get_pty_master()` method provides safe access
- Multiple concurrent attachments are prevented by the Mutex

## Example Use Cases

### Interactive Shell Sessions

```bash
# Start a development environment
apm spawn dev-env bash

# Attach to run commands
apm attach dev-env
$ npm install
$ npm run dev
# Ctrl+Q, D to detach

# Process keeps running, reattach later
apm attach dev-env
```

### Monitoring Long-Running Processes

```bash
# Start a server
apm spawn web-server python -m http.server 8080

# Monitor output in read-only mode
apm attach web-server --read-only
```

### Debugging Applications

```bash
# Run application with debugger
apm spawn debug-session gdb ./myapp

# Attach to interact with debugger
apm attach debug-session
(gdb) break main
(gdb) run
```

## Integration with tmux

APM works well with tmux for terminal multiplexing:

```bash
# In tmux, create a new window attached to APM process
tmux new-window -n "myapp" "apm attach myapp"

# Or split pane
tmux split-window "apm attach myapp --read-only"
```

## Security Considerations

- Terminal attachment requires access to the APM API (currently unauthenticated)
- PTY access provides full control over the process
- Read-only mode prevents input but still shows all output
- Future versions will add authentication and access control

## Limitations

- Currently, only one attachment session is supported at a time per process
- Binary protocols over PTY may have edge cases
- Very high output rates might cause buffering
- No session recording/playback functionality yet

## Future Enhancements

- Multiple concurrent read-only attachments
- Session recording and playback
- Collaborative sessions with multiple users
- Built-in terminal sharing via web interface
- Integration with terminal recording tools (asciinema, etc.)