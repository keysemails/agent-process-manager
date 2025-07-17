//! Real-time process exit detection using SIGCHLD and other event sources

use super::ProcessId;
use crate::ApmError;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::signal::unix::{signal, SignalKind};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use tracing::{debug, error, info, warn};

type Result<T> = std::result::Result<T, ApmError>;

#[derive(Debug, Clone)]
pub enum ProcessExitEvent {
    /// Process exited normally with exit code
    ProcessExited { 
        process_id: ProcessId, 
        pid: u32, 
        exit_code: i32 
    },
    /// Process was killed by signal
    ProcessKilled { 
        process_id: ProcessId, 
        pid: u32, 
        signal: i32 
    },
    /// Tmux session ended
    TmuxSessionEnded { 
        process_id: ProcessId, 
        session_name: String 
    },
    /// PTY process ended
    PtyProcessEnded { 
        process_id: ProcessId, 
        pid: u32 
    },
}

#[derive(Debug)]
struct ProcessInfo {
    process_id: ProcessId,
    pid: u32,
    monitor_type: ProcessMonitorType,
}

#[derive(Debug, Clone)]
enum ProcessMonitorType {
    Tmux { session_name: String },
    Pty,
    Direct,
}

pub struct ProcessExitMonitor {
    /// Maps PID to ProcessInfo for SIGCHLD handling
    pid_map: Arc<RwLock<HashMap<u32, ProcessInfo>>>,
    /// Event sender for notifying about process exits
    event_sender: mpsc::Sender<ProcessExitEvent>,
    /// Event receiver for consumers
    event_receiver: Arc<Mutex<mpsc::Receiver<ProcessExitEvent>>>,
}

impl ProcessExitMonitor {
    pub fn new() -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::channel(1000);
        
