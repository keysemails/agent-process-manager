//! Process supervisor implementation with PTY support

use super::{ProcessId, ProcessInfo, ProcessStatus};
use super::health::HealthMonitor;
use super::exit_monitor::{ProcessExitMonitor, ProcessExitEvent};
use crate::{ApmError, Result, tmux::TmuxManager, logs::LogStorage};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pty: bool,
    #[serde(default = "default_true")]
    pub use_tmux: bool,
    #[serde(default)]
    pub restart_policy: RestartPolicy,
    #[serde(default)]
    pub resources: ResourceLimits,
    pub access_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    pub enabled: bool,
    pub max_retries: u32,
    pub backoff_ms: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_retries: 3,
            backoff_ms: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceLimits {
    pub max_memory_mb: Option<u64>,
    pub max_cpu_percent: Option<f32>,
}

pub struct Process {
    pub id: ProcessId,
    pub config: ProcessConfig,
    pub status: ProcessStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub restart_count: u32,
    pub session_pid: Option<u32>,
    #[allow(dead_code)]
    pty_master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>,
    #[allow(dead_code)]
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    tmux_session: Option<String>,
}

pub struct ProcessManager {
    storage: Arc<LogStorage>,
    log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>,
    health_monitor: Arc<HealthMonitor>,
    exit_monitor: Arc<ProcessExitMonitor>,
}

