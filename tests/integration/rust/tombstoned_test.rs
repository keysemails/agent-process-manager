use agent_process_manager::logs::LogStorage;
use tempfile::TempDir;
use agent_process_manager::process::{ProcessManager, ProcessStatus, ProcessId};
use agent_process_manager::process::supervisor::{ProcessConfig, RestartPolicy, ResourceLimits};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

// Helper function to create test ProcessManager
async fn create_test_manager() -> (ProcessManager, mpsc::Receiver<(ProcessId, String)>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let (tx, rx) = mpsc::channel(100);
    let manager = ProcessManager::new(log_storage, tx).unwrap();
    
    (manager, rx, temp_dir)
}

#[tokio::test]
async fn test_tombstoned_status() {
    let (manager, _rx, _temp_dir) = create_test_manager().await;
    
    // Spawn a long-running process
    let config = ProcessConfig {
        name: "test-tombstone".to_string(),
        command: "sleep".to_string(),
        args: vec!["60".to_string()],
        cwd: None,
        env: HashMap::new(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: RestartPolicy::default(),
        resources: ResourceLimits::default(),
        access_group: None,
    };
    
    let info = manager.spawn_process(config).await.unwrap();
    let process_id = info.id.clone();
    
    // Verify it's running
    assert_eq!(info.status, ProcessStatus::Running);
    
    // Wait a moment for process to fully start
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Kill the process
    manager.kill_process(&process_id).await.unwrap();
    
    // Check status - should be Tombstoned
    let killed_info = manager.get_process(&process_id).await.unwrap();
    assert_eq!(killed_info.status, ProcessStatus::Tombstoned);
}

#[tokio::test]
async fn test_stopped_vs_tombstoned() {
    let (manager, _rx, _temp_dir) = create_test_manager().await;
    
    // Test 1: Natural exit (Stopped)
    let config_natural = ProcessConfig {
        name: "test-natural-exit".to_string(),
        command: "echo".to_string(),
        args: vec!["done".to_string()],
        cwd: None,
        env: HashMap::new(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: RestartPolicy::default(),
        resources: ResourceLimits::default(),
        access_group: None,
    };
    
    let natural_info = manager.spawn_process(config_natural).await.unwrap();
    let natural_id = natural_info.id.clone();
    
    // Wait for process to complete naturally
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    let natural_final = manager.get_process(&natural_id).await.unwrap();
    assert_eq!(natural_final.status, ProcessStatus::Stopped);
    
    // Test 2: Force kill (Tombstoned)
    let config_killed = ProcessConfig {
        name: "test-force-kill".to_string(),
        command: "sleep".to_string(),
        args: vec!["60".to_string()],
        cwd: None,
        env: HashMap::new(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: RestartPolicy::default(),
        resources: ResourceLimits::default(),
        access_group: None,
    };
    
    let killed_info = manager.spawn_process(config_killed).await.unwrap();
    let killed_id = killed_info.id.clone();
    
    // Wait for it to start
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Kill it before it finishes
    manager.kill_process(&killed_id).await.unwrap();
    
    let killed_final = manager.get_process(&killed_id).await.unwrap();
    assert_eq!(killed_final.status, ProcessStatus::Tombstoned);
}

#[tokio::test]
async fn test_clean_includes_tombstoned() {
    let (manager, _rx, temp_dir) = create_test_manager().await;
    
    // Get the storage directly to test clean
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    let storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    
    // Create stopped process
    let config_stopped = ProcessConfig {
        name: "test-stopped-clean".to_string(),
        command: "echo".to_string(),
        args: vec!["done".to_string()],
        cwd: None,
        env: HashMap::new(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: RestartPolicy::default(),
        resources: ResourceLimits::default(),
        access_group: None,
    };
    
    let stopped_info = manager.spawn_process(config_stopped).await.unwrap();
    
    // Create tombstoned process
    let config_tombstoned = ProcessConfig {
        name: "test-tombstoned-clean".to_string(),
        command: "sleep".to_string(),
        args: vec!["60".to_string()],
        cwd: None,
        env: HashMap::new(),
        tags: vec![],
        pty: false,
        use_tmux: true,
        restart_policy: RestartPolicy::default(),
        resources: ResourceLimits::default(),
        access_group: None,
    };
    
    let tombstoned_info = manager.spawn_process(config_tombstoned).await.unwrap();
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    manager.kill_process(&tombstoned_info.id).await.unwrap();
    
    // Wait for statuses to update
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Verify both have expected statuses
    let stopped_final = manager.get_process(&stopped_info.id).await.unwrap();
    assert_eq!(stopped_final.status, ProcessStatus::Stopped);
    
    let tombstoned_final = manager.get_process(&tombstoned_info.id).await.unwrap();
    assert_eq!(tombstoned_final.status, ProcessStatus::Tombstoned);
    
    // Clean both types
    let cleaned = storage.clean_stopped_processes(None, None, false).await.unwrap();
    
    // Should have cleaned both
    assert_eq!(cleaned.len(), 2);
    let names: Vec<String> = cleaned.iter().map(|(_, name)| name.clone()).collect();
    assert!(names.contains(&"test-stopped-clean".to_string()));
    assert!(names.contains(&"test-tombstoned-clean".to_string()));
}