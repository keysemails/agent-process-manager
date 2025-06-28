//! API request handlers

use crate::{
    process::{ProcessConfig, ProcessId, ProcessManager},
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
pub struct AgentQuery {
    pub question: String,
    pub context: Option<Vec<String>>,
}

pub async fn agent_query(
    Extension(process_manager): Extension<Arc<ProcessManager>>,
    Extension(log_storage): Extension<Arc<LogStorage>>,
    Json(query): Json<AgentQuery>,
) -> impl IntoResponse {
    // This is a simplified implementation
    // In a real system, we'd use NLP or pattern matching
    
    let response = if query.question.to_lowercase().contains("port") {
        // Find all ports in use
        let processes = process_manager.list_processes().await.unwrap_or_default();
        let mut ports = Vec::new();
        
        for process in processes {
            if let Ok(summary) = log_storage.get_summary(&process.id).await {
                ports.extend(summary.detected_ports);
            }
        }
        
        serde_json::json!({
            "answer": format!("Ports in use: {:?}", ports),
            "ports": ports,
        })
    } else if query.question.to_lowercase().contains("error") {
        // Find recent errors
        let processes = process_manager.list_processes().await.unwrap_or_default();
        let mut all_errors = Vec::new();
        
        for process in processes {
            let query = LogQuery {
                process_id: process.id,
                format: LogFormat::Errors,
                lines: Some(10),
                search: None,
                level: Some(crate::logs::LogLevel::Error),
                since: Some(chrono::Utc::now() - chrono::Duration::minutes(5)),
            };
            
            if let Ok(logs) = log_storage.query(query).await {
                all_errors.extend(logs.into_iter().map(|log| log.clean_line));
            }
        }
        
        serde_json::json!({
            "answer": format!("Found {} recent errors", all_errors.len()),
            "errors": all_errors,
        })
    } else {
        serde_json::json!({
            "answer": "I don't understand the question. Try asking about ports or errors.",
        })
    };

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