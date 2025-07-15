use agent_process_manager::config::Config;

fn main() {
    let config = Config::load().unwrap_or_default();
    println!("MCP enabled: {}", config.mcp.enabled);
    println!("MCP transport: {:?}", config.mcp.transport);
    println!("MCP TCP port: {}", config.mcp.tcp_port);
}