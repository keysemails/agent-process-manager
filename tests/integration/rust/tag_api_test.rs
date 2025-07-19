use agent_process_manager::api::create_router;
use agent_process_manager::logs::LogStorage;
use agent_process_manager::process::{ProcessManager, ProcessConfig};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use serial_test::serial;
use std::sync::Arc;
use tower::ServiceExt;
use tokio::sync::mpsc;
use tempfile::TempDir;

async fn setup_test_app() -> (axum::Router, Arc<ProcessManager>, Arc<LogStorage>, TempDir) {
    let (tx, _rx) = mpsc::channel(100);
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.display());
    
    let storage = Arc::new(LogStorage::new(&db_url).await.unwrap());
    let manager = Arc::new(ProcessManager::new(storage.clone(), tx).unwrap());
    
    let app = create_router(manager.clone(), storage.clone(), None);
    
    (app, manager, storage, temp_dir)
}

#[tokio::test]
#[serial]
async fn test_spawn_process_with_tags() {
    let (app, _manager, _storage, _temp_dir) = setup_test_app().await;
    
    let spawn_request = json!({
        "name": "test-tagged-process",
        "command": "echo",
        "args": ["hello"],
        "tags": ["web", "frontend", "test"],
        "pty": false
    });
    
    let response = app.clone()
        .oneshot(Request::builder()
            .method("POST")
            .uri("/api/processes")
            .header("content-type", "application/json")
            .body(Body::from(spawn_request.to_string()))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    let process_info = &json["data"];
    assert_eq!(process_info["name"], "test-tagged-process");
    assert_eq!(process_info["tags"], json!(["web", "frontend", "test"]));
}

#[tokio::test]
#[serial]
async fn test_list_processes_with_tag_filter() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create processes with different tags
    let configs = vec![
        ("web-app", vec!["web", "production"]),
        ("api-server", vec!["api", "production"]),
        ("dev-server", vec!["web", "development"]),
        ("test-runner", vec!["test"]),
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
    
    // Test OR filtering with single tag
    let response = app.clone()
        .oneshot(Request::builder()
            .uri("/api/processes?tags=web")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let processes = json["data"].as_array().unwrap();
    assert_eq!(processes.len(), 2); // web-app and dev-server
    
    // Test OR filtering with multiple tags
    let response = app.clone()
        .oneshot(Request::builder()
            .uri("/api/processes?tags=api,test")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let processes = json["data"].as_array().unwrap();
    assert_eq!(processes.len(), 2); // api-server and test-runner
    
    // Test AND filtering
    let response = app.clone()
        .oneshot(Request::builder()
            .uri("/api/processes?all_tags=web,production")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let processes = json["data"].as_array().unwrap();
    assert_eq!(processes.len(), 1); // Only web-app has both tags
    assert_eq!(processes[0]["name"], "web-app");
}

#[tokio::test]
#[serial]
async fn test_add_tag_to_process() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create a process
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
    let process_id = process_info.id.to_string();
    
    // Add a tag
    let add_tag_request = json!({
        "tag": "new-tag"
    });
    
    let response = app.clone()
        .oneshot(Request::builder()
            .method("POST")
            .uri(&format!("/api/processes/{}/tags", process_id))
            .header("content-type", "application/json")
            .body(Body::from(add_tag_request.to_string()))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify tag was added
    let process_info = manager.get_process(&process_info.id).await.unwrap();
    assert_eq!(process_info.tags.len(), 2);
    assert!(process_info.tags.contains(&"initial".to_string()));
    assert!(process_info.tags.contains(&"new-tag".to_string()));
}

#[tokio::test]
#[serial]
async fn test_remove_tag_from_process() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create a process with tags
    let config = ProcessConfig {
        name: "test-process".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
        pty: false,
        use_tmux: false,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    let process_info = manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.to_string();
    
    // Remove a tag
    let response = app.clone()
        .oneshot(Request::builder()
            .method("DELETE")
            .uri(&format!("/api/processes/{}/tags/tag2", process_id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify tag was removed
    let process_info = manager.get_process(&process_info.id).await.unwrap();
    assert_eq!(process_info.tags.len(), 2);
    assert!(process_info.tags.contains(&"tag1".to_string()));
    assert!(!process_info.tags.contains(&"tag2".to_string()));
    assert!(process_info.tags.contains(&"tag3".to_string()));
}

#[tokio::test]
#[serial]
async fn test_get_process_tags() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create a process with tags
    let config = ProcessConfig {
        name: "test-process".to_string(),
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        cwd: None,
        env: Default::default(),
        tags: vec!["web".to_string(), "frontend".to_string(), "react".to_string()],
        pty: false,
        use_tmux: false,
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group: None,
    };
    let process_info = manager.spawn_process(config).await.unwrap();
    let process_id = process_info.id.to_string();
    
    // Get tags
    let response = app.clone()
        .oneshot(Request::builder()
            .uri(&format!("/api/processes/{}/tags", process_id))
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    let data = &json["data"];
    assert_eq!(data["process_id"], process_id);
    assert_eq!(data["name"], "test-process");
    assert_eq!(data["tags"], json!(["web", "frontend", "react"]));
}

#[tokio::test]
#[serial]
async fn test_get_all_tags() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create processes with various tags
    let configs = vec![
        ("process1", vec!["web", "frontend"]),
        ("process2", vec!["api", "backend"]),
        ("process3", vec!["web", "backend", "api"]),
        ("process4", vec!["database", "backend"]),
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
    
    // Get all tags
    let response = app.clone()
        .oneshot(Request::builder()
            .uri("/api/tags")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["success"].as_bool().unwrap());
    let tags = json["data"]["tags"].as_array().unwrap();
    
    // Should have 5 unique tags in alphabetical order
    assert_eq!(tags.len(), 5);
    assert_eq!(tags, &vec![
        json!("api"), 
        json!("backend"), 
        json!("database"), 
        json!("frontend"), 
        json!("web")
    ]);
}

#[tokio::test]
#[serial]
async fn test_tag_validation_errors() {
    let (app, manager, _storage, _temp_dir) = setup_test_app().await;
    
    // Create a process
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
    let process_id = process_info.id.to_string();
    
    // Test invalid tag (with spaces)
    let add_tag_request = json!({
        "tag": "invalid tag with spaces"
    });
    
    let response = app.clone()
        .oneshot(Request::builder()
            .method("POST")
            .uri(&format!("/api/processes/{}/tags", process_id))
            .header("content-type", "application/json")
            .body(Body::from(add_tag_request.to_string()))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(!json["success"].as_bool().unwrap());
    assert!(json["error"].as_str().unwrap().contains("alphanumeric"));
    
    // Test empty tag
    let add_tag_request = json!({
        "tag": ""
    });
    
    let response = app.clone()
        .oneshot(Request::builder()
            .method("POST")
            .uri(&format!("/api/processes/{}/tags", process_id))
            .header("content-type", "application/json")
            .body(Body::from(add_tag_request.to_string()))
            .unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}