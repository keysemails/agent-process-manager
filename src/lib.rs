//! Agent Process Manager - AI-native process management
//! 
//! This crate provides intelligent process management specifically designed
//! for AI agents, reducing context usage while maintaining full human access.

pub mod api;
pub mod config;
pub mod logs;
pub mod process;
pub mod tmux;
pub mod mcp;
pub mod utils;
pub mod agent;

#[cfg(test)]
pub mod test_utils;

// Re-export important types
pub use process::{Process, ProcessConfig, ProcessId, ProcessManager};
pub use logs::{LogEntry, LogStorage, LogQuery};
pub use config::Config;

// Error types
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApmError {
    #[error("Process error: {0}")]
    Process(String),
    
    #[error("Process error: {0}")]
    ProcessError(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Pattern error: {0}")]
    Pattern(#[from] regex::Error),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ApmError>;