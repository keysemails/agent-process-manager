//! tmux integration for process management

use crate::ApmError;
use std::process::Command;
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use once_cell::sync::Lazy;

// Find the actual tmux executable path at startup
static TMUX_PATH: Lazy<String> = Lazy::new(|| {
    // First try common locations to bypass shell aliases
    for possible_path in &["/opt/homebrew/bin/tmux", "/usr/local/bin/tmux", "/usr/bin/tmux"] {
        if std::path::Path::new(possible_path).exists() {
            info!("Found tmux at: {}", possible_path);
            return possible_path.to_string();
        }
    }
    
    // Fall back to command lookup
    let output = Command::new("sh")
        .args(&["-c", "command which tmux"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "tmux".to_string());
    
    info!("Using tmux from path lookup: {}", output);
    output
});

/// Represents a tmux session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmuxSession {
    pub name: String,
    pub created: String,
    pub attached: bool,
    pub windows: usize,
}

/// tmux integration manager
pub struct TmuxManager;

impl TmuxManager {
    /// Check if tmux is available on the system
    pub fn is_available() -> bool {
        Command::new("sh")
            .args(&["-c", "command -v tmux >/dev/null 2>&1"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Create a new tmux session with the given command
    pub fn create_session(
        session_name: &str,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &[(String, String)],
        log_path: Option<&str>,
    ) -> Result<(), ApmError> {
        // Build the full command to run in the session
        let full_command = if args.is_empty() {
            format!("{} ; echo 'Process exited with code '$? ; read -p 'Press enter to close session'", command)
        } else {
            format!("{} {} ; echo 'Process exited with code '$? ; read -p 'Press enter to close session'", command, args.join(" "))
        };
        
        info!("Creating tmux session '{}' with command: {}", session_name, full_command);
        
        // Use direct tmux command to avoid shell escaping issues
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        
        // Build the tmux command with proper escaping
        let mut tmux_cmd = format!("command tmux new-session -d -s {}", session_name);
        
        // Set working directory if provided
        if let Some(cwd) = cwd {
            tmux_cmd.push_str(&format!(" -c '{}'", cwd));
        }
        
        // Start with bash shell to run commands
        tmux_cmd.push_str(" bash");
        
        // If log path is provided, add pipe-pane command
        if let Some(log_path) = log_path {
            tmux_cmd.push_str(&format!(" \\; pipe-pane -o 'cat >> {}'", log_path));
        }
        
        // Send environment variables first if any
        for (key, value) in env {
            tmux_cmd.push_str(&format!(" \\; send-keys 'export {}=\"{}\"' Enter", 
                key, value.replace("'", "'\"'\"'")));
        }
        
        // Send the actual command to run
        tmux_cmd.push_str(&format!(" \\; send-keys '{}' Enter", 
            full_command.replace("'", "'\"'\"'")));
        
        cmd.arg(&tmux_cmd);
        
        // Set environment variables
        for (key, value) in env {
            cmd.env(key, value);
        }
        
        let output = cmd.output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to spawn tmux: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("tmux failed: {}", stderr);
            return Err(ApmError::ProcessError(format!("tmux failed: {}", stderr)));
        }
        
        Ok(())
    }

    /// Kill a tmux session
    pub fn kill_session(session_name: &str) -> Result<(), ApmError> {
        let output = Command::new("tmux")
            .args(&["kill-session", "-t", session_name])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to kill tmux session: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If session doesn't exist, that's ok
            if !stderr.contains("can't find session") {
                return Err(ApmError::ProcessError(format!("Failed to kill session: {}", stderr)));
            }
        }
        
        Ok(())
    }

    /// Check if a session exists
    pub fn session_exists(session_name: &str) -> bool {
        Command::new("sh")
            .args(&["-c", &format!("command tmux has-session -t {}", session_name)])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get the PID of the main process in a tmux session
    pub fn get_session_pid(session_name: &str) -> Result<u32, ApmError> {
        let output = Command::new("tmux")
            .args(&[
                "list-panes",
                "-t", &format!("{}:0.0", session_name),
                "-F", "#{pane_pid}"
            ])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to get tmux pane PID: {}", e)))?;
        
        if !output.status.success() {
            return Err(ApmError::ProcessError("Failed to get session PID".to_string()));
        }
        
        let pid_str = String::from_utf8_lossy(&output.stdout);
        let pid = pid_str.trim()
            .parse::<u32>()
            .map_err(|_| ApmError::ProcessError("Invalid PID format".to_string()))?;
        
        Ok(pid)
    }

    /// Send keys to a tmux session
    pub fn send_keys(session_name: &str, keys: &str) -> Result<(), ApmError> {
        let output = Command::new("tmux")
            .args(&["send-keys", "-t", session_name, keys])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to send keys: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApmError::ProcessError(format!("Failed to send keys: {}", stderr)));
        }
        
        Ok(())
    }

