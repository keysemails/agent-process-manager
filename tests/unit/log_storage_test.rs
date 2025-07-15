use agent_process_manager::logs::{LogEntry, LogStorage, LogQuery, LogLevel, LogFormat};
use agent_process_manager::process::ProcessId;
use chrono::{Utc, Duration};
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

async fn create_test_storage() -> (Arc<LogStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let storage = LogStorage::new(&db_url).await.unwrap();
    (Arc::new(storage), temp_dir)
}

#[tokio::test]
#[serial]
async fn test_store_and_retrieve_logs() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store some log entries using the store method
    for i in 0..5 {
        storage.store(process_id.clone(), format!("Test log line {}", i)).await.unwrap();
    }
    
    // Retrieve logs
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: None,
    };
    
    let retrieved = storage.query(query).await.unwrap();
    
    // Check we got logs back
    assert!(!retrieved.is_empty());
}

#[tokio::test]
#[serial]
async fn test_query_by_level() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store logs with different levels
    let lines = vec![
        "INFO: This is an info message",
        "WARN: This is a warning",
        "ERROR: This is an error",
        "DEBUG: This is debug info",
        "INFO: Another info message",
    ];
    
    for line in &lines {
        storage.store(process_id.clone(), line.to_string()).await.unwrap();
    }
    
    // Query for error level
    let query = LogQuery {
        process_id: process_id.clone(),
        level: Some(LogLevel::Error),
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: None,
    };
    
    let result = storage.query(query).await.unwrap();
    
    
    // Should have at least one error
    assert!(!result.is_empty(), "No results returned for error level query");
    assert!(result.iter().any(|e| e.level == LogLevel::Error), 
            "No error level entries found. Got: {:?}", 
            result.iter().map(|e| &e.level).collect::<Vec<_>>());
}

#[tokio::test]
#[serial]
async fn test_query_by_time_range() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store some logs
    for i in 0..5 {
        storage.store(process_id.clone(), format!("Log entry {}", i)).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Query for logs since 1 second ago
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: Some(Utc::now() - Duration::seconds(1)),
    };
    
    let recent_logs = storage.query(query).await.unwrap();
    assert!(!recent_logs.is_empty());
}

#[tokio::test]
#[serial]
async fn test_search_pattern() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store logs with specific patterns
    let entries = vec![
        "Starting server on port 8080",
        "Error connecting to database",
        "Server ready at http://localhost:8080",
        "Normal log without patterns",
    ];
    
    for entry in &entries {
        storage.store(process_id.clone(), entry.to_string()).await.unwrap();
    }
    
    // Search for logs containing "port"
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: Some("port".to_string()),
        since: None,
    };
    
    let port_logs = storage.query(query).await.unwrap();
    assert!(port_logs.iter().any(|e| e.raw_line.contains("port")));
}

#[tokio::test]
#[serial]
async fn test_log_lines_limit() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store 20 log entries
    for i in 0..20 {
        storage.store(process_id.clone(), format!("Log line {}", i)).await.unwrap();
    }
    
    // Query with limit
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: Some(5),
        search: None,
        since: None,
    };
    
    let limited = storage.query(query).await.unwrap();
    assert!(limited.len() <= 5);
}

#[tokio::test]
#[serial]
async fn test_get_raw_logs() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store logs
    for i in 0..10 {
        storage.store(process_id.clone(), format!("Test log {}", i)).await.unwrap();
    }
    
    // Get raw logs
    let raw_logs = storage.get_raw_logs(&process_id, None).await.unwrap();
    assert!(!raw_logs.is_empty());
    
    // Should be plain text with newlines
    assert!(raw_logs.contains('\n'));
}

#[tokio::test]
#[serial]
async fn test_delete_process_logs() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store some logs
    for i in 0..5 {
        storage.store(process_id.clone(), format!("Log {}", i)).await.unwrap();
    }
    
    // Verify logs exist
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: None,
    };
    let logs = storage.query(query.clone()).await.unwrap();
    assert!(!logs.is_empty());
    
    // Delete logs
    storage.clear_logs(&process_id).await.unwrap();
    
    // Verify logs are gone
    let logs_after = storage.query(query).await.unwrap();
    assert!(logs_after.is_empty());
}

#[tokio::test]
#[serial]
async fn test_log_summary() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store logs with various patterns
    let test_data = vec![
        "INFO: Server starting...",
        "INFO: Listening on port 8080",
        "WARN: Connection timeout",
        "ERROR: Failed to connect to database",
        "ERROR: Retrying connection...",
        "INFO: Connected to http://db.example.com",
    ];
    
    for line in &test_data {
        storage.store(process_id.clone(), line.to_string()).await.unwrap();
    }
    
    // Get summary format
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Summary,
        lines: None,
        search: None,
        since: None,
    };
    
    let summary = storage.query(query).await.unwrap();
    
    // Verify we got a summary
    assert!(!summary.is_empty());
}

#[tokio::test]
#[serial]
async fn test_concurrent_log_writes() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Spawn multiple tasks writing logs concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let storage = storage.clone();
        let pid = process_id.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                storage.store(pid.clone(), format!("Task {} log {}", i, j)).await.unwrap();
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    futures::future::join_all(handles).await;
    
    // Verify logs were written
    let query = LogQuery {
        process_id: process_id.clone(),
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: None,
    };
    
    let logs = storage.query(query).await.unwrap();
    assert!(!logs.is_empty());
}

#[tokio::test]
#[serial]
async fn test_ansi_escape_stripping() {
    let (storage, _temp_dir) = create_test_storage().await;
    let process_id = ProcessId::new();
    
    // Store log with ANSI escape codes
    let raw_line = "\x1b[32mSUCCESS:\x1b[0m Server started on \x1b[1;33mport 8080\x1b[0m";
    storage.store(process_id.clone(), raw_line.to_string()).await.unwrap();
    
    // Retrieve and verify
    let query = LogQuery {
        process_id,
        level: None,
        format: LogFormat::Raw,
        lines: None,
        search: None,
        since: None,
    };
    
    let logs = storage.query(query).await.unwrap();
    assert!(!logs.is_empty());
    
    // Clean line should have ANSI codes stripped
    let entry = &logs[0];
    assert!(!entry.clean_line.contains("\x1b["), 
            "Clean line still contains ANSI codes: {:?}", entry.clean_line);
    assert!(entry.raw_line.contains("\x1b["), 
            "Raw line missing ANSI codes: {:?}", entry.raw_line);
}