        Ok(Self {
            pid_map: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
        })
    }

    /// Get the event receiver for consuming process exit events
    pub fn get_event_receiver(&self) -> Arc<Mutex<mpsc::Receiver<ProcessExitEvent>>> {
        self.event_receiver.clone()
    }

    /// Register a process for monitoring
    async fn register_process(
        &self,
        process_id: ProcessId,
        pid: u32,
        monitor_type: ProcessMonitorType,
    ) {
        let process_info = ProcessInfo {
            process_id: process_id.clone(),
            pid,
            monitor_type,
        };

        self.pid_map.write().await.insert(pid, process_info);
        debug!("Registered process {} (PID {}) for exit monitoring", process_id, pid);
    }

    /// Register a tmux process
    pub async fn register_tmux_process(
        &self,
        process_id: ProcessId,
        pid: u32,
        session_name: String,
    ) {
        self.register_process(
            process_id,
            pid,
            ProcessMonitorType::Tmux { session_name },
        ).await;
    }

    /// Register a PTY process
    pub async fn register_pty_process(&self, process_id: ProcessId, pid: u32) {
        self.register_process(process_id, pid, ProcessMonitorType::Pty).await;
    }

    /// Unregister a process from monitoring
    pub async fn unregister_process(&self, pid: u32) -> bool {
        let removed = self.pid_map.write().await.remove(&pid);
        if let Some(ref info) = removed {
            debug!("Unregistered process {} (PID {}) from exit monitoring", info.process_id, pid);
            true
        } else {
            false
        }
    }

    /// Start the exit monitoring system
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("🔧 EXIT MONITOR: Starting process exit monitoring");

        // Initialize SIGCHLD handler
        let mut sigchld = signal(SignalKind::child())
            .map_err(|e| ApmError::Process(format!("Failed to create SIGCHLD handler: {}", e)))?;

        // Start SIGCHLD monitoring task
        let pid_map = self.pid_map.clone();
        let event_sender = self.event_sender.clone();
        tokio::spawn(async move {
            loop {
                // Wait for SIGCHLD signal
                if sigchld.recv().await.is_some() {
                    Self::handle_sigchld(&pid_map, &event_sender).await;
                } else {
                    warn!("SIGCHLD signal stream ended");
                    break;
                }
            }
        });

        Ok(())
    }

    /// Handle SIGCHLD signal by reaping child processes
    async fn handle_sigchld(
        pid_map: &Arc<RwLock<HashMap<u32, ProcessInfo>>>,
        event_sender: &mpsc::Sender<ProcessExitEvent>,
    ) {
        info!("🔧 EXIT MONITOR: Received SIGCHLD, checking for exited processes");

        // Check all registered processes for exit status
        let pids: Vec<u32> = pid_map.read().await.keys().cloned().collect();
        
        for pid in pids {
            match waitpid(Some(Pid::from_raw(pid as i32)), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, exit_code)) => {
                    if let Some(process_info) = pid_map.write().await.remove(&pid) {
                        info!("Process {} (PID {}) exited with code {}", 
                              process_info.process_id, pid, exit_code);
                        
                        let event = ProcessExitEvent::ProcessExited {
                            process_id: process_info.process_id,
                            pid,
                            exit_code,
                        };

                        if let Err(e) = event_sender.send(event).await {
                            error!("Failed to send process exit event: {}", e);
                        }
                    }
                }
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    if let Some(process_info) = pid_map.write().await.remove(&pid) {
                        info!("Process {} (PID {}) killed by signal {}", 
                              process_info.process_id, pid, signal as i32);
                        
                        let event = ProcessExitEvent::ProcessKilled {
                            process_id: process_info.process_id,
                            pid,
                            signal: signal as i32,
                        };

                        if let Err(e) = event_sender.send(event).await {
                            error!("Failed to send process kill event: {}", e);
                        }
                    }
                }
                Ok(WaitStatus::StillAlive) => {
                    // Process is still running, continue monitoring
                }
                Ok(status) => {
                    debug!("Process {} has unexpected wait status: {:?}", pid, status);
                }
                Err(nix::errno::Errno::ECHILD) => {
                    // No such child process - it may have already been reaped
                    debug!("No child process found for PID {}", pid);
                }
                Err(e) => {
                    error!("Error waiting for process {}: {}", pid, e);
                }
            }
        }
    }

    /// Send a tmux session ended event
    pub async fn notify_tmux_session_ended(
        &self,
        process_id: ProcessId,
        session_name: String,
    ) -> Result<()> {
        let event = ProcessExitEvent::TmuxSessionEnded {
            process_id,
            session_name,
        };

        self.event_sender.send(event).await
            .map_err(|e| ApmError::Process(format!("Failed to send tmux session ended event: {}", e)))
    }

    /// Send a PTY process ended event
    pub async fn notify_pty_process_ended(
        &self,
        process_id: ProcessId,
        pid: u32,
    ) -> Result<()> {
        let event = ProcessExitEvent::PtyProcessEnded {
            process_id,
            pid,
        };

        self.event_sender.send(event).await
            .map_err(|e| ApmError::Process(format!("Failed to send PTY process ended event: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_process_exit_monitor_creation() {
        let monitor = ProcessExitMonitor::new().expect("Failed to create monitor");
        
        // Test that we can register a process
        let process_id = ProcessId::new();
        monitor.register_pty_process(process_id.clone(), 12345).await;
        
        // Verify it's registered
        let pid_map = monitor.pid_map.read().await;
        assert!(pid_map.contains_key(&12345));
        assert_eq!(pid_map.get(&12345).unwrap().process_id, process_id);
    }

    #[tokio::test]
    async fn test_process_unregistration() {
        let monitor = ProcessExitMonitor::new().expect("Failed to create monitor");
        
        let process_id = ProcessId::new();
        monitor.register_pty_process(process_id.clone(), 12345).await;
        
        // Unregister the process
        let removed = monitor.unregister_process(12345).await;
        assert!(removed);
        
        // Verify it's no longer registered
        let pid_map = monitor.pid_map.read().await;
        assert!(!pid_map.contains_key(&12345));
    }
}