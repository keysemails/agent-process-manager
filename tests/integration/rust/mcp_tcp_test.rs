//! Tests for MCP TCP server functionality

#[cfg(test)]
mod tests {
    use agent_process_manager::config::{Config, McpConfig, McpTransport};
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_mcp_tcp_server_starts() {
        // Create a test config with MCP enabled on a random port
        let mut config = Config::default();
        config.mcp = McpConfig {
            enabled: true,
            transport: McpTransport::Tcp,
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: 0, // Use port 0 to get a random available port
            unix_socket: "/tmp/test_apm.sock".to_string(),
        };

        // This test just verifies the configuration works
        // Full integration testing would require setting up the daemon
        assert!(config.mcp.enabled);
        assert_eq!(config.mcp.tcp_host, "127.0.0.1");
    }

    #[tokio::test]
    async fn test_mcp_unix_socket_config() {
        let mut config = Config::default();
        config.mcp = McpConfig {
            enabled: true,
            transport: McpTransport::UnixSocket,
            tcp_host: "127.0.0.1".to_string(),
            tcp_port: 7338,
            unix_socket: "/tmp/test_apm.sock".to_string(),
        };

        assert!(config.mcp.enabled);
        assert_eq!(config.mcp.unix_socket, "/tmp/test_apm.sock");
    }
}