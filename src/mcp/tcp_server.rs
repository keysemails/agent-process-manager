//! TCP/Unix socket server implementation for MCP

use anyhow::Result;
use std::sync::Arc;
use tokio::net::{TcpListener, UnixListener};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{info, error, debug};
use rmcp::service::ServiceExt;

use crate::config::{McpConfig, McpTransport};
use crate::logs::LogStorage;
use crate::process::ProcessManager;

use super::McpServerHandler;

/// Starts the MCP server with TCP or Unix socket transport
pub async fn start_mcp_server(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    mcp_config: McpConfig,
    full_config: crate::config::Config,
) -> Result<()> {
    match mcp_config.transport {
        McpTransport::Tcp => {
            start_tcp_server(process_manager, log_storage, &mcp_config, full_config).await
        }
        McpTransport::UnixSocket => {
            start_unix_socket_server(process_manager, log_storage, &mcp_config, full_config).await
        }
    }
}

async fn start_tcp_server(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    mcp_config: &McpConfig,
    full_config: crate::config::Config,
) -> Result<()> {
    let addr = format!("{}:{}", mcp_config.tcp_host, mcp_config.tcp_port);
    let listener = TcpListener::bind(&addr).await?;
    info!("MCP TCP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer_addr)) => {
                info!("New MCP connection from {}", peer_addr);
                let handler = McpServerHandler {
                    process_manager: process_manager.clone(),
                    log_storage: log_storage.clone(),
                    config: full_config.clone(),
                };
                
                tokio::spawn(async move {
                    debug!("Spawning handler for connection from {}", peer_addr);
                    if let Err(e) = handle_connection(stream, handler).await {
                        error!("MCP connection error from {}: {}", peer_addr, e);
                    }
                    debug!("Connection from {} ended", peer_addr);
                });
            }
            Err(e) => {
                error!("Failed to accept MCP connection: {}", e);
            }
        }
    }
}

async fn start_unix_socket_server(
    process_manager: Arc<ProcessManager>,
    log_storage: Arc<LogStorage>,
    mcp_config: &McpConfig,
    full_config: crate::config::Config,
) -> Result<()> {
    // Remove existing socket file if it exists
    if std::path::Path::new(&mcp_config.unix_socket).exists() {
        std::fs::remove_file(&mcp_config.unix_socket)?;
    }

    let listener = UnixListener::bind(&mcp_config.unix_socket)?;
    info!("MCP Unix socket server listening on {}", mcp_config.unix_socket);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                info!("New MCP Unix socket connection");
                let handler = McpServerHandler {
                    process_manager: process_manager.clone(),
                    log_storage: log_storage.clone(),
                    config: full_config.clone(),
                };
                
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, handler).await {
                        error!("MCP connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept MCP connection: {}", e);
            }
        }
    }
}

async fn handle_connection<T>(stream: T, handler: McpServerHandler) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    debug!("Handling new MCP connection");
    
    // Split the stream into read and write halves
    let (read, write) = tokio::io::split(stream);
    
    // Create a transport adapter for rmcp
    let transport = (read, write);
    
    // Use rmcp's service handling
    let service = handler
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("MCP service initialization error: {:?}", e))?;
    
    // Wait for the service to complete
    service
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("MCP service error: {}", e))?;
    
    Ok(())
}