    /// Capture pane content
    pub fn capture_pane(session_name: &str, history: bool) -> Result<String, ApmError> {
        let mut cmd = Command::new("tmux");
        cmd.args(&["capture-pane", "-t", session_name, "-p"]);
        
        if history {
            // Capture entire history
            cmd.args(&["-S", "-"]);
        }
        
        let output = cmd.output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to capture pane: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApmError::ProcessError(format!("Failed to capture pane: {}", stderr)));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Start piping pane output to a command
    pub fn pipe_pane_start(session_name: &str, command: &str) -> Result<(), ApmError> {
        info!("Starting pipe-pane for session {} with command: {}", session_name, command);
        info!("Using tmux at: {}", *TMUX_PATH);
        
        // Use the discovered tmux path
        let mut cmd = Command::new(&*TMUX_PATH);
        cmd.args(&["pipe-pane", "-o", "-t", session_name, command]);
        
        info!("Executing pipe-pane command: {:?}", cmd);
        
        let output = cmd.output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to start pipe-pane: {}", e)))?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        info!("pipe-pane stdout: {}", stdout);
        info!("pipe-pane stderr: {}", stderr);
        info!("pipe-pane exit status: {}", output.status);
        
        if !output.status.success() {
            error!("pipe-pane failed for session {}: {}", session_name, stderr);
            return Err(ApmError::ProcessError(format!("Failed to start pipe-pane: {}", stderr)));
        }
        
        info!("pipe-pane started successfully for session {}", session_name);
        Ok(())
    }

    /// Stop piping pane output
    pub fn pipe_pane_stop(session_name: &str) -> Result<(), ApmError> {
        let output = Command::new("tmux")
            .args(&["pipe-pane", "-t", session_name])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to stop pipe-pane: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApmError::ProcessError(format!("Failed to stop pipe-pane: {}", stderr)));
        }
        
        Ok(())
    }

    /// Get session info
    pub fn get_session_info(session_name: &str) -> Result<TmuxSession, ApmError> {
        let output = Command::new("tmux")
            .args(&[
                "list-sessions",
                "-F",
                "#{session_name}:#{session_created}:#{session_attached}:#{session_windows}"
            ])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to list sessions: {}", e)))?;
        
        if !output.status.success() {
            return Err(ApmError::ProcessError("Failed to list sessions".to_string()));
        }
        
        let sessions = String::from_utf8_lossy(&output.stdout);
        for line in sessions.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 && parts[0] == session_name {
                return Ok(TmuxSession {
                    name: parts[0].to_string(),
                    created: parts[1].to_string(),
                    attached: parts[2] == "1",
                    windows: parts[3].parse().unwrap_or(0),
                });
            }
        }
        
        Err(ApmError::NotFound(format!("Session {} not found", session_name)))
    }

    /// List all sessions  
    pub fn list_sessions_detailed() -> Result<Vec<TmuxSession>, ApmError> {
        let output = Command::new("tmux")
            .args(&[
                "list-sessions",
                "-F",
                "#{session_name}:#{session_created}:#{session_attached}:#{session_windows}"
            ])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to list sessions: {}", e)))?;
        
        if !output.status.success() {
            // No sessions is not an error
            return Ok(Vec::new());
        }
        
        let sessions_str = String::from_utf8_lossy(&output.stdout);
        let mut sessions = Vec::new();
        
        for line in sessions_str.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 4 {
                sessions.push(TmuxSession {
                    name: parts[0].to_string(),
                    created: parts[1].to_string(),
                    attached: parts[2] == "1",
                    windows: parts[3].parse().unwrap_or(0),
                });
            }
        }
        
        Ok(sessions)
    }
    
    /// Set user options for a tmux session (for metadata storage)
    pub fn set_session_metadata(session_name: &str, key: &str, value: &str) -> Result<(), ApmError> {
        let output = Command::new("tmux")
            .args(&["set-option", "-t", session_name, &format!("@{}", key), value])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to set tmux option: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ApmError::ProcessError(format!("Failed to set session option: {}", stderr)));
        }
        
        Ok(())
    }
    
    /// Get user options from a tmux session
    pub fn get_session_metadata(session_name: &str, key: &str) -> Result<String, ApmError> {
        let output = Command::new("tmux")
            .args(&["show-option", "-vt", session_name, &format!("@{}", key)])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to get tmux option: {}", e)))?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(ApmError::ProcessError("Option not found".to_string()))
        }
    }
    
    /// List all tmux sessions (just names)
    pub fn list_sessions() -> Result<Vec<String>, ApmError> {
        let output = Command::new("tmux")
            .args(&["list-sessions", "-F", "#{session_name}"])
            .output()
            .map_err(|e| ApmError::ProcessError(format!("Failed to list tmux sessions: {}", e)))?;
        
        if output.status.success() {
            let sessions = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            Ok(sessions)
        } else {
            // If no sessions exist, tmux returns non-zero but that's ok
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no server running") || stderr.contains("no sessions") {
                Ok(Vec::new())
            } else {
                Err(ApmError::ProcessError(format!("Failed to list sessions: {}", stderr)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_available() {
        // This test might fail in CI without tmux
        let available = TmuxManager::is_available();
        println!("tmux available: {}", available);
    }
}