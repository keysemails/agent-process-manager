//! WebSocket handlers for real-time log streaming and terminal attachment

use crate::{
    process::{ProcessId, ProcessManager},
    logs::{LogStorage, LogQuery, LogFormat},
    tmux::TmuxManager,
};
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Extension},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
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
    Extension(process_manager): Extension<Arc<ProcessManager>>,
) -> impl IntoResponse {
    let process_id = ProcessId(id.parse().unwrap());
    
    ws.on_upgrade(move |socket| handle_attach(socket, process_id, process_manager))
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
    socket: WebSocket,
    process_id: ProcessId,
    process_manager: Arc<ProcessManager>,
) {
    info!("Terminal attachment requested for process {}", process_id);
    
    // Check if this is a tmux session
    let tmux_session = match process_manager.get_tmux_session(&process_id).await {
        Ok(session) => session,
        Err(e) => {
            error!("Failed to check tmux session: {}", e);
            return;
        }
    };
    
    if let Some(session_name) = tmux_session {
        handle_tmux_attach(socket, session_name).await;
        return;
    }
    
    // Split the WebSocket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // Get the PTY master for the process
    let pty_master = match process_manager.get_pty_master(&process_id).await {
        Ok(Some(master)) => master,
        Ok(None) => {
            error!("Process {} has no PTY", process_id);
            let error_msg = serde_json::json!({
                "type": "error",
                "message": "Process has no PTY attached"
            });
            let _ = ws_sender.send(Message::Text(error_msg.to_string())).await;
            return;
        }
        Err(e) => {
            error!("Failed to get PTY master for process {}: {}", process_id, e);
            let error_msg = serde_json::json!({
                "type": "error",
                "message": format!("Failed to get PTY: {}", e)
            });
            let _ = ws_sender.send(Message::Text(error_msg.to_string())).await;
            return;
        }
    };
    
    // Create channel only for PTY output
    let (output_tx, mut output_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
    
    // Send connection success message
    let connect_msg = serde_json::json!({
        "type": "connected",
        "process_id": process_id.to_string()
    });
    if let Err(e) = ws_sender.send(Message::Text(connect_msg.to_string())).await {
        error!("Failed to send connection message: {}", e);
        return;
    }
    
    // Task 1: Read from PTY and send to WebSocket
    let pty_reader = {
        let master = pty_master.lock().await;
        match master.try_clone_reader() {
            Ok(reader) => reader,
            Err(e) => {
                error!("Failed to clone PTY reader: {}", e);
                let error_msg = serde_json::json!({
                    "type": "error",
                    "message": "Failed to access PTY"
                });
                let _ = ws_sender.send(Message::Text(error_msg.to_string())).await;
                return;
            }
        }
    };
    
    let output_task = tokio::spawn(async move {
        // Bridge sync reader to async channel
        std::thread::spawn(move || {
            use std::io::Read;
            let mut reader = pty_reader;
            let mut buffer = [0u8; 4096];
            
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = buffer[..n].to_vec();
                        if output_tx.blocking_send(data).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading from PTY: {}", e);
                        break;
                    }
                }
            }
        });
    });
    
    // Get a PTY writer for direct writes
    let pty_writer = {
        let master = pty_master.lock().await;
        match master.take_writer() {
            Ok(writer) => writer,
            Err(e) => {
                error!("Failed to take PTY writer: {}", e);
                let error_msg = serde_json::json!({
                    "type": "error",
                    "message": "Failed to access PTY writer"
                });
                let _ = ws_sender.send(Message::Text(error_msg.to_string())).await;
                return;
            }
        }
    };
    let pty_writer = Arc::new(Mutex::new(pty_writer));
    
    // Create a channel to send WebSocket messages
    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<Message>(100);
    
    // Task 2: Forward PTY output to WebSocket as binary frames
    let ws_tx_clone = ws_tx.clone();
    let ws_output_task = tokio::spawn(async move {
        while let Some(data) = output_rx.recv().await {
            // Send raw bytes as binary WebSocket frame
            if ws_tx_clone.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });
    
    // Task to send WebSocket messages
    let ws_send_task = tokio::spawn(async move {
        while let Some(msg) = ws_rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    // Task 3: Handle WebSocket input and write directly to PTY
    let pty_writer_clone = pty_writer.clone();
    let ws_input_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Direct write to PTY for binary data
                    use std::io::Write;
                    let mut writer = pty_writer_clone.lock().await;
                    if let Err(e) = writer.write_all(&data) {
                        error!("Error writing to PTY: {}", e);
                        break;
                    }
                    let _ = writer.flush();
                }
                Ok(Message::Text(text)) => {
                    // Handle control messages (resize, etc)
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        match parsed["type"].as_str() {
                            Some("resize") => {
                                if let (Some(rows), Some(cols)) = (
                                    parsed["rows"].as_u64(),
                                    parsed["cols"].as_u64()
                                ) {
                                    // Resize the PTY
                                    let master = pty_master.lock().await;
                                    let size = portable_pty::PtySize {
                                        rows: rows as u16,
                                        cols: cols as u16,
                                        pixel_width: 0,
                                        pixel_height: 0,
                                    };
                                    if let Err(e) = master.resize(size) {
                                        error!("Failed to resize PTY: {}", e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
        
        // Send disconnect message if still connected
        let disconnect_msg = serde_json::json!({
            "type": "disconnected"
        });
        let _ = ws_tx.send(Message::Text(disconnect_msg.to_string())).await;
    });
    
    // Wait for any task to complete
    tokio::select! {
        _ = output_task => {},
        _ = ws_output_task => {},
        _ = ws_input_task => {},
        _ = ws_send_task => {},
    }
    
    info!("Terminal attachment ended for process {}", process_id);
}

async fn handle_tmux_attach(socket: WebSocket, session_name: String) {
    info!("Handling tmux attachment for session {}", session_name);
    
    let (mut ws_sender, mut ws_receiver) = socket.split();
    
    // Send connection success message
    let connect_msg = serde_json::json!({
        "type": "connected",
        "session": session_name
    });
    if let Err(e) = ws_sender.send(Message::Text(connect_msg.to_string())).await {
        error!("Failed to send connection message: {}", e);
        return;
    }
    
    // For tmux sessions, we'll use a different approach:
    // 1. Capture pane content periodically and send updates
    // 2. Forward input to tmux using send-keys
    
    let session_name_clone = session_name.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(100);
    
    // Task to capture tmux output periodically
    let capture_task = tokio::spawn(async move {
        let mut last_content = String::new();
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            match TmuxManager::capture_pane(&session_name_clone, false) {
                Ok(content) => {
                    if content != last_content {
                        // Send only the new content
                        let new_lines = content.lines()
                            .skip(last_content.lines().count())
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        if !new_lines.is_empty() {
                            let data = format!("{}\n", new_lines);
                            if tx.send(data.as_bytes().to_vec()).await.is_err() {
                                break;
                            }
                        }
                        
                        last_content = content;
                    }
                }
                Err(e) => {
                    error!("Failed to capture tmux pane: {}", e);
                    break;
                }
            }
        }
    });
    
    // Task to send captured output to WebSocket
    let output_task = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_sender.send(Message::Binary(data)).await.is_err() {
                break;
            }
        }
    });
    
    // Task to handle input from WebSocket
    let session_for_input = session_name.clone();
    let input_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Convert binary data to string and send to tmux
                    if let Ok(text) = String::from_utf8(data) {
                        // Send each character to tmux
                        for ch in text.chars() {
                            if let Err(e) = TmuxManager::send_keys(&session_for_input, &ch.to_string()) {
                                error!("Failed to send keys to tmux: {}", e);
                            }
                        }
                    }
                }
                Ok(Message::Text(text)) => {
                    // Handle control messages
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        match parsed["type"].as_str() {
                            Some("resize") => {
                                // WebSocket terminal attachment deferred - see Issue #10
                                // Using tmux directly provides better terminal experience
                                // May revisit based on Vibe integration needs
                            }
                            _ => {}
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });
    
    // Wait for any task to complete
    tokio::select! {
        _ = capture_task => {},
        _ = output_task => {},
        _ = input_task => {},
    }
    
    info!("tmux attachment ended for session {}", session_name);
}