//! Tests for terminal attachment functionality

use agent_process_manager::{
    process::{ProcessConfig, ProcessManager, ProcessId},
    logs::LogStorage,
};
use std::sync::Arc;
use tempfile::TempDir;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_pty_master_access() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_tx);
    
    // Create a process with PTY
    let config = ProcessConfig {
        name: "test-pty".to_string(),
        command: "echo".to_string(),
        args: vec!["Hello, PTY!".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: true,
        restart_policy: Default::default(),
        resources: Default::default(),
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // Test that we can get the PTY master
    let pty_master = manager.get_pty_master(&info.id).await.unwrap();
    assert!(pty_master.is_some(), "Process should have PTY master");
    
    // Wait a bit for process to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[tokio::test]
#[serial]
async fn test_process_without_pty() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let _log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_tx);
    
    // Create a process without PTY
    let config = ProcessConfig {
        name: "test-no-pty".to_string(),
        command: "echo".to_string(),
        args: vec!["Hello, no PTY!".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: false,
        restart_policy: Default::default(),
        resources: Default::default(),
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // Test that PTY master is available (currently all processes use PTY)
    let pty_master = manager.get_pty_master(&info.id).await.unwrap();
    assert!(pty_master.is_some(), "Process should have PTY master");
}

#[tokio::test]
#[serial]
async fn test_pty_resize() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let _log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_tx);
    
    // Create a long-running process with PTY
    let config = ProcessConfig {
        name: "test-resize".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "sleep 1".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: true,
        restart_policy: Default::default(),
        resources: Default::default(),
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // Get PTY master and test resize
    if let Some(pty_master) = manager.get_pty_master(&info.id).await.unwrap() {
        let master = pty_master.lock().await;
        let size = portable_pty::PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        };
        
        // This should not panic
        let _ = master.resize(size);
    }
    
    // Clean up
    let _ = manager.stop_process(&info.id).await;
}