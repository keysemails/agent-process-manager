//! Process management module

pub mod supervisor;
pub mod health;

pub use supervisor::{ProcessManager, ProcessConfig, Process};
pub use health::HealthMonitor;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessId(pub Uuid);

impl ProcessId {
    pub fn new() -> Self {
        ProcessId(Uuid::new_v4())
    }
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub id: ProcessId,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub uptime_seconds: u64,
    pub restart_count: u32,
    pub tags: Vec<String>,
    pub cpu_percent: Option<f32>,
    pub memory_mb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<std::path::PathBuf>,
    #[serde(default)]
    pub detected_ports: Vec<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
    Restarting,
}