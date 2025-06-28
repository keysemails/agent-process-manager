//! Health monitoring for processes

use super::ProcessId;
use sysinfo::{System, Pid};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProcessHealth {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub is_alive: bool,
}

pub struct HealthMonitor {
    system: Arc<RwLock<System>>,
    health_data: Arc<RwLock<HashMap<ProcessId, ProcessHealth>>>,
}

impl HealthMonitor {
    pub fn new() -> Self {
        Self {
            system: Arc::new(RwLock::new(System::new_all())),
            health_data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_health(&self, process_id: &ProcessId, pid: u32) {
        let mut system = self.system.write().await;
        let pid = Pid::from(pid as usize);
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            sysinfo::ProcessRefreshKind::new()
                .with_cpu()
                .with_memory(),
        );

        if let Some(process) = system.process(pid) {
            let health = ProcessHealth {
                cpu_percent: process.cpu_usage(),
                memory_mb: process.memory() / 1024 / 1024,
                is_alive: true,
            };
            
            self.health_data.write().await.insert(process_id.clone(), health);
        } else {
            // Process not found - mark as not alive
            self.health_data.write().await.insert(
                process_id.clone(),
                ProcessHealth {
                    cpu_percent: 0.0,
                    memory_mb: 0,
                    is_alive: false,
                }
            );
        }
    }

    pub async fn get_health(&self, process_id: &ProcessId) -> Option<ProcessHealth> {
        self.health_data.read().await.get(process_id).cloned()
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