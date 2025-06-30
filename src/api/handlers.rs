//! API request handlers

use crate::{
    process::{ProcessConfig, ProcessId, ProcessManager, ProcessStatus},
    logs::{LogStorage, LogQuery, LogFormat},
};
use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

pub async fn health_check() -> impl IntoResponse {
    Json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

pub async fn create_process(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Json(config): Json<ProcessConfig>,
) -> impl IntoResponse {
    match process_manager.spawn_process(config).await {
        Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn list_processes(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
) -> impl IntoResponse {
    match process_manager.list_processes().await {
        Ok(processes) => (StatusCode::OK, Json(ApiResponse::success(processes))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<serde_json::Value>>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn get_process(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    match process_manager.get_process(&process_id).await {
        Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn stop_process(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    match process_manager.stop_process(&process_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(ApiResponse::success(serde_json::json!({
                "message": "Process stopped"
            }))),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn restart_process(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    match process_manager.restart_process(&process_id).await {
        Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn get_process_health(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    match process_manager.get_process(&process_id).await {
        Ok(info) => {
            let health_data = serde_json::json!({
                "process_id": info.id,
                "name": info.name,
                "status": info.status,
                "cpu_percent": info.cpu_percent,
                "memory_mb": info.memory_mb,
                "uptime_seconds": info.uptime_seconds,
                "is_healthy": matches!(info.status, crate::process::ProcessStatus::Running),
            });
            (StatusCode::OK, Json(ApiResponse::success(health_data))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
        ).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    format: Option<String>,
    lines: Option<usize>,
    search: Option<String>,
}

pub async fn get_logs(
    Extension(log_storage): Extension<Arc<LogStorage>>,
    Path(id): Path<String>,
    Query(params): Query<LogQueryParams>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    let format = params.format
        .as_deref()
        .and_then(|f| match f {
            "summary" => Some(LogFormat::Summary),
            "errors" => Some(LogFormat::Errors),
            "raw" => Some(LogFormat::Raw),
            _ => None,
        })
        .unwrap_or(LogFormat::Json);

    if matches!(format, LogFormat::Summary) {
        // Return summary instead of raw logs
        match log_storage.get_summary(&process_id).await {
            Ok(summary) => return (StatusCode::OK, Json(ApiResponse::success(summary))).into_response(),
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<serde_json::Value>::error(e.to_string())),
            ).into_response(),
        }
    }

    let query = LogQuery {
        process_id,
        format,
        lines: params.lines,
        search: params.search,
        level: if matches!(format, LogFormat::Errors) {
            Some(crate::logs::LogLevel::Error)
        } else {
            None
        },
        since: None,
    };

    match log_storage.query(query).await {
        Ok(logs) => (StatusCode::OK, Json(ApiResponse::success(logs))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<Vec<serde_json::Value>>::error(e.to_string())),
        ).into_response(),
    }
}

pub async fn get_raw_logs(
    Extension(log_storage): Extension<Arc<LogStorage>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    let query = LogQuery {
        process_id,
        format: LogFormat::Raw,
        lines: Some(1000),
        search: None,
        level: None,
        since: None,
    };

    match log_storage.query(query).await {
        Ok(logs) => {
            let raw_output = logs
                .iter()
                .map(|log| &log.raw_line)
                .cloned()
                .collect::<Vec<_>>()
                .join("");
            
            (StatusCode::OK, raw_output).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentQuery {
    SystemOverview,
    ProcessErrors {
        process_filter: Option<Vec<String>>,
        time_window: Option<String>, // "5m", "1h", etc.
        min_severity: Option<String>,
    },
    PortMapping {
        include_urls: bool,
    },
    PerformanceMetrics {
        process_filter: Option<Vec<String>>,
        metrics: Vec<String>, // ["cpu", "memory"]
    },
    LogSearch {
        pattern: String,
        process_filter: Option<Vec<String>>,
        limit: Option<usize>,
    },
    EventCorrelation {
        event_types: Vec<String>,
        time_window: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    pub summary: String,              // One-line summary
    pub data: serde_json::Value,      // Structured data
    pub metadata: ResponseMetadata,   // Query metadata
}

#[derive(Debug, Serialize)]
pub struct ResponseMetadata {
    pub processes_analyzed: usize,
    pub time_range: Option<String>,
    pub data_points: usize,
    pub query_time_ms: u64,
}

pub async fn agent_query(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Extension(log_storage): Extension<Arc<LogStorage>>,
    Json(query): Json<AgentQuery>,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();
    
    let response = match query {
        AgentQuery::SystemOverview => {
            handle_system_overview(process_manager, log_storage).await
        }
        AgentQuery::ProcessErrors { process_filter, time_window, min_severity } => {
            handle_process_errors(process_manager, log_storage, process_filter, time_window, min_severity).await
        }
        AgentQuery::PortMapping { include_urls } => {
            handle_port_mapping(process_manager, log_storage, include_urls).await
        }
        AgentQuery::PerformanceMetrics { process_filter, metrics } => {
            handle_performance_metrics(process_manager, process_filter, metrics).await
        }
        AgentQuery::LogSearch { pattern, process_filter, limit } => {
            handle_log_search(process_manager, log_storage, pattern, process_filter, limit).await
        }
        AgentQuery::EventCorrelation { event_types, time_window } => {
            handle_event_correlation(process_manager, log_storage, event_types, time_window).await
        }
    };
    
    let mut response = response;
    response.metadata.query_time_ms = start_time.elapsed().as_millis() as u64;

    Json(ApiResponse::success(response))
}

pub async fn agent_summary(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Extension(log_storage): Extension<Arc<LogStorage>>,
) -> impl IntoResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    let mut services = serde_json::Map::new();
    
    for process in processes {
        let summary = log_storage.get_summary(&process.id).await.ok();
        
        services.insert(
            process.name.clone(),
            serde_json::json!({
                "status": format!("{:?}", process.status).to_lowercase(),
                "uptime": format!("{}s", process.uptime_seconds),
                "cpu_percent": process.cpu_percent,
                "memory_mb": process.memory_mb,
                "urls": summary.as_ref().map(|s| &s.detected_urls).unwrap_or(&vec![]),
                "ports": summary.as_ref().map(|s| &s.detected_ports).unwrap_or(&vec![]),
                "last_event": summary.and_then(|s| s.key_events.first().cloned()).unwrap_or_default(),
            }),
        );
    }
    
    let summary = serde_json::json!({
        "summary": format!("{} services running", services.len()),
        "services": services,
        "issues": [], // Would populate with actual issues
    });

    Json(ApiResponse::success(summary))
}

// Handler functions for structured agent queries

async fn handle_system_overview(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    let total_processes = processes.len();
    
    let mut healthy_count = 0;
    let mut error_count = 0;
    let mut total_cpu = 0.0;
    let mut total_memory = 0.0;
    let mut all_ports = Vec::new();
    let mut all_urls = Vec::new();
    
    for process in &processes {
        if let ProcessStatus::Running = process.status {
            healthy_count += 1;
        }
        
        total_cpu += process.cpu_percent.unwrap_or(0.0);
        total_memory += process.memory_mb.unwrap_or(0) as f64;
        
        if let Ok(summary) = log_storage.get_summary(&process.id).await {
            error_count += summary.error_count;
            all_ports.extend(summary.detected_ports);
            all_urls.extend(summary.detected_urls);
        }
    }
    
    // Deduplicate
    all_ports.sort();
    all_ports.dedup();
    all_urls.sort();
    all_urls.dedup();
    
    let data = serde_json::json!({
        "processes": {
            "total": total_processes,
            "healthy": healthy_count,
            "unhealthy": total_processes - healthy_count
        },
        "resources": {
            "total_cpu_percent": total_cpu,
            "total_memory_mb": total_memory
        },
        "networking": {
            "ports_in_use": all_ports,
            "endpoints": all_urls
        },
        "errors": {
            "total_count": error_count
        }
    });
    
    AgentResponse {
        summary: format!("{} processes running, {} healthy, {} total errors", 
                        total_processes, healthy_count, error_count),
        data,
        metadata: ResponseMetadata {
            processes_analyzed: total_processes,
            time_range: None,
            data_points: total_processes,
            query_time_ms: 0, // Will be set by caller
        },
    }
}

async fn handle_process_errors(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    process_filter: Option<Vec<String>>,
    time_window: Option<String>,
    _min_severity: Option<String>,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    
    // Filter processes if requested
    let filtered_processes = if let Some(filter) = &process_filter {
        processes.into_iter()
            .filter(|p| filter.contains(&p.name))
            .collect()
    } else {
        processes
    };
    
    // Parse time window
    let since = time_window.as_ref().and_then(|tw| parse_time_window(tw));
    
    let mut errors_by_process = serde_json::Map::new();
    let mut total_errors = 0;
    let mut error_patterns = std::collections::HashMap::new();
    
    for process in &filtered_processes {
        let query = LogQuery {
            process_id: process.id.clone(),
            format: LogFormat::Errors,
            lines: Some(100),
            search: None,
            level: Some(crate::logs::LogLevel::Error),
            since,
        };
        
        if let Ok(logs) = log_storage.query(query).await {
            let process_errors: Vec<_> = logs.iter()
                .map(|log| {
                    // Count error patterns
                    for pattern in &log.patterns {
                        if let crate::logs::DetectedPattern::Error(msg) = pattern {
                            *error_patterns.entry(msg.clone()).or_insert(0) += 1;
                        }
                    }
                    
                    serde_json::json!({
                        "timestamp": log.timestamp,
                        "message": log.clean_line,
                    })
                })
                .collect();
            
            total_errors += process_errors.len();
            errors_by_process.insert(process.name.clone(), process_errors.into());
        }
    }
    
    // Sort error patterns by frequency
    let mut pattern_list: Vec<_> = error_patterns.into_iter().collect();
    pattern_list.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    
    let data = serde_json::json!({
        "total_errors": total_errors,
        "errors_by_process": errors_by_process,
        "common_patterns": pattern_list.into_iter()
            .take(5)
            .map(|(pattern, count)| serde_json::json!({
                "pattern": pattern,
                "count": count
            }))
            .collect::<Vec<_>>(),
        "time_window": time_window.as_deref().unwrap_or("all_time")
    });
    
    AgentResponse {
        summary: format!("Found {} errors across {} processes", 
                        total_errors, filtered_processes.len()),
        data,
        metadata: ResponseMetadata {
            processes_analyzed: filtered_processes.len(),
            time_range: time_window,
            data_points: total_errors,
            query_time_ms: 0,
        },
    }
}

async fn handle_port_mapping(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    include_urls: bool,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    let mut port_map = serde_json::Map::new();
    let mut url_map = serde_json::Map::new();
    
    for process in &processes {
        if let Ok(summary) = log_storage.get_summary(&process.id).await {
            for port in &summary.detected_ports {
                port_map.insert(
                    port.to_string(),
                    serde_json::json!({
                        "process": process.name,
                        "status": format!("{:?}", process.status).to_lowercase(),
                    })
                );
            }
            
            if include_urls {
                for url in &summary.detected_urls {
                    url_map.insert(
                        url.clone(),
                        serde_json::json!({
                            "process": process.name,
                            "status": format!("{:?}", process.status).to_lowercase(),
                        })
                    );
                }
            }
        }
    }
    
    let data = if include_urls {
        serde_json::json!({
            "ports": port_map,
            "urls": url_map,
        })
    } else {
        serde_json::json!({
            "ports": port_map,
        })
    };
    
    let port_count = port_map.len();
    let url_count = url_map.len();
    
    AgentResponse {
        summary: if include_urls {
            format!("Found {} ports and {} URLs in use", port_count, url_count)
        } else {
            format!("Found {} ports in use", port_count)
        },
        data,
        metadata: ResponseMetadata {
            processes_analyzed: processes.len(),
            time_range: None,
            data_points: port_count + url_count,
            query_time_ms: 0,
        },
    }
}

async fn handle_performance_metrics(
    process_manager: Arc<ProcessManager>,
    process_filter: Option<Vec<String>>,
    metrics: Vec<String>,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    
    // Filter processes if requested
    let filtered_processes = if let Some(filter) = &process_filter {
        processes.into_iter()
            .filter(|p| filter.contains(&p.name))
            .collect()
    } else {
        processes
    };
    
    let include_cpu = metrics.contains(&"cpu".to_string());
    let include_memory = metrics.contains(&"memory".to_string());
    
    let mut metrics_data = serde_json::Map::new();
    let mut high_cpu_processes = Vec::new();
    let mut high_memory_processes = Vec::new();
    
    for process in &filtered_processes {
        let mut process_metrics = serde_json::Map::new();
        
        if include_cpu {
            process_metrics.insert("cpu_percent".to_string(), process.cpu_percent.into());
            if process.cpu_percent.unwrap_or(0.0) > 80.0 {
                high_cpu_processes.push(&process.name);
            }
        }
        
        if include_memory {
            process_metrics.insert("memory_mb".to_string(), process.memory_mb.into());
            if process.memory_mb.unwrap_or(0) > 1000 { // > 1GB
                high_memory_processes.push(&process.name);
            }
        }
        
        process_metrics.insert("uptime_seconds".to_string(), process.uptime_seconds.into());
        metrics_data.insert(process.name.clone(), process_metrics.into());
    }
    
    let data = serde_json::json!({
        "metrics": metrics_data,
        "alerts": {
            "high_cpu": high_cpu_processes,
            "high_memory": high_memory_processes,
        }
    });
    
    AgentResponse {
        summary: format!("Performance metrics for {} processes", filtered_processes.len()),
        data,
        metadata: ResponseMetadata {
            processes_analyzed: filtered_processes.len(),
            time_range: None,
            data_points: filtered_processes.len() * metrics.len(),
            query_time_ms: 0,
        },
    }
}

async fn handle_log_search(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    pattern: String,
    process_filter: Option<Vec<String>>,
    limit: Option<usize>,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    
    // Filter processes if requested
    let filtered_processes = if let Some(filter) = &process_filter {
        processes.into_iter()
            .filter(|p| filter.contains(&p.name))
            .collect()
    } else {
        processes
    };
    
    let mut matches = Vec::new();
    let search_limit = limit.unwrap_or(50);
    
    for process in &filtered_processes {
        let query = LogQuery {
            process_id: process.id.clone(),
            format: LogFormat::Raw,
            lines: Some(1000), // Search through recent logs
            search: Some(pattern.clone()),
            level: None,
            since: None,
        };
        
        if let Ok(logs) = log_storage.query(query).await {
            for log in logs.into_iter().take(search_limit.saturating_sub(matches.len())) {
                matches.push(serde_json::json!({
                    "process": process.name,
                    "timestamp": log.timestamp,
                    "line": log.clean_line,
                }));
                
                if matches.len() >= search_limit {
                    break;
                }
            }
        }
        
        if matches.len() >= search_limit {
            break;
        }
    }
    
    let data = serde_json::json!({
        "pattern": pattern,
        "matches": matches,
        "truncated": matches.len() >= search_limit,
    });
    
    AgentResponse {
        summary: format!("Found {} matches for pattern '{}'", matches.len(), pattern),
        data,
        metadata: ResponseMetadata {
            processes_analyzed: filtered_processes.len(),
            time_range: None,
            data_points: matches.len(),
            query_time_ms: 0,
        },
    }
}

async fn handle_event_correlation(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    event_types: Vec<String>,
    time_window: Option<String>,
) -> AgentResponse {
    let processes = process_manager.list_processes().await.unwrap_or_default();
    let since = time_window.as_ref().and_then(|tw| parse_time_window(tw));
    
    let mut timeline = Vec::new();
    
    for process in &processes {
        if let Ok(summary) = log_storage.get_summary(&process.id).await {
            // Add key events
            if event_types.contains(&"started".to_string()) || event_types.contains(&"key_event".to_string()) {
                for event in &summary.key_events {
                    timeline.push(serde_json::json!({
                        "timestamp": chrono::Utc::now().timestamp(), // Would need actual timestamp
                        "process": process.name,
                        "type": "key_event",
                        "message": event,
                    }));
                }
            }
            
            // Add errors if requested
            if event_types.contains(&"error".to_string()) {
                let query = LogQuery {
                    process_id: process.id.clone(),
                    format: LogFormat::Errors,
                    lines: Some(20),
                    search: None,
                    level: Some(crate::logs::LogLevel::Error),
                    since,
                };
                
                if let Ok(logs) = log_storage.query(query).await {
                    for log in logs {
                        timeline.push(serde_json::json!({
                            "timestamp": log.timestamp,
                            "process": process.name,
                            "type": "error",
                            "message": log.clean_line,
                        }));
                    }
                }
            }
        }
    }
    
    // Sort by timestamp
    timeline.sort_by_key(|event| {
        event.get("timestamp")
            .and_then(|t| t.as_f64())
            .unwrap_or(0.0) as i64
    });
    
    let data = serde_json::json!({
        "timeline": timeline,
        "event_types": event_types,
        "time_window": time_window.as_deref().unwrap_or("all_time"),
    });
    
    AgentResponse {
        summary: format!("Found {} events across {} processes", timeline.len(), processes.len()),
        data,
        metadata: ResponseMetadata {
            processes_analyzed: processes.len(),
            time_range: time_window,
            data_points: timeline.len(),
            query_time_ms: 0,
        },
    }
}

// Helper function to parse time window strings like "5m", "1h", "30s"
fn parse_time_window(window: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let now = chrono::Utc::now();
    
    if let Some(captures) = regex::Regex::new(r"^(\d+)([smhd])$").unwrap().captures(window) {
        let amount: i64 = captures.get(1)?.as_str().parse().ok()?;
        let unit = captures.get(2)?.as_str();
        
        let duration = match unit {
            "s" => chrono::Duration::seconds(amount),
            "m" => chrono::Duration::minutes(amount),
            "h" => chrono::Duration::hours(amount),
            "d" => chrono::Duration::days(amount),
            _ => return None,
        };
        
        Some(now - duration)
    } else {
        None
    }
}

// Helper endpoints for API discovery

pub async fn get_query_schema() -> impl IntoResponse {
    let schema = serde_json::json!({
        "query_types": {
            "system_overview": {
                "description": "Get a high-level overview of all processes",
                "parameters": {}
            },
            "process_errors": {
                "description": "Find and analyze errors across processes",
                "parameters": {
                    "process_filter": {
                        "type": "array",
                        "items": "string",
                        "optional": true,
                        "description": "Filter by specific process names"
                    },
                    "time_window": {
                        "type": "string",
                        "optional": true,
                        "pattern": "^\\d+[smhd]$",
                        "examples": ["5m", "1h", "24h"],
                        "description": "Time window for error search"
                    },
                    "min_severity": {
                        "type": "string",
                        "optional": true,
                        "enum": ["warning", "error", "critical", "fatal"],
                        "description": "Minimum error severity to include"
                    }
                }
            },
            "port_mapping": {
                "description": "Discover which processes are using which ports",
                "parameters": {
                    "include_urls": {
                        "type": "boolean",
                        "description": "Include detected URLs/endpoints"
                    }
                }
            },
            "performance_metrics": {
                "description": "Get CPU and memory usage metrics",
                "parameters": {
                    "process_filter": {
                        "type": "array",
                        "items": "string",
                        "optional": true,
                        "description": "Filter by specific process names"
                    },
                    "metrics": {
                        "type": "array",
                        "items": {
                            "enum": ["cpu", "memory"]
                        },
                        "description": "Which metrics to include"
                    }
                }
            },
            "log_search": {
                "description": "Search logs across processes",
                "parameters": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern/text"
                    },
                    "process_filter": {
                        "type": "array",
                        "items": "string",
                        "optional": true,
                        "description": "Filter by specific process names"
                    },
                    "limit": {
                        "type": "integer",
                        "optional": true,
                        "default": 50,
                        "description": "Maximum results to return"
                    }
                }
            },
            "event_correlation": {
                "description": "Get a timeline of correlated events",
                "parameters": {
                    "event_types": {
                        "type": "array",
                        "items": {
                            "enum": ["started", "stopped", "error", "key_event"]
                        },
                        "description": "Types of events to include"
                    },
                    "time_window": {
                        "type": "string",
                        "optional": true,
                        "pattern": "^\\d+[smhd]$",
                        "description": "Time window for events"
                    }
                }
            }
        }
    });
    
    Json(ApiResponse::success(schema))
}

pub async fn get_capabilities() -> impl IntoResponse {
    let capabilities = serde_json::json!({
        "version": "1.0",
        "features": {
            "structured_queries": true,
            "real_time_streaming": true,
            "pattern_detection": true,
            "performance_monitoring": true,
            "log_aggregation": true,
            "error_correlation": true
        },
        "detected_patterns": [
            "urls",
            "ports",
            "ip_addresses",
            "errors",
            "file_paths",
            "build_times",
            "key_events"
        ],
        "metrics": {
            "cpu_usage": true,
            "memory_usage": true,
            "uptime": true
        },
        "query_features": {
            "time_windows": ["5m", "15m", "30m", "1h", "6h", "12h", "24h", "7d"],
            "log_formats": ["raw", "summary", "errors", "json"],
            "process_filtering": true,
            "pattern_search": true
        }
    });
    
    Json(ApiResponse::success(capabilities))
}