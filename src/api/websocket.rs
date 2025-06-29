//! WebSocket handlers for real-time log streaming and terminal attachment

use crate::{
    process::ProcessId,
    logs::{LogStorage, LogQuery, LogFormat},
};
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Extension},
    response::IntoResponse,
};
// Note: Axum's WebSocket has its own send method, doesn't need SinkExt
use std::sync::Arc;
use tracing::{error, info};

pub async fn stream_logs(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
    Extension(log_storage): Extension<Arc<LogStorage>>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    ws.on_upgrade(move |socket| handle_log_stream(socket, process_id, log_storage))
}

pub async fn attach_to_process(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    ws.on_upgrade(move |socket| handle_attach(socket, process_id))
}

async fn handle_log_stream(
    mut socket: WebSocket,
    process_id: ProcessId,
    log_storage: Arc<LogStorage>,
) {
    info!("WebSocket client connected for process {}", process_id);
    
    // First, verify the process exists
    let test_query = LogQuery {
        process_id: process_id.clone(),
        format: LogFormat::Raw,
        lines: Some(1),
        search: None,
        level: None,
        since: None,
    };
    
    match log_storage.query(test_query).await {
        Ok(_) => info!("Process {} found in log storage", process_id),
        Err(e) => {
            error!("Process {} not found or error querying: {}", process_id, e);
            let error_msg = serde_json::json!({
                "type": "error",
                "message": format!("Process not found or error: {}", e)
            });
            let _ = socket.send(Message::Text(error_msg.to_string())).await;
            let _ = socket.close().await;
            return;
        }
    }
    
    // Send initial connection message
    let connect_msg = serde_json::json!({
        "type": "connected",
        "process_id": process_id.to_string()
    });
    
    if let Err(e) = socket.send(Message::Text(connect_msg.to_string())).await {
        error!("Failed to send connection message: {}", e);
        return;
    }
    
    let mut last_id = 0i64;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Query for new logs
                let query = LogQuery {
                    process_id: process_id.clone(),
                    format: LogFormat::Raw,
                    lines: Some(100),
                    search: None,
                    level: None,
                    since: None,
                };
                
                match log_storage.query(query).await {
                    Ok(logs) => {
                        info!("Queried logs, found {} entries", logs.len());
                        let current_last_id = last_id;
                        let new_logs: Vec<_> = logs.iter().filter(|l| l.id > current_last_id).collect();
                        info!("Found {} new logs to send", new_logs.len());
                        
                        for log in new_logs {
                            let log_msg = serde_json::json!({
                                "type": "log",
                                "id": log.id,
                                "timestamp": log.timestamp,
                                "line": log.raw_line,
                                "level": format!("{:?}", log.level),
                                "patterns": log.patterns
                            });
                            
                            match socket.send(Message::Text(log_msg.to_string())).await {
                                Ok(_) => {
                                    info!("Sent log {} to client", log.id);
                                    last_id = log.id;
                                }
                                Err(e) => {
                                    error!("Failed to send log to client: {}", e);
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to query logs: {}", e);
                    }
                }
            }
            
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Could implement filter commands here
                        info!("Received message: {}", text);
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        info!("Client disconnected");
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
        }
    }
    
    info!("WebSocket client disconnected for process {}", process_id);
}

async fn handle_attach(
    _socket: WebSocket,
    _process_id: ProcessId,
) {
    // TODO: Implement terminal attachment
    // This would:
    // 1. Get the PTY master for the process
    // 2. Bridge input/output between WebSocket and PTY
    // 3. Handle resize events
    // 4. Handle detach commands
}