impl ProcessManager {
    pub fn new(
        storage: Arc<LogStorage>,
        log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>
    ) -> Result<Self> {
        let health_monitor = Arc::new(HealthMonitor::new());
        let exit_monitor = Arc::new(ProcessExitMonitor::new()?);
        
        // Start the health monitoring background task
        let monitor = health_monitor.clone();
        tokio::spawn(async move {
            monitor.start_monitoring().await;
        });
        
        // Start the exit monitoring
        let exit_mon = exit_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = exit_mon.start_monitoring().await {
                error!("Failed to start exit monitoring: {}", e);
            }
        });

        // Start health event handler (legacy - will be replaced by exit monitor)
        let event_receiver = health_monitor.get_event_receiver();
        let legacy_storage = storage.clone();
        tokio::spawn(async move {
            let mut rx = event_receiver.write().await;
            while let Some(event) = rx.recv().await {
                match event {
                    super::health::HealthEvent::ProcessDied(process_id) => {
                        info!("Health monitor detected process {} has died", process_id);
                        if let Err(e) = legacy_storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await {
                            error!("Failed to update process status for dead process {}: {}", process_id, e);
                        }
                    }
                }
            }
        });

        // Start exit event handler
        let exit_event_receiver = exit_monitor.get_event_receiver();
        let exit_storage = storage.clone();
        tokio::spawn(async move {
            let mut rx = exit_event_receiver.lock().await;
            while let Some(event) = rx.recv().await {
                match event {
                    ProcessExitEvent::ProcessExited { process_id, pid, exit_code } => {
                        info!("Process {} (PID {}) exited with code {}", process_id, pid, exit_code);
                        let final_status = if exit_code == 0 { ProcessStatus::Stopped } else { ProcessStatus::Failed };
                        if let Err(e) = exit_storage.update_process_status(&process_id, final_status, None).await {
                            error!("Failed to update process status for exited process {}: {}", process_id, e);
                        }
                    }
                    ProcessExitEvent::ProcessKilled { process_id, pid, signal } => {
                        info!("Process {} (PID {}) killed by signal {}", process_id, pid, signal);
                        if let Err(e) = exit_storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await {
                            error!("Failed to update process status for killed process {}: {}", process_id, e);
                        }
                    }
                    ProcessExitEvent::TmuxSessionEnded { process_id, session_name } => {
                        info!("Tmux session {} for process {} ended", session_name, process_id);
                        if let Err(e) = exit_storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await {
                            error!("Failed to update process status for tmux session end {}: {}", process_id, e);
                        }
                    }
                    ProcessExitEvent::PtyProcessEnded { process_id, pid } => {
                        info!("PTY process {} (PID {}) ended", process_id, pid);
                        if let Err(e) = exit_storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await {
                            error!("Failed to update process status for PTY process end {}: {}", process_id, e);
                        }
                    }
                }
            }
        });
        
        Ok(Self {
            storage,
            log_sender,
            health_monitor,
            exit_monitor,
        })
    }
    
    pub async fn initialize(&self) -> Result<()> {
        // Recover any orphaned tmux sessions
        let recovered = self.storage.recover_orphaned_tmux_sessions().await?;
        if !recovered.is_empty() {
            info!("Recovered {} orphaned tmux sessions", recovered.len());
            for id in &recovered {
                info!("  - Recovered process {}", id);
            }
        }
        
        // Start periodic status reconciliation
        let storage = self.storage.clone();
        let health_monitor = self.health_monitor.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                
                // Get all running processes
                if let Ok(processes) = storage.list_processes(Some(ProcessStatus::Running)).await {
                    for process in processes {
                        // Skip if no PID
                        if let Some(session_pid) = process.session_pid {
                            // Check health status
                            if let Some(health) = health_monitor.get_health(&process.id).await {
                                if !health.is_alive {
                                    info!("Reconciliation: Process {} (session PID {}) is dead but marked as running", process.id, session_pid);
                                    let _ = storage.update_process_status(&process.id, ProcessStatus::Stopped, None).await;
                                }
                            }
                        }
                        
                        // Check tmux session if applicable
                        if let Some(tmux_session) = &process.tmux_session {
                            use crate::tmux::TmuxManager;
                            if !TmuxManager::session_exists(tmux_session) {
                                info!("Reconciliation: Tmux session {} for process {} no longer exists", tmux_session, process.id);
                                let _ = storage.update_process_status(&process.id, ProcessStatus::Stopped, None).await;
                            }
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    #[allow(dead_code)]
    fn clone_for_restart(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            log_sender: self.log_sender.clone(),
            health_monitor: self.health_monitor.clone(),
            exit_monitor: self.exit_monitor.clone(),
        }
    }

    pub async fn spawn_process(&self, config: ProcessConfig) -> Result<ProcessInfo> {
        let id = ProcessId::new();
        info!("Spawning process '{}' with ID {}", config.name, id);

        let process = if config.use_tmux && TmuxManager::is_available() {
            self.spawn_with_tmux(id.clone(), config.clone()).await?
        } else if config.pty {
            self.spawn_with_pty(id.clone(), config.clone()).await?
        } else {
            self.spawn_without_pty(id.clone(), config.clone()).await?
        };

        // Get initial health metrics if PID is available
        let (cpu_percent, memory_mb) = if let Some(session_pid) = process.session_pid {
            self.health_monitor.update_health(&id, session_pid).await;
            if let Some(health) = self.health_monitor.get_health(&id).await {
                (Some(health.cpu_percent), Some(health.memory_mb))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        let info = ProcessInfo {
            id: id.clone(),
            name: process.config.name.clone(),
            command: process.config.command.clone(),
            args: process.config.args.clone(),
            status: process.status,
            session_pid: process.session_pid,
            process_pid: None, // Will be populated by monitoring
            process_name: None, // Will be populated by monitoring
            started_at: process.started_at,
            uptime_seconds: 0,
            restart_count: process.restart_count,
            tags: process.config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: process.config.access_group.clone(),
            cwd: process.config.cwd.clone(),
            detected_ports: vec![], // Will be populated as logs are processed
        };

        // Store process in database
        let tmux_session = if config.use_tmux && TmuxManager::is_available() {
            Some(format!("apm-{}", id.0))
        } else {
            None
        };
        
        self.storage.store_process(
            &id,
            &config.name,
            &config.command,
            &config.args,
            process.status,
            &config,
            tmux_session.as_deref()
        ).await?;
        
        // Update with PID if available
        if let Some(session_pid) = process.session_pid {
            self.storage.update_process_status(&id, process.status, Some(session_pid)).await?;
        }

        // Extract tmux session before moving process
        let tmux_session = process.tmux_session.clone();
        
        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process).await;
        
        // Start monitoring process health
        if let Some(session_pid) = info.session_pid {
            if let Some(session) = tmux_session {
                // Use tmux-specific health monitoring that tracks actual command
                self.start_tmux_health_monitoring(id, session_pid, session).await;
            } else {
                // Use regular health monitoring for non-tmux processes
                self.start_health_monitoring(id, session_pid).await;
            }
        }

        Ok(info)
    }
    
    async fn start_health_monitoring(&self, process_id: ProcessId, pid: u32) {
        let health_monitor = self.health_monitor.clone();
        
        tokio::spawn(async move {
            loop {
                // Update health metrics every 2 seconds
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                health_monitor.update_health(&process_id, pid).await;
                
                // Check if process is still alive
                if let Some(health) = health_monitor.get_health(&process_id).await {
                    if !health.is_alive {
                        info!("Process {} (PID {}) is no longer alive, stopping monitoring", process_id, pid);
                        // The health monitor will have already sent the ProcessDied event
                        break;
                    }
                }
            }
        });
    }
    
    /// Start health monitoring for tmux processes, tracking both shell and actual command
    async fn start_tmux_health_monitoring(&self, process_id: ProcessId, shell_pid: u32, session_name: String) {
        let health_monitor = self.health_monitor.clone();
        let storage = self.storage.clone();
        
        tokio::spawn(async move {
            let mut system = sysinfo::System::new();
            let mut last_actual_pid: Option<u32> = None;
            
            loop {
                // Update health metrics every 2 seconds
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                
                // Refresh process information
                system.refresh_processes(sysinfo::ProcessesToUpdate::All);
                
                // Try to find the actual command PID
                let actual_info = crate::process::tree::find_actual_command_pid(&system, shell_pid);
                
                // Update health with actual PID if found
                if let Some((actual_pid, actual_name)) = actual_info {
                    // Update storage if actual PID changed
                    if last_actual_pid != Some(actual_pid) {
                        info!("Tracking health for actual command: {} (PID {})", actual_name, actual_pid);
                        last_actual_pid = Some(actual_pid);
                        let _ = storage.update_process_pid(&process_id, actual_pid, Some(&actual_name)).await;
                    }
                    
                    // Update health with both PIDs
                    health_monitor.update_health_with_actual_pid(&process_id, shell_pid, Some(actual_pid)).await;
                } else {
                    // Just monitor the shell
                    health_monitor.update_health(&process_id, shell_pid).await;
                }
                
                // Check if tmux session still exists
                if !TmuxManager::session_exists(&session_name) {
                    info!("Tmux session {} no longer exists, stopping health monitoring", session_name);
                    break;
                }
                
                // Check if process is still alive
                if let Some(health) = health_monitor.get_health(&process_id).await {
                    if !health.is_alive {
                        info!("Process {} is no longer alive, stopping monitoring", process_id);
                        break;
                    }
                }
            }
        });
    }

    async fn spawn_with_tmux(&self, id: ProcessId, config: ProcessConfig) -> Result<Process> {
        info!("🔧 SUPERVISOR: spawn_with_tmux called - command='{}', args={:?}", config.command, config.args);
        let session_name = format!("apm-{}", id.0);
        
        // Prepare environment variables
        let env_vars: Vec<(String, String)> = config.env.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        // Create log file path and pre-create the file
        let log_path = format!("/tmp/apm-{}.log", id.0);
        std::fs::File::create(&log_path)
            .map_err(|e| ApmError::ProcessError(format!("Failed to create log file: {}", e)))?;
        
        // Create tmux session with pipe-pane logging
        TmuxManager::create_session(
            &session_name,
            &config.command,
            &config.args,
            config.cwd.as_ref().and_then(|p| p.to_str()),
            &env_vars,
            Some(&log_path),
        )?;
        
        // Get the PID of the process in tmux
        let pid = TmuxManager::get_session_pid(&session_name)
            .ok();
        
        // Find the actual command PID (not just the shell)
        let (actual_pid, actual_name) = if let Some(shell_pid) = pid {
            // Create a system instance to find child processes
            let mut system = sysinfo::System::new();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All);
            
            // Try to find the actual command a few times as it may take a moment to spawn
            let mut actual_process = None;
            for _ in 0..10 {
                if let Some((cmd_pid, cmd_name)) = crate::process::tree::find_actual_command_pid(&system, shell_pid) {
                    info!("Found actual command process: {} (PID {})", cmd_name, cmd_pid);
                    actual_process = Some((cmd_pid, cmd_name));
                    break;
                }
                // Wait a bit and refresh
                std::thread::sleep(std::time::Duration::from_millis(100));
                system.refresh_processes(sysinfo::ProcessesToUpdate::All);
            }
            
            if actual_process.is_none() {
                info!("No actual command found yet for tmux session {}, will monitor shell PID {}", session_name, shell_pid);
            }
            
            (actual_process.as_ref().map(|(pid, _)| *pid), actual_process.map(|(_, name)| name))
        } else {
            (None, None)
        };
        
        // Register with exit monitor if we have a PID
        if let Some(pid_val) = pid {
            self.exit_monitor.register_tmux_process(
                id.clone(),
                pid_val,
                session_name.clone()
            ).await;
        }
        
        // Store actual PID info in storage if we found it
        if let Some(actual_pid_val) = actual_pid {
            self.storage.update_process_pid(&id, actual_pid_val, actual_name.as_deref()).await
                .unwrap_or_else(|e| error!("Failed to update actual PID: {}", e));
        }
        
        // Start log monitoring
        let log_sender = self.log_sender.clone();
        let id_clone = id.clone();
        
        // Wait for tmux session to be ready
        let mut retries = 10;
        while retries > 0 && !TmuxManager::session_exists(&session_name) {
            std::thread::sleep(std::time::Duration::from_millis(100));
            retries -= 1;
        }
        
        if !TmuxManager::session_exists(&session_name) {
            return Err(ApmError::ProcessError("tmux session failed to start".to_string()));
        }
        
        // Store metadata in tmux session as backup
        let _ = TmuxManager::set_session_metadata(&session_name, "apm_process_name", &config.name);
        let _ = TmuxManager::set_session_metadata(&session_name, "apm_command", &config.command);
        let _ = TmuxManager::set_session_metadata(&session_name, "apm_args", &serde_json::to_string(&config.args).unwrap_or_default());
        let _ = TmuxManager::set_session_metadata(&session_name, "apm_process_id", &id.0.to_string());
        
        // Start monitoring the pipe in a separate task using blocking I/O
        let log_path_clone = log_path.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            use std::fs::File;
            
            info!("Starting log monitor thread for {}", log_path_clone);
            
            // Wait for file to be created
            let mut retries = 40; // 10 seconds total
            loop {
                if std::path::Path::new(&log_path_clone).exists() {
                    break;
                }
                if retries == 0 {
                    error!("Log file {} was not created after 10 seconds", log_path_clone);
                    return;
                }
                retries -= 1;
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            
            // Open file for reading
            let file = match File::open(&log_path_clone) {
                Ok(f) => {
                    info!("Successfully opened log file {}", log_path_clone);
                    f
                },
                Err(e) => {
                    error!("Failed to open log file {}: {}", log_path_clone, e);
                    return;
                }
            };
            
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            
            loop {
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        // EOF - wait and continue
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        continue;
                    }
                    Ok(_) => {
                        // Send the line
                        let line_to_send = line.trim_end().to_string();
                        if !line_to_send.is_empty() {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            if let Err(e) = rt.block_on(async {
                                log_sender.send((id_clone.clone(), line_to_send)).await
                            }) {
                                error!("Failed to send log: {}", e);
                                break;
                            }
                        }
                        line.clear();
                    }
                    Err(e) => {
                        error!("Error reading log file: {}", e);
                        break;
                    }
                }
            }
            
            info!("Log monitor thread for {} ended", log_path_clone);
        });
        
        Ok(Process {
            id,
            config,
            status: ProcessStatus::Running,
            started_at: chrono::Utc::now(),
            restart_count: 0,
            session_pid: pid,
            pty_master: None, // No direct PTY access with tmux
            child: None, // No direct child process
            tmux_session: Some(session_name),
        })
    }

    async fn spawn_with_pty(&self, id: ProcessId, config: ProcessConfig) -> Result<Process> {
        let pty_system = native_pty_system();
        
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ApmError::Process(format!("Failed to open PTY: {}", e)))?;

        let mut cmd = CommandBuilder::new(&config.command);
        cmd.args(&config.args);
        
        if let Some(cwd) = &config.cwd {
            cmd.cwd(cwd);
        }
        
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let child = pty_pair.slave.spawn_command(cmd)
            .map_err(|e| ApmError::Process(format!("Failed to spawn process: {}", e)))?;

        let pid = child.process_id();
        
        // Register with exit monitor
        if let Some(pid_val) = pid {
            self.exit_monitor.register_pty_process(id.clone(), pid_val).await;
        }

        Ok(Process {
            id,
            config,
            status: ProcessStatus::Running,
            started_at: chrono::Utc::now(),
            restart_count: 0,
            session_pid: pid,
            pty_master: Some(Arc::new(Mutex::new(pty_pair.master))),
            child: Some(child),
            tmux_session: None,
        })
    }

    async fn spawn_without_pty(&self, id: ProcessId, config: ProcessConfig) -> Result<Process> {
        // For now, we'll always use PTY for consistency
        // In the future, we can implement non-PTY spawning using tokio::process
        self.spawn_with_pty(id, config).await
    }

    async fn monitor_process_output(
        &self,
        id: ProcessId,
        process: Process,
    ) {
        // Check if this is a tmux session
        if let Some(session_name) = &process.tmux_session {
            // For tmux sessions, monitoring is handled by the pipe-pane setup
            // Start a task to monitor session health
            let session = session_name.clone();
            let process_id = id.clone();
            let storage = self.storage.clone();
            
            tokio::spawn(async move {
                // Use shorter interval for tests
                let check_interval = if cfg!(test) {
                    tokio::time::Duration::from_millis(100)
                } else {
                    tokio::time::Duration::from_secs(5)
                };
                
                let mut process_exit_detected = false;
                
                // Create system instance for process tracking
                let mut system = sysinfo::System::new();
                let mut last_actual_pid: Option<u32> = None;
                
                loop {
                    // First, check for file markers (instant detection)
                    let exit_marker_path = format!("/tmp/apm-{}.exited", session);
                    if std::path::Path::new(&exit_marker_path).exists() && !process_exit_detected {
                        info!("Exit marker detected for process {} in tmux session {}", process_id, session);
                        process_exit_detected = true;
                        
                        // Try to read exit code
                        let exit_code_path = format!("/tmp/apm-{}.exit-code", session);
                        let exit_code = std::fs::read_to_string(&exit_code_path)
                            .ok()
                            .and_then(|s| s.trim().parse::<i32>().ok());
                        
                        if let Some(code) = exit_code {
                            info!("Process {} exited with code {}", process_id, code);
                        }
                        
                        // Update process status to stopped (natural exit)
                        let _ = storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await;
                        
                        // Clean up marker files
                        let _ = std::fs::remove_file(&exit_marker_path);
                        let _ = std::fs::remove_file(&exit_code_path);
                        
                        // Continue monitoring until session is actually closed
                    }
                    
                    // Update actual command PID tracking
                    if let Ok(shell_pid) = TmuxManager::get_session_pid(&session) {
                        system.refresh_processes(sysinfo::ProcessesToUpdate::All);
                        
                        if let Some((actual_pid, actual_name)) = crate::process::tree::find_actual_command_pid(&system, shell_pid) {
                            if last_actual_pid != Some(actual_pid) {
                                info!("Actual command PID updated for {}: {} ({})", process_id, actual_name, actual_pid);
                                last_actual_pid = Some(actual_pid);
                                
                                // Update storage with actual PID
                                let _ = storage.update_process_pid(&process_id, actual_pid, Some(&actual_name)).await;
                            }
                        }
                    }
                    
                    // Sleep before next check
                    tokio::time::sleep(check_interval).await;
                    
                    // Check if session still exists
                    if !TmuxManager::session_exists(&session) {
                        info!("Tmux session {} for process {} has ended", session, process_id);
                        // Update process status - if exit marker existed, it's Stopped, otherwise Killed
                        let final_status = if process_exit_detected {
                            ProcessStatus::Stopped
                        } else {
                            ProcessStatus::Killed
                        };
                        let _ = storage.update_process_status(&process_id, final_status, None).await;
                        break;
                    }
                    
                    // Fallback: check tmux pane content if no marker found
                    if !process_exit_detected {
                        use crate::tmux::TmuxManager;
                        if let Ok(pane_content) = TmuxManager::capture_pane(&session, false) {
                            // Check for common exit patterns
                            if pane_content.contains("Process exited with code") ||
                               pane_content.contains("Press enter to close session") ||
                               pane_content.ends_with("exit\n") {
                                info!("Process {} has exited within tmux session {} (detected via pane content)", process_id, session);
                                process_exit_detected = true;
                                // Update process status to stopped
                                let _ = storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await;
                                // Continue monitoring until session is actually closed
                            }
                        }
                    }
                }
            });
            return;
        }
        
        // TODO: Implement PTY monitoring for non-tmux processes
        // For now, we only support tmux-based processes
    }
                

    pub async fn get_process(&self, id: &ProcessId) -> Result<ProcessInfo> {
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let uptime = if process_record.status == ProcessStatus::Running {
            chrono::Utc::now()
                .signed_duration_since(process_record.started_at)
                .num_seconds() as u64
        } else if let Some(stopped_at) = process_record.stopped_at {
            stopped_at
                .signed_duration_since(process_record.started_at)
                .num_seconds() as u64
        } else {
            0
        };

        // Get current health metrics
        let (cpu_percent, memory_mb) = if let Some(_pid) = process_record.session_pid {
            if let Some(health) = self.health_monitor.get_health(id).await {
                (Some(health.cpu_percent), Some(health.memory_mb))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        // Get detected ports from log summary
        let detected_ports = if let Ok(summary) = self.storage.get_summary(&process_record.id).await {
            summary.detected_ports
        } else {
            vec![]
        };
        
        Ok(ProcessInfo {
            id: process_record.id.clone(),
            name: process_record.config.name.clone(),
            command: process_record.config.command.clone(),
            args: process_record.config.args.clone(),
            status: process_record.status,
            session_pid: process_record.session_pid,
            process_pid: process_record.process_pid,
            process_name: process_record.process_name.clone(),
            started_at: process_record.started_at,
            uptime_seconds: uptime,
            restart_count: process_record.restart_count,
            tags: process_record.config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: process_record.config.access_group.clone(),
            cwd: process_record.config.cwd.clone(),
            detected_ports,
        })
    }

    pub async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let process_records = self.storage.list_processes(None).await?;
        let mut infos = Vec::new();

        for proc in process_records {
            let uptime = if proc.status == ProcessStatus::Running {
                chrono::Utc::now()
                    .signed_duration_since(proc.started_at)
                    .num_seconds() as u64
            } else if let Some(stopped_at) = proc.stopped_at {
                stopped_at
                    .signed_duration_since(proc.started_at)
                    .num_seconds() as u64
            } else {
                0
            };

            // Get current health metrics
            let (cpu_percent, memory_mb) = if let Some(_session_pid) = proc.session_pid {
                if let Some(health) = self.health_monitor.get_health(&proc.id).await {
                    (Some(health.cpu_percent), Some(health.memory_mb))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            
            // Get detected ports from log summary (only for running processes to avoid too many queries)
            let detected_ports = if proc.status == ProcessStatus::Running {
                if let Ok(summary) = self.storage.get_summary(&proc.id).await {
                    summary.detected_ports
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            
            infos.push(ProcessInfo {
                id: proc.id.clone(),
                name: proc.config.name.clone(),
                command: proc.config.command.clone(),
                args: proc.config.args.clone(),
                status: proc.status,
                session_pid: proc.session_pid,
                process_pid: proc.process_pid,
                process_name: proc.process_name.clone(),
                started_at: proc.started_at,
                uptime_seconds: uptime,
                restart_count: proc.restart_count,
                tags: proc.config.tags.clone(),
                cpu_percent,
                memory_mb,
                access_group: proc.config.access_group.clone(),
                cwd: proc.config.cwd.clone(),
                detected_ports,
            });
        }

        Ok(infos)
    }

    pub async fn kill_process(&self, id: &ProcessId) -> Result<()> {
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;

        // Update status to stopping
        self.storage.update_process_status(id, ProcessStatus::Stopping, process_record.session_pid).await?;

        // Handle tmux session termination
        if let Some(tmux_session) = &process_record.tmux_session {
            use crate::tmux::TmuxManager;
            TmuxManager::kill_session(tmux_session)?;
        } else if let Some(pid) = process_record.session_pid {
            // For non-tmux processes, try to kill by PID
            use std::process::Command;
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .output();
        }

        // Update status to stopped
        self.storage.update_process_status(id, ProcessStatus::Stopped, process_record.session_pid).await?;
        
        // Remove from health monitor
        self.health_monitor.remove_process(id).await;
        
        // Remove from exit monitor if we have a PID
        if let Some(pid) = process_record.session_pid {
            self.exit_monitor.unregister_process(pid).await;
        }
        
        Ok(())
    }

    pub async fn restart_process(&self, id: &ProcessId) -> Result<ProcessInfo> {
        use crate::tmux::TmuxManager;
        
        // Get the existing process config
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let old_restart_count = process_record.restart_count;
        let config = process_record.config.clone();
        
        // Kill the existing process
        self.kill_process(id).await?;
        
        // Increment restart count in database
        self.storage.increment_restart_count(id).await?;

        // Create new process with same config
        let process = if config.use_tmux && TmuxManager::is_available() {
            self.spawn_with_tmux(id.clone(), config.clone()).await?
        } else if config.pty {
            self.spawn_with_pty(id.clone(), config.clone()).await?
        } else {
            self.spawn_without_pty(id.clone(), config.clone()).await?
        };
        
        // Update process in database with new status and PID
        let tmux_session = if config.use_tmux && TmuxManager::is_available() {
            Some(format!("apm-{}", id.0))
        } else {
            None
        };
        
        self.storage.store_process(
            id,
            &config.name,
            &config.command,
            &config.args,
            process.status,
            &config,
            tmux_session.as_deref()
        ).await?;
        
        if let Some(session_pid) = process.session_pid {
            self.storage.update_process_status(id, process.status, Some(session_pid)).await?;
        }
        
        // Get initial health metrics if PID is available
        let (cpu_percent, memory_mb) = if let Some(session_pid) = process.session_pid {
            self.health_monitor.update_health(id, session_pid).await;
            if let Some(health) = self.health_monitor.get_health(id).await {
                (Some(health.cpu_percent), Some(health.memory_mb))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        let info = ProcessInfo {
            id: id.clone(),
            name: config.name.clone(),
            command: config.command.clone(),
            args: config.args.clone(),
            status: process.status,
            session_pid: process.session_pid,
            process_pid: None, // Will be populated by monitoring
            process_name: None, // Will be populated by monitoring
            started_at: process.started_at,
            uptime_seconds: 0,
            restart_count: old_restart_count + 1,
            tags: config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: config.access_group.clone(),
            cwd: config.cwd.clone(),
            detected_ports: vec![], // Will be populated as logs are processed
        };

        // Extract tmux session before moving process
        let tmux_session = process.tmux_session.clone();
        
        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process).await;
        
        // Start monitoring process health
        if let Some(session_pid) = info.session_pid {
            if let Some(session) = tmux_session {
                // Use tmux-specific health monitoring that tracks actual command
                self.start_tmux_health_monitoring(id.clone(), session_pid, session).await;
            } else {
                // Use regular health monitoring for non-tmux processes
                self.start_health_monitoring(id.clone(), session_pid).await;
            }
        }

        Ok(info)
    }

    pub async fn get_pty_master(
        &self,
        _id: &ProcessId,
    ) -> Result<Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>> {
        // Since we're using database storage, PTY masters are not persisted
        // This method should return None for database-backed processes
        // In the future, we could implement a PTY registry if needed
        Ok(None)
    }
    
    pub async fn get_tmux_session(
        &self,
        id: &ProcessId,
    ) -> Result<Option<String>> {
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        Ok(process_record.tmux_session)
    }
    
    pub async fn cleanup_tmux_sessions(&self) -> Result<()> {
        use crate::tmux::TmuxManager;
        
        let stopped_processes = self.storage.list_processes(Some(ProcessStatus::Stopped)).await?;
        for proc in stopped_processes {
            if let Some(session) = &proc.tmux_session {
                info!("Cleaning up tmux session {} for stopped process {}", session, proc.id);
                let _ = TmuxManager::kill_session(session);
            }
        }
        Ok(())
    }

    // Tag management methods
    pub async fn add_tag(&self, id: &ProcessId, tag: &str) -> Result<()> {
        // Get the existing process record
        let mut process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        // Validate tag
        if tag.is_empty() || tag.len() > 50 {
            return Err(ApmError::InvalidInput("Tag must be 1-50 characters".to_string()));
        }
        
        // Only allow alphanumeric, dash, underscore, dot
        if !tag.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
            return Err(ApmError::InvalidInput("Tag can only contain alphanumeric characters, dash, underscore, and dot".to_string()));
        }
        
        // Add tag if not already present
        if !process_record.config.tags.contains(&tag.to_string()) {
            process_record.config.tags.push(tag.to_string());
            
            // Update process in storage
            self.storage.store_process(
                id,
                &process_record.config.name,
                &process_record.config.command,
                &process_record.config.args,
                process_record.status,
                &process_record.config,
                process_record.tmux_session.as_deref()
            ).await?;
        }
        
        Ok(())
    }
    
    pub async fn remove_tag(&self, id: &ProcessId, tag: &str) -> Result<()> {
        // Get the existing process record
        let mut process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        // Remove tag if present
        if let Some(pos) = process_record.config.tags.iter().position(|t| t == tag) {
            process_record.config.tags.remove(pos);
            
            // Update process in storage
            self.storage.store_process(
                id,
                &process_record.config.name,
                &process_record.config.command,
                &process_record.config.args,
                process_record.status,
                &process_record.config,
                process_record.tmux_session.as_deref()
            ).await?;
        }
        
        Ok(())
    }
}