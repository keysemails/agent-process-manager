//! Integration tests for the enhanced log summarization

use agent_process_manager::{
    process::ProcessManager,
    process::supervisor::{ProcessConfig, RestartPolicy, ResourceLimits},
    logs::{LogStorage, LogSummarizer},
    api,
};
use tempfile::TempDir;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

async fn setup_test_env() -> (Arc<ProcessManager>, Arc<LogStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    
    // Create log channel
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    
    let log_storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let process_manager = Arc::new(ProcessManager::new(log_storage.clone(), tx.clone()).unwrap());
    
    // Start log storage task
    let storage_clone = log_storage.clone();
    tokio::spawn(async move {
        while let Some((process_id, line)) = rx.recv().await {
            let _ = storage_clone.store(process_id, line).await;
        }
    });
    
    (process_manager, log_storage, temp_dir)
}

#[tokio::test]
async fn test_log_summary_with_real_data() {
    let (process_manager, log_storage, _temp_dir) = setup_test_env().await;
    
    // Spawn a test process that generates logs
    let config = ProcessConfig {
        name: "test-logger".to_string(),
        command: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            r#"
            echo 'Server starting...'
            echo 'Listening on port 8080'
            echo 'ERROR: Connection refused'
            echo 'ERROR: Connection refused'
            echo 'WARNING: High memory usage'
            sleep 1
            "#.to_string()
        ],
        env: std::collections::HashMap::new(),
        cwd: None,
        tags: vec![],
        pty: false,
        resources: ResourceLimits::default(),
        restart_policy: RestartPolicy::default(),
        use_tmux: true,
        access_group: None,
    };
    
    let process_info = process_manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.clone();
    
    // Wait for logs to be processed
    sleep(Duration::from_secs(2)).await;
    
    // Get the logs and create summary
    let logs = log_storage.query(agent_process_manager::logs::LogQuery {
        process_id: process_id.clone(),
        format: agent_process_manager::logs::LogFormat::Summary,
        lines: Some(1000),
        search: None,
        level: None,
        since: None,
    }).await.unwrap();
    
    let summarizer = LogSummarizer::new();
    let summary = summarizer.summarize_logs(&logs, Some(&process_info));
    
    // Check detected patterns
    assert!(summary.detected_ports.contains(&8080), "Should detect port 8080");
    assert!(summary.recent_errors.len() >= 2, "Should have at least 2 errors");
    assert!(summary.key_events.len() > 0, "Should have key events");
    
    // Stop the process
    let _ = process_manager.kill_process(&process_id).await;
}

#[tokio::test]
async fn test_error_pattern_detection() {
    let (process_manager, log_storage, _temp_dir) = setup_test_env().await;
    
    // Spawn a process that generates various error patterns
    let config = ProcessConfig {
        name: "error-generator".to_string(),
        command: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            r#"
            echo 'ERROR: Failed to connect to database at /var/lib/postgres/data'
            echo 'ERROR: Failed to connect to database at /usr/local/postgres/data'
            echo 'ERROR: Port 5432 already in use'
            echo 'ERROR: Port 8080 already in use'
            echo 'ERROR: Timeout connecting to redis:6379'
            echo 'ERROR: Timeout connecting to redis:6380'
            "#.to_string()
        ],
        env: std::collections::HashMap::new(),
        cwd: None,
        tags: vec![],
        pty: false,
        resources: ResourceLimits::default(),
        restart_policy: RestartPolicy::default(),
        use_tmux: true,
        access_group: None,
    };
    
    let process_info = process_manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.clone();
    
    // Wait for logs to be processed
    sleep(Duration::from_secs(2)).await;
    
    // Get the logs and create summary
    let logs = log_storage.query(agent_process_manager::logs::LogQuery {
        process_id: process_id.clone(),
        format: agent_process_manager::logs::LogFormat::Summary,
        lines: Some(1000),
        search: None,
        level: None,
        since: None,
    }).await.unwrap();
    
    let summarizer = LogSummarizer::new();
    let summary = summarizer.summarize_logs(&logs, Some(&process_info));
    
    // Should have detected multiple errors
    assert!(summary.recent_errors.len() >= 6, "Should have at least 6 errors");
    assert!(summary.error_patterns.len() > 0, "Should have error patterns");
    
    // Stop the process
    let _ = process_manager.kill_process(&process_id).await;
}

#[tokio::test]
async fn test_enhanced_api_with_summarizer() {
    let (process_manager, log_storage, _temp_dir) = setup_test_env().await;
    
    // Create API router
    let app = api::create_router(process_manager.clone(), log_storage.clone(), None);
    
    // Spawn a test process
    let config = ProcessConfig {
        name: "api-test-process".to_string(),
        command: "sh".to_string(),
        args: vec![
            "-c".to_string(),
            r#"
            while true; do
                echo 'Server running on port 3000'
                echo 'ERROR: Request timeout'
                sleep 1
            done
            "#.to_string()
        ],
        env: std::collections::HashMap::new(),
        cwd: None,
        tags: vec![],
        pty: false,
        resources: ResourceLimits::default(),
        restart_policy: RestartPolicy::default(),
        use_tmux: true,
        access_group: None,
    };
    
    let process_info = process_manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.clone();
    
    // Wait for some logs
    sleep(Duration::from_secs(3)).await;
    
    // Query the API for system overview
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(serde_json::json!({
            "type": "system_overview"
        }).to_string()))
        .unwrap();
    
    let response = tower::ServiceExt::oneshot(app.clone(), request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Check that the enhanced data is present
    assert!(json["data"]["data"]["networking"]["ports_in_use"].as_array().unwrap().contains(&serde_json::json!(3000)));
    assert!(json["data"]["data"]["errors"]["total_count"].as_u64().unwrap() > 0);
    
    // Stop the process
    let _ = process_manager.kill_process(&process_id).await;
}