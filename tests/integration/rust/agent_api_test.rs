//! Tests for the enhanced AI Agent API

use agent_process_manager::{
    process::ProcessManager,
    process::supervisor::{ProcessConfig, RestartPolicy, ResourceLimits},
    logs::LogStorage,
    api,
};
use axum::http::{Request, StatusCode};
use axum::body::Body;
use tower::ServiceExt;
use serde_json::json;
use tempfile::TempDir;
use std::sync::Arc;

async fn setup_test_app() -> (axum::Router, Arc<ProcessManager>, Arc<LogStorage>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());
    
    // Create log channel
    let (tx, mut _rx) = tokio::sync::mpsc::channel(1024);
    
    let log_storage = Arc::new(
        LogStorage::new(&db_url)
            .await
            .unwrap()
    );
    let process_manager = Arc::new(ProcessManager::new(log_storage.clone(), tx));
    
    let app = api::create_router(process_manager.clone(), log_storage.clone(), None);
    
    (app, process_manager, log_storage, temp_dir)
}

#[tokio::test]
async fn test_system_overview_query() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Spawn a test process
    let config = ProcessConfig {
        name: "test-server".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        env: std::collections::HashMap::new(),
        cwd: None,
        tags: vec![],
        pty: false,
        resources: ResourceLimits::default(),
        restart_policy: RestartPolicy::default(),
        use_tmux: false,
        access_group: None,
    };
    
    let _process_id = manager.spawn_process(config).await.unwrap();
    
    // Query system overview
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "system_overview"
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["summary"].as_str().unwrap().contains("1 processes running"));
}

#[tokio::test]
async fn test_port_mapping_query() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "port_mapping",
            "include_urls": true
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["data"]["ports"].is_object());
    assert!(json["data"]["data"]["urls"].is_object());
}

#[tokio::test]
async fn test_performance_metrics_query() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Spawn a test process
    let config = ProcessConfig {
        name: "test-app".to_string(),
        command: "sleep".to_string(),
        args: vec!["1".to_string()],
        env: std::collections::HashMap::new(),
        cwd: None,
        tags: vec![],
        pty: false,
        resources: ResourceLimits::default(),
        restart_policy: RestartPolicy::default(),
        use_tmux: false,
        access_group: None,
    };
    
    let _process_id = manager.spawn_process(config).await.unwrap();
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "performance_metrics",
            "metrics": ["cpu", "memory"]
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["data"]["metrics"].is_object());
}

#[tokio::test]
async fn test_log_search_query() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "log_search",
            "pattern": "error",
            "limit": 10
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["data"]["matches"].is_array());
}

#[tokio::test]
async fn test_process_errors_query_with_time_window() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "process_errors",
            "time_window": "5m"
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert_eq!(json["data"]["metadata"]["time_range"].as_str().unwrap(), "5m");
}

#[tokio::test]
async fn test_event_correlation_query() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "event_correlation",
            "event_types": ["error", "key_event"],
            "time_window": "1h"
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["data"]["timeline"].is_array());
}

#[tokio::test]
async fn test_query_schema_endpoint() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("GET")
        .uri("/api/agent/query-schema")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["query_types"].is_object());
    assert!(json["data"]["query_types"]["system_overview"].is_object());
    assert!(json["data"]["query_types"]["process_errors"].is_object());
}

#[tokio::test]
async fn test_capabilities_endpoint() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("GET")
        .uri("/api/agent/capabilities")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    assert!(json["data"]["features"]["structured_queries"].as_bool().unwrap());
    assert!(json["data"]["detected_patterns"].is_array());
}

#[tokio::test]
async fn test_invalid_query_type() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "invalid_query_type"
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    // Should return bad request for invalid JSON
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test] 
async fn test_response_metadata() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Spawn test processes
    for i in 0..3 {
        let config = ProcessConfig {
            name: format!("test-app-{}", i),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            env: std::collections::HashMap::new(),
            cwd: None,
            tags: vec![],
            pty: false,
            resources: ResourceLimits::default(),
            restart_policy: RestartPolicy::default(),
            use_tmux: false,
        access_group: None,
        };
        let _ = manager.spawn_process(config).await.unwrap();
    }
    
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/query")
        .header("content-type", "application/json")
        .body(Body::from(json!({
            "type": "system_overview"
        }).to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // Check metadata
    assert_eq!(json["data"]["metadata"]["processes_analyzed"].as_u64().unwrap(), 3);
    assert!(json["data"]["metadata"]["query_time_ms"].as_u64().is_some());
}