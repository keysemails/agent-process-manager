use agent_process_manager::{ApmError, process::{ProcessManager, ProcessConfig}};
use agent_process_manager::logs::LogStorage;
use tempfile::TempDir;
use std::sync::Arc;
use tokio::sync::mpsc;

async fn setup_test_manager() -> (Arc<ProcessManager>, Arc<LogStorage>, TempDir) {
    let (tx, _rx) = mpsc::channel(100);
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let manager = Arc::new(ProcessManager::new(storage.clone(), tx).unwrap());
    
    (manager, storage, temp_dir)
}

#[tokio::test]
async fn test_tag_validation() {
    let (manager, _storage, _temp_dir) = setup_test_manager().await;
    
    // Create a test process
    let config = ProcessConfig {
        name: "test-process".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec![],
        pty: false,
        use_tmux: false,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let process_info = manager.spawn_process(config).await.unwrap();
    let process_id = &process_info.id;
    
    // Test valid tags
    assert!(manager.add_tag(process_id, "valid-tag").await.is_ok());
    assert!(manager.add_tag(process_id, "valid_tag").await.is_ok());
    assert!(manager.add_tag(process_id, "valid.tag").await.is_ok());
    assert!(manager.add_tag(process_id, "123").await.is_ok());
    assert!(manager.add_tag(process_id, "a").await.is_ok()); // Single character
    
    // Test invalid tags
    assert!(matches!(
        manager.add_tag(process_id, "").await,
        Err(ApmError::InvalidInput(_))
    ));
    assert!(matches!(
        manager.add_tag(process_id, "tag with spaces").await,
        Err(ApmError::InvalidInput(_))
    ));
    assert!(matches!(
        manager.add_tag(process_id, "tag@special").await,
        Err(ApmError::InvalidInput(_))
    ));
    assert!(matches!(
        manager.add_tag(process_id, "tag!").await,
        Err(ApmError::InvalidInput(_))
    ));
    
    // Test tag length limit (51 characters)
    let long_tag = "a".repeat(51);
    assert!(matches!(
        manager.add_tag(process_id, &long_tag).await,
        Err(ApmError::InvalidInput(_))
    ));
    
    // 50 characters should be ok
    let max_tag = "a".repeat(50);
    assert!(manager.add_tag(process_id, &max_tag).await.is_ok());
}

#[tokio::test]
async fn test_add_and_remove_tags() {
    let (manager, _storage, _temp_dir) = setup_test_manager().await;
    
    // Create a test process with initial tags
    let config = ProcessConfig {
        name: "test-process".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec!["initial".to_string()],
        pty: false,
        use_tmux: false,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let process_info = manager.spawn_process(config).await.unwrap();
    let process_id = &process_info.id;
    
    // Check initial tags
    let info = manager.get_process(process_id).await.unwrap();
    assert_eq!(info.tags, vec!["initial"]);
    
    // Add tags
    manager.add_tag(process_id, "tag1").await.unwrap();
    manager.add_tag(process_id, "tag2").await.unwrap();
    
    let info = manager.get_process(process_id).await.unwrap();
    assert_eq!(info.tags.len(), 3);
    assert!(info.tags.contains(&"initial".to_string()));
    assert!(info.tags.contains(&"tag1".to_string()));
    assert!(info.tags.contains(&"tag2".to_string()));
    
    // Test adding duplicate tag (should not add again)
    manager.add_tag(process_id, "tag1").await.unwrap();
    let info = manager.get_process(process_id).await.unwrap();
    assert_eq!(info.tags.len(), 3); // Still 3 tags
    
    // Remove tags
    manager.remove_tag(process_id, "tag1").await.unwrap();
    let info = manager.get_process(process_id).await.unwrap();
    assert_eq!(info.tags.len(), 2);
    assert!(!info.tags.contains(&"tag1".to_string()));
    assert!(info.tags.contains(&"initial".to_string()));
    assert!(info.tags.contains(&"tag2".to_string()));
    
    // Remove non-existent tag (should not error)
    manager.remove_tag(process_id, "non-existent").await.unwrap();
    let info = manager.get_process(process_id).await.unwrap();
    assert_eq!(info.tags.len(), 2); // Still 2 tags
}

#[tokio::test]
async fn test_process_filtering_by_tags() {
    let (manager, _storage, _temp_dir) = setup_test_manager().await;
    
    // Create processes with different tags
    let configs = vec![
        ("process1", vec!["web", "frontend"]),
        ("process2", vec!["web", "backend"]),
        ("process3", vec!["db", "backend"]),
        ("process4", vec!["cache"]),
        ("process5", vec![]), // No tags
    ];
    
    for (name, tags) in configs {
        let config = ProcessConfig {
            name: name.to_string(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            cwd: None,
            env: Default::default(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            pty: false,
            use_tmux: false,
            restart_policy: Default::default(),
            resources: Default::default(),
            access_group: None,
        };
        manager.spawn_process(config).await.unwrap();
    }
    
    // Get all processes
    let all_processes = manager.list_processes().await.unwrap();
    assert_eq!(all_processes.len(), 5);
    
    // Manual filtering tests (simulating what the API/CLI does)
    
    // OR logic: processes with "web" OR "cache" tags
    let or_tags = vec!["web", "cache"];
    let filtered: Vec<_> = all_processes.iter()
        .filter(|p| {
            or_tags.iter().any(|tag| p.tags.contains(&tag.to_string()))
        })
        .collect();
    assert_eq!(filtered.len(), 3); // process1, process2, process4
    
    // AND logic: processes with "web" AND "backend" tags
    let and_tags = vec!["web", "backend"];
    let filtered: Vec<_> = all_processes.iter()
        .filter(|p| {
            and_tags.iter().all(|tag| p.tags.contains(&tag.to_string()))
        })
        .collect();
    assert_eq!(filtered.len(), 1); // Only process2
    
    // Single tag filter
    let filtered: Vec<_> = all_processes.iter()
        .filter(|p| p.tags.contains(&"backend".to_string()))
        .collect();
    assert_eq!(filtered.len(), 2); // process2, process3
    
    // Empty tag filter (should return all)
    let filtered: Vec<_> = all_processes.iter()
        .filter(|_| true)
        .collect();
    assert_eq!(filtered.len(), 5);
}

#[tokio::test]
async fn test_tag_persistence() {
    let (manager, _storage, _temp_dir) = setup_test_manager().await;
    
    // Create a process with tags
    let config = ProcessConfig {
        name: "persistent-process".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        pty: false,
        use_tmux: false,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    
    let process_info = manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.clone();
    
    // Add more tags
    manager.add_tag(&process_id, "tag3").await.unwrap();
    
    // Verify tags are stored in the database
    let process_record = storage.get_process(&process_id).await.unwrap().unwrap();
    assert_eq!(process_record.config.tags.len(), 3);
    assert!(process_record.config.tags.contains(&"tag1".to_string()));
    assert!(process_record.config.tags.contains(&"tag2".to_string()));
    assert!(process_record.config.tags.contains(&"tag3".to_string()));
    
    // Simulate loading process from database (tags should persist)
    let loaded_info = manager.get_process(&process_id).await.unwrap();
    assert_eq!(loaded_info.tags.len(), 3);
    assert_eq!(loaded_info.tags, process_record.config.tags);
}