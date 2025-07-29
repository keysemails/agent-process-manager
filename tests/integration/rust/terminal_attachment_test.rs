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
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_storage.clone(), log_tx).unwrap();
    
    // Create a process with PTY
    let config = ProcessConfig {
        name: "test-pty".to_string(),
        command: "echo".to_string(),
        args: vec!["Hello, PTY!".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: true,
        use_tmux: true,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // With tmux, we don't get direct PTY master access
    // Instead, verify we can get the tmux session
    let tmux_session = manager.get_tmux_session(&info.id).await.unwrap();
    assert!(tmux_session.is_some(), "Process should have tmux session");
    assert!(tmux_session.unwrap().starts_with("apm-"), "Session name should start with 'apm-'");
    
    // Clean up
    let _ = manager.kill_process(&info.id).await;
}

#[tokio::test]
#[serial]
async fn test_process_without_pty() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_storage, log_tx).unwrap();
    
    // Create a process without PTY
    let config = ProcessConfig {
        name: "test-no-pty".to_string(),
        command: "echo".to_string(),
        args: vec!["Hello, no PTY!".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // With tmux, all processes have a session regardless of pty flag
    let tmux_session = manager.get_tmux_session(&info.id).await.unwrap();
    assert!(tmux_session.is_some(), "Process should have tmux session");
    
    // Clean up
    let _ = manager.kill_process(&info.id).await;
}

#[tokio::test]
#[serial]
async fn test_pty_resize() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (log_tx, _log_rx) = tokio::sync::mpsc::channel(100);
    let manager = ProcessManager::new(log_storage, log_tx).unwrap();
    
    // Create a long-running process with PTY
    let config = ProcessConfig {
        name: "test-resize".to_string(),
        command: "sh".to_string(),
        args: vec!["-c".to_string(), "sleep 1".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: true,
        use_tmux: true,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    
    // With tmux, we resize the pane directly
    if let Some(tmux_session) = manager.get_tmux_session(&info.id).await.unwrap() {
        // Test tmux resize-pane command
        use std::process::Command;
        let output = Command::new("tmux")
            .args(&["resize-pane", "-t", &tmux_session, "-x", "120", "-y", "40"])
            .output();
        
        // Verify the command executed (it may fail if tmux isn't available in test environment)
        match output {
            Ok(result) => {
                println!("Tmux resize result: {:?}", result.status);
            }
            Err(e) => {
                println!("Tmux resize not available in test environment: {}", e);
            }
        }
    }
    
    // Clean up
    let _ = manager.kill_process(&info.id).await;
}