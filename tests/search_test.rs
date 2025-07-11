//! Integration tests for full-text search functionality

use agent_process_manager::{
    logs::{LogSearchEngine, SearchQuery, LogEntry, LogLevel, DetectedPattern},
    process::ProcessId,
};
use chrono::Utc;
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_basic_search_functionality() {
    // Create temporary directory for index
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path().join("test_index");
    
    // Initialize search engine
    let search_engine = Arc::new(LogSearchEngine::new(&index_path).await.unwrap());
    
    // Create test log entries
    let process_id = ProcessId(uuid::Uuid::new_v4());
    let timestamp = Utc::now();
    
    let test_logs = vec![
        LogEntry {
            id: 1,
            process_id: process_id.clone(),
            timestamp,
            raw_line: "Server started on port 8080".to_string(),
            clean_line: "Server started on port 8080".to_string(),
            patterns: vec![DetectedPattern::Port(8080)],
            level: LogLevel::Info,
        },
        LogEntry {
            id: 2,
            process_id: process_id.clone(),
            timestamp,
            raw_line: "Database connection error: timeout".to_string(),
            clean_line: "Database connection error: timeout".to_string(),
            patterns: vec![DetectedPattern::Error("Database connection error".to_string())],
            level: LogLevel::Error,
        },
        LogEntry {
            id: 3,
            process_id: process_id.clone(),
            timestamp,
            raw_line: "Processing request for user authentication".to_string(),
            clean_line: "Processing request for user authentication".to_string(),
            patterns: vec![],
            level: LogLevel::Info,
        },
    ];
    
    // Index the test logs
    search_engine.index_logs_batch(&test_logs).await.unwrap();
    search_engine.commit().await.unwrap();
    
    // Test basic search
    let query = SearchQuery {
        query: "server".to_string(),
        process_id: None,
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(10),
        offset: None,
        highlight: None,
    };
    
    let results = search_engine.search(query).await.unwrap();
    assert_eq!(results.results.len(), 1);
    assert!(results.results[0].raw_line.contains("Server started"));
    
    // Test error search
    let error_query = SearchQuery {
        query: "error".to_string(),
        process_id: None,
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(10),
        offset: None,
        highlight: None,
    };
    
    let error_results = search_engine.search(error_query).await.unwrap();
    assert_eq!(error_results.results.len(), 1);
    assert!(error_results.results[0].raw_line.contains("Database connection error"));
    
    // Test phrase search
    let phrase_query = SearchQuery {
        query: "user authentication".to_string(),
        process_id: None,
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(10),
        offset: None,
        highlight: None,
    };
    
    let phrase_results = search_engine.search(phrase_query).await.unwrap();
    assert_eq!(phrase_results.results.len(), 1);
    assert!(phrase_results.results[0].raw_line.contains("user authentication"));
    
    // Test no results
    let no_results_query = SearchQuery {
        query: "nonexistent".to_string(),
        process_id: None,
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(10),
        offset: None,
        highlight: None,
    };
    
    let no_results = search_engine.search(no_results_query).await.unwrap();
    assert_eq!(no_results.results.len(), 0);
    
    // Test index stats
    let stats = search_engine.get_stats().await.unwrap();
    assert_eq!(stats.num_docs, 3);
    assert!(stats.index_size_bytes > 0);
}

#[tokio::test]
async fn test_search_with_process_filter() {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path().join("test_index_filter");
    
    let search_engine = Arc::new(LogSearchEngine::new(&index_path).await.unwrap());
    
    let process1 = ProcessId(uuid::Uuid::new_v4());
    let process2 = ProcessId(uuid::Uuid::new_v4());
    let timestamp = Utc::now();
    
    let test_logs = vec![
        LogEntry {
            id: 1,
            process_id: process1.clone(),
            timestamp,
            raw_line: "Process 1 log message".to_string(),
            clean_line: "Process 1 log message".to_string(),
            patterns: vec![],
            level: LogLevel::Info,
        },
        LogEntry {
            id: 2,
            process_id: process2.clone(),
            timestamp,
            raw_line: "Process 2 log message".to_string(),
            clean_line: "Process 2 log message".to_string(),
            patterns: vec![],
            level: LogLevel::Info,
        },
    ];
    
    search_engine.index_logs_batch(&test_logs).await.unwrap();
    search_engine.commit().await.unwrap();
    
    // Search with process filter
    let query = SearchQuery {
        query: "log".to_string(),
        process_id: Some(process1.clone()),
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(10),
        offset: None,
        highlight: None,
    };
    
    let results = search_engine.search(query).await.unwrap();
    assert_eq!(results.results.len(), 1);
    assert_eq!(results.results[0].process_id, process1);
}

#[tokio::test]
async fn test_search_performance() {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path().join("test_index_perf");
    
    let search_engine = Arc::new(LogSearchEngine::new(&index_path).await.unwrap());
    
    // Index many logs
    let process_id = ProcessId(uuid::Uuid::new_v4());
    let mut test_logs = Vec::new();
    
    for i in 0..1000 {
        test_logs.push(LogEntry {
            id: i,
            process_id: process_id.clone(),
            timestamp: Utc::now(),
            raw_line: format!("Log entry number {} with some content", i),
            clean_line: format!("Log entry number {} with some content", i),
            patterns: vec![],
            level: LogLevel::Info,
        });
    }
    
    // Measure indexing time
    let start = std::time::Instant::now();
    search_engine.index_logs_batch(&test_logs).await.unwrap();
    search_engine.commit().await.unwrap();
    let indexing_time = start.elapsed();
    
    println!("Indexed 1000 logs in {:?}", indexing_time);
    assert!(indexing_time.as_millis() < 5000); // Should be fast
    
    // Measure search time
    let query = SearchQuery {
        query: "content".to_string(),
        process_id: None,
        level: None,
        since: None,
        until: None,
        patterns: None,
        limit: Some(50),
        offset: None,
        highlight: None,
    };
    
    let start = std::time::Instant::now();
    let results = search_engine.search(query).await.unwrap();
    let search_time = start.elapsed();
    
    println!("Searched through 1000 logs in {:?}", search_time);
    assert!(search_time.as_millis() < 100); // Should be very fast
    assert!(results.results.len() > 0);
    assert!(results.query_time_ms < 100);
}