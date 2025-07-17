//! Health monitoring for processes

use super::ProcessId;
use sysinfo::{System, Pid};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProcessHealth {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub is_alive: bool,
}

#[derive(Debug, Clone)]
pub enum HealthEvent {
    ProcessDied(ProcessId),
}

pub struct HealthMonitor {
    system: Arc<RwLock<System>>,
    health_data: Arc<RwLock<HashMap<ProcessId, ProcessHealth>>>,
    event_sender: mpsc::Sender<HealthEvent>,
    event_receiver: Arc<RwLock<mpsc::Receiver<HealthEvent>>>,
}

impl HealthMonitor {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            health_data: Arc::new(RwLock::new(HashMap::new())),
            event_sender: tx,
            event_receiver: Arc::new(RwLock::new(rx)),
        }
    }
    
    pub fn get_event_receiver(&self) -> Arc<RwLock<mpsc::Receiver<HealthEvent>>> {
        self.event_receiver.clone()
    }

    pub async fn update_health(&self, process_id: &ProcessId, pid: u32) {
        self.update_health_with_actual_pid(process_id, pid, None).await;
    }

    /// Update health for a process, optionally tracking both shell and actual command PIDs
    pub async fn update_health_with_actual_pid(
        &self, 
        process_id: &ProcessId, 
        shell_pid: u32,
        actual_pid: Option<u32>
    ) {
        let mut system = self.system.write().await;
        
        // Determine which PID to monitor for health metrics
        let monitor_pid = actual_pid.unwrap_or(shell_pid);
        let pid = Pid::from(monitor_pid as usize);
        
        // Refresh specific processes
        let mut pids_to_refresh = vec![Pid::from(shell_pid as usize)];
        if let Some(actual) = actual_pid {
            pids_to_refresh.push(Pid::from(actual as usize));
        }
        
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&pids_to_refresh),
            sysinfo::ProcessRefreshKind::new()
                .with_cpu()
                .with_memory(),
        );

        // Check if process was previously alive
        let was_alive = self.health_data.read().await
            .get(process_id)
            .map(|h| h.is_alive)
            .unwrap_or(true);

        // Check if the monitored process exists
        if let Some(process) = system.process(pid) {
            let health = ProcessHealth {
                cpu_percent: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
                is_alive: true,
            };
            
            self.health_data.write().await.insert(process_id.clone(), health);
        } else {
            // Process not found - mark as not alive
            let health = ProcessHealth {
                cpu_percent: 0.0,
                memory_mb: 0,
                is_alive: false,
            };
            
            self.health_data.write().await.insert(process_id.clone(), health);
            
            // If process just died, send event
            if was_alive {
                let _ = self.event_sender.send(HealthEvent::ProcessDied(process_id.clone())).await;
            }
        }
    }

    pub async fn get_health(&self, process_id: &ProcessId) -> Option<ProcessHealth> {
        self.health_data.read().await.get(process_id).cloned()
    }

    pub async fn remove_process(&self, process_id: &ProcessId) {
        self.health_data.write().await.remove(process_id);
    }

    pub async fn start_monitoring(&self) {
        let system = self.system.clone();
        let _health_data = self.health_data.clone();

        tokio::spawn(async move {
            loop {
                // Update system info every 5 seconds
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                
                let mut sys = system.write().await;
                sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
                
                // In a real implementation, we'd update health for all tracked processes
                // For now, this is a placeholder
            }
        });
    }
}