//! Process supervisor implementation with PTY support

use super::{ProcessId, ProcessInfo, ProcessStatus};
use super::health::HealthMonitor;
use crate::{ApmError, Result, tmux::TmuxManager};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
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
    processes: Arc<RwLock<HashMap<ProcessId, Arc<RwLock<Process>>>>>,
    log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>,
    health_monitor: Arc<HealthMonitor>,
}

impl ProcessManager {
    pub fn new(log_sender: tokio::sync::mpsc::Sender<(ProcessId, String)>) -> Self {
        let health_monitor = Arc::new(HealthMonitor::new());
        
        // Start the health monitoring background task
        let monitor = health_monitor.clone();
        tokio::spawn(async move {
            monitor.start_monitoring().await;
        });
        
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            log_sender,
            health_monitor,
        }
    }
    
    fn clone_for_restart(&self) -> Self {
        Self {
            processes: self.processes.clone(),
            log_sender: self.log_sender.clone(),
            health_monitor: self.health_monitor.clone(),
        }
    }

    pub async fn spawn_process(&self, config: ProcessConfig) -> Result<ProcessInfo> {
        let id = ProcessId::new();
        info!("Spawning process '{}' with ID {}", config.name, id);

        let process = if config.use_tmux && TmuxManager::is_available() {
            self.spawn_with_tmux(id.clone(), config).await?
        } else if config.pty {
            self.spawn_with_pty(id.clone(), config).await?
        } else {
            self.spawn_without_pty(id.clone(), config).await?
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
        };

        let process_arc = Arc::new(RwLock::new(process));
        self.processes.write().await.insert(id.clone(), process_arc.clone());

        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process_arc.clone()).await;
        
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
        process: Arc<RwLock<Process>>,
    ) {
        // Check if this is a tmux session
        let is_tmux = {
            let proc = process.read().await;
            proc.tmux_session.is_some()
        };
        
        if is_tmux {
            // For tmux sessions, monitoring is handled by the pipe-pane setup
            // We just need to monitor if the session is still alive
            let session_name = {
                let proc = process.read().await;
                proc.tmux_session.clone()
            };
            
            if let Some(session) = session_name {
                let processes = self.processes.clone();
                let id_clone = id.clone();
                
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        
                        if !TmuxManager::session_exists(&session) {
                            info!("tmux session {} has ended", session);
                            
                            // Update process status
                            if let Some(proc_arc) = processes.read().await.get(&id_clone) {
                                let mut proc = proc_arc.write().await;
                                proc.status = ProcessStatus::Stopped;
                            }
                            break;
                        }
                    }
                });
            }
            return;
        }
        
        // Original PTY monitoring code
        let log_sender = self.log_sender.clone();
        let processes = self.processes.clone();
        let manager_ref = self.clone_for_restart();
        
        tokio::spawn(async move {
            let reader = {
                let proc = process.read().await;
                match &proc.pty_master {
                    Some(master) => {
                        let master_guard = master.lock().await;
                        master_guard.try_clone_reader()
                    }
                    None => {
                        error!("No PTY master for process {}", id);
                        return;
                    }
                }
            };

            let reader = match reader {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to clone PTY reader: {}", e);
                    return;
                }
            };

            // Use a thread to bridge sync/async gap
            let id_clone = id.clone();
            let process_clone = process.clone();
            let processes_clone = processes.clone();
            let manager_for_restart = manager_ref;
            std::thread::spawn(move || {
                use std::io::{BufRead, BufReader};
                let mut reader = BufReader::new(reader);
                let mut line = String::new();
                
                loop {
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            // EOF - process has exited
                            info!("Process {} has exited (thread: {:?})", id_clone, std::thread::current().id());
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            rt.block_on(async {
                                let mut proc = process_clone.write().await;
                                let prev_status = proc.status;
                                proc.status = ProcessStatus::Stopped;
                                
                                info!("Process {} status changed from {:?} to Stopped", id_clone, prev_status);
                                
                                // Check if we need to restart
                                if proc.config.restart_policy.enabled && 
                                   proc.restart_count < proc.config.restart_policy.max_retries {
                                    let restart_count = proc.restart_count;
                                    let max_retries = proc.config.restart_policy.max_retries;
                                    info!("Process {} will restart, current count: {}/{}", id_clone, restart_count, max_retries);
                                    drop(proc); // Release lock before restart
                                    
                                    // Wait for backoff
                                    let backoff = process_clone.read().await.config.restart_policy.backoff_ms;
                                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                                    
                                    // Attempt restart
                                    match manager_for_restart.restart_process(&id_clone).await {
                                        Ok(info) => {
                                            info!("Process {} restarted successfully, new PID: {:?}, restart count: {}", 
                                                  id_clone, info.pid, info.restart_count);
                                        }
                                        Err(e) => {
                                            error!("Failed to restart process {}: {}", id_clone, e);
                                            // Mark as failed if we can't restart
                                            if let Some(proc_arc) = processes_clone.read().await.get(&id_clone) {
                                                proc_arc.write().await.status = ProcessStatus::Failed;
                                            }
                                        }
                                    }
                                } else if proc.config.restart_policy.enabled {
                                    // Max retries exceeded
                                    info!("Process {} max retries exceeded, marking as failed", id_clone);
                                    proc.status = ProcessStatus::Failed;
                                }
                            });
                            break;
                        }
                        Ok(_) => {
                            // Send log line to storage
                            let line_to_send = line.clone();
                            let id_for_send = id_clone.clone();
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            if let Err(e) = rt.block_on(async {
                                log_sender.send((id_for_send, line_to_send)).await
                            }) {
                                error!("Failed to send log line: {}", e);
                            }
                            line.clear();
                        }
                        Err(e) => {
                            error!("Error reading from process {}: {}", id_clone, e);
                            break;
                        }
                    }
                }
            });
        });
    }

    pub async fn get_process(&self, id: &ProcessId) -> Result<ProcessInfo> {
        let processes = self.processes.read().await;
        let process = processes.get(id)
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let proc = process.read().await;
        let uptime = chrono::Utc::now()
            .signed_duration_since(proc.started_at)
            .num_seconds() as u64;

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
        
        Ok(ProcessInfo {
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
        })
    }

    pub async fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        let processes = self.processes.read().await;
        let mut infos = Vec::new();

        for (_, process) in processes.iter() {
            let proc = process.read().await;
            let uptime = chrono::Utc::now()
                .signed_duration_since(proc.started_at)
                .num_seconds() as u64;

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
            });
        }

        Ok(infos)
    }

    pub async fn stop_process(&self, id: &ProcessId) -> Result<()> {
        let processes = self.processes.read().await;
        let process = processes.get(id)
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;

        let mut proc = process.write().await;
        proc.status = ProcessStatus::Stopping;

        if let Some(mut child) = proc.child.take() {
            child.kill()
                .map_err(|e| ApmError::Process(format!("Failed to kill process: {}", e)))?;
        }

        proc.status = ProcessStatus::Stopped;
        Ok(())
    }

    pub async fn restart_process(&self, id: &ProcessId) -> Result<ProcessInfo> {
        self.stop_process(id).await?;
        
        let (config, old_restart_count) = {
            let processes = self.processes.read().await;
            let process = processes.get(id)
                .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
            let proc = process.read().await;
            (proc.config.clone(), proc.restart_count)
        };

        // Create new process with same ID
        let process = if config.pty {
            self.spawn_with_pty(id.clone(), config).await?
        } else {
            self.spawn_without_pty(id.clone(), config).await?
        };
        
        // Update restart count
        let mut process = process;
        process.restart_count = old_restart_count + 1;
        
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
        };

        let process_arc = Arc::new(RwLock::new(process));
        
        // Update the existing process in the map
        self.processes.write().await.insert(id.clone(), process_arc.clone());

        // Start monitoring the process output
        self.monitor_process_output(id.clone(), process_arc.clone()).await;
        
        // Start monitoring process health
        if let Some(pid) = info.pid {
            self.start_health_monitoring(id.clone(), pid).await;
        }

        Ok(info)
    }

    pub async fn get_pty_master(
        &self,
        id: &ProcessId,
    ) -> Result<Option<Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>>> {
        let processes = self.processes.read().await;
        let process = processes.get(id)
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let proc = process.read().await;
        Ok(proc.pty_master.clone())
    }
    
    pub async fn get_tmux_session(
        &self,
        id: &ProcessId,
    ) -> Result<Option<String>> {
        let processes = self.processes.read().await;
        let process = processes.get(id)
            .ok_or_else(|| ApmError::NotFound(format!("Process {} not found", id)))?;
        
        let proc = process.read().await;
        Ok(proc.tmux_session.clone())
    }
    
    pub async fn cleanup_tmux_sessions(&self) -> Result<()> {
        let processes = self.processes.read().await;
        for (id, process) in processes.iter() {
            let proc = process.read().await;
            if let Some(session) = &proc.tmux_session {
                if proc.status == ProcessStatus::Stopped {
                    info!("Cleaning up tmux session {} for stopped process {}", session, id);
                    let _ = TmuxManager::kill_session(session);
                }
            }
        }
        Ok(())
    }
}