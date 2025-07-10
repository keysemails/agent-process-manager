//! Process supervisor implementation with PTY support

use super::{ProcessId, ProcessInfo, ProcessStatus};
use super::health::HealthMonitor;
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
    pub pid: Option<u32>,
    pty_master: Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    tmux_session: Option<String>,
}

pub struct ProcessManager {
    storage: Arc<LogStorage>,
    log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>,
    health_monitor: Arc<HealthMonitor>,
}

impl ProcessManager {
    pub fn new(
        storage: Arc<LogStorage>,
        log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>
    ) -> Self {
        let health_monitor = Arc::new(HealthMonitor::new());
        
        // Start the health monitoring background task
        let monitor = health_monitor.clone();
        tokio::spawn(async move {
            monitor.start_monitoring().await;
        });
        
        Self {
            storage,
            log_sender,
            health_monitor,
        }
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
        Ok(())
    }
    
    fn clone_for_restart(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            log_sender: self.log_sender.clone(),
            health_monitor: self.health_monitor.clone(),
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
        let (cpu_percent, memory_mb) = if let Some(pid) = process.pid {
            self.health_monitor.update_health(&id, pid).await;
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
            pid: process.pid,
            started_at: process.started_at,
            uptime_seconds: 0,
            restart_count: process.restart_count,
            tags: process.config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: process.config.access_group.clone(),
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
        if let Some(pid) = process.pid {
            self.storage.update_process_status(&id, process.status, Some(pid)).await?;
        }

        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process).await;
        
        // Start monitoring process health
        if let Some(pid) = info.pid {
            self.start_health_monitoring(id, pid).await;
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
            }
        });
    }

    async fn spawn_with_tmux(&self, id: ProcessId, config: ProcessConfig) -> Result<Process> {
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
            pid,
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

        Ok(Process {
            id,
            config,
            status: ProcessStatus::Running,
            started_at: chrono::Utc::now(),
            restart_count: 0,
            pid,
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
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    
                    // Check if session still exists
                    if !TmuxManager::session_exists(&session) {
                        info!("Tmux session {} for process {} has ended", session, process_id);
                        // Update process status in database
                        let _ = storage.update_process_status(&process_id, ProcessStatus::Stopped, None).await;
                        break;
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
        let (cpu_percent, memory_mb) = if let Some(_pid) = process_record.pid {
            if let Some(health) = self.health_monitor.get_health(id).await {
                (Some(health.cpu_percent), Some(health.memory_mb))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        Ok(ProcessInfo {
            id: process_record.id.clone(),
            name: process_record.config.name.clone(),
            command: process_record.config.command.clone(),
            args: process_record.config.args.clone(),
            status: process_record.status,
            pid: process_record.pid,
            started_at: process_record.started_at,
            uptime_seconds: uptime,
            restart_count: process_record.restart_count,
            tags: process_record.config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: process_record.config.access_group.clone(),
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
            let (cpu_percent, memory_mb) = if let Some(_pid) = proc.pid {
                if let Some(health) = self.health_monitor.get_health(&proc.id).await {
                    (Some(health.cpu_percent), Some(health.memory_mb))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            
            infos.push(ProcessInfo {
                id: proc.id.clone(),
                name: proc.config.name.clone(),
                command: proc.config.command.clone(),
                args: proc.config.args.clone(),
                status: proc.status,
                pid: proc.pid,
                started_at: proc.started_at,
                uptime_seconds: uptime,
                restart_count: proc.restart_count,
                tags: proc.config.tags.clone(),
                cpu_percent,
                memory_mb,
                access_group: proc.config.access_group.clone(),
            });
        }

        Ok(infos)
    }

    pub async fn stop_process(&self, id: &ProcessId) -> Result<()> {
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;

        // Update status to stopping
        self.storage.update_process_status(id, ProcessStatus::Stopping, process_record.pid).await?;

        // Handle tmux session termination
        if let Some(tmux_session) = &process_record.tmux_session {
            use crate::tmux::TmuxManager;
            TmuxManager::kill_session(tmux_session)?;
        } else if let Some(pid) = process_record.pid {
            // For non-tmux processes, try to kill by PID
            use std::process::Command;
            let _ = Command::new("kill")
                .arg(pid.to_string())
                .output();
        }

        // Update status to stopped
        self.storage.update_process_status(id, ProcessStatus::Stopped, process_record.pid).await?;
        
        // Remove from health monitor
        self.health_monitor.remove_process(id).await;
        
        Ok(())
    }

    pub async fn restart_process(&self, id: &ProcessId) -> Result<ProcessInfo> {
        use crate::tmux::TmuxManager;
        
        // Get the existing process config
        let process_record = self.storage.get_process(id).await?
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let old_restart_count = process_record.restart_count;
        let config = process_record.config.clone();
        
        // Stop the existing process
        self.stop_process(id).await?;
        
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
        
        if let Some(pid) = process.pid {
            self.storage.update_process_status(id, process.status, Some(pid)).await?;
        }
        
        // Get initial health metrics if PID is available
        let (cpu_percent, memory_mb) = if let Some(pid) = process.pid {
            self.health_monitor.update_health(id, pid).await;
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
            pid: process.pid,
            started_at: process.started_at,
            uptime_seconds: 0,
            restart_count: old_restart_count + 1,
            tags: config.tags.clone(),
            cpu_percent,
            memory_mb,
            access_group: config.access_group.clone(),
        };

        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process).await;
        
        // Start monitoring process health
        if let Some(pid) = info.pid {
            self.start_health_monitoring(id.clone(), pid).await;
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
}