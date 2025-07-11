//! Agent Process Manager CLI and daemon

use agent_process_manager::{
    api, config::Config, logs::LogStorage, process::ProcessManager, mcp::start_mcp_server,
};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Parser)]
#[command(name = "apm")]
#[command(version = "0.1.0")]
#[command(about = "Agent Process Manager - AI-native process management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the APM daemon
    Start {
        /// Configuration file path
        #[arg(short, long)]
        config: Option<String>,
    },
    
    /// Start a new process
    #[command(name = "spawn")]
    StartProcess {
        /// Process name
        name: String,
        /// Command to execute
        command: String,
        /// Command arguments
        args: Vec<String>,
        /// Tags for categorization
        #[arg(short, long)]
        tag: Vec<String>,
        /// Enable PTY
        #[arg(long)]
        pty: bool,
    },
    
    /// List all processes
    List {
        /// Show all processes regardless of working directory
        #[arg(long)]
        all: bool,
    },
    
    /// Show process logs
    Logs {
        /// Process name or ID
        name: String,
        /// Show only errors
        #[arg(long)]
        errors: bool,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Access logs from any directory
        #[arg(long)]
        all: bool,
    },
    
    /// Attach to a process (like tmux attach)
    Attach {
        /// Process name or ID
        name: String,
        /// Read-only mode
        #[arg(long)]
        read_only: bool,
    },
    
    /// Get system status
    Status,
    
    /// Stop a process
    Stop {
        /// Process name or ID
        name: String,
        /// Stop process from any directory
        #[arg(long)]
        all: bool,
    },
    
    /// Restart a process
    Restart {
        /// Process name or ID
        name: String,
        /// Restart process from any directory
        #[arg(long)]
        all: bool,
    },
    
    /// Bridge stdio to MCP TCP server
    McpBridge {
        /// TCP host to connect to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// TCP port to connect to
        #[arg(long, default_value = "7338")]
        port: u16,
    },
}

fn init_default_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agent_process_manager=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config } => {
            // Initialize logging
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "agent_process_manager=info".into()),
                )
                .with(tracing_subscriber::fmt::layer())
                .init();
                
            start_daemon(config).await
        }
        Commands::StartProcess { name, command, args, tag, pty } => {
            init_default_logging();
            start_process_cli(name, command, args, tag, pty).await
        }
        Commands::List { all } => {
            init_default_logging();
            list_processes_cli(all).await
        }
        Commands::Logs { name, errors, follow, all } => {
            init_default_logging();
            show_logs_cli(name, errors, follow, all).await
        }
        Commands::Attach { name, read_only } => {
            init_default_logging();
            attach_to_process_cli(name, read_only).await
        }
        Commands::Status => {
            init_default_logging();
            show_status_cli().await
        }
        Commands::Stop { name, all } => {
            init_default_logging();
            stop_process_cli(name, all).await
        }
        Commands::Restart { name, all } => {
            init_default_logging();
            restart_process_cli(name, all).await
        }
        Commands::McpBridge { host, port } => {
            // Don't initialize logging for bridge mode - we need clean stdio
            run_mcp_bridge(host, port).await
        }
    }
}


async fn start_daemon(config_path: Option<String>) -> anyhow::Result<()> {
    info!("Starting Agent Process Manager daemon...");

    // Load configuration
    let config = if let Some(_path) = config_path {
        // Load from specific file
        todo!("Load config from file")
    } else {
        match Config::load() {
            Ok(loaded_config) => {
                info!("Successfully loaded config - MCP enabled: {}", loaded_config.mcp.enabled);
                loaded_config
            }
            Err(e) => {
                error!("Failed to load config: {}. Using defaults.", e);
                Config::default()
            }
        }
    };

    // Initialize storage
    let log_storage = Arc::new(
        LogStorage::new(&config.storage.database_url).await?
    );

    // Create log channel
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(1000);

    // Initialize process manager
    let process_manager = Arc::new(ProcessManager::new(log_storage.clone(), log_tx));
    
    // Initialize and recover orphaned sessions
    process_manager.initialize().await?;

    // Start log processing task
    let storage = log_storage.clone();
    tokio::spawn(async move {
        while let Some((process_id, line)) = log_rx.recv().await {
            if let Err(e) = storage.store(process_id, line).await {
                error!("Failed to store log: {}", e);
            }
        }
    });

    // Start MCP server if enabled
    let mcp_handle = if config.mcp.enabled {
        info!("Starting MCP server...");
        let mcp_manager = process_manager.clone();
        let mcp_storage = log_storage.clone();
        let mcp_config = config.mcp.clone();
        let full_config = config.clone();
        
        Some(tokio::spawn(async move {
            if let Err(e) = start_mcp_server(mcp_manager, mcp_storage, mcp_config, full_config).await {
                error!("MCP server error: {}", e);
            }
        }))
    } else {
        None
    };

    // Create router
    let app = api::create_router(process_manager, log_storage);

    // Start HTTP server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("APM HTTP API listening on {}", addr);
    info!("Dashboard available at http://{}/dashboard", addr);
    
    // Setup shutdown signal
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received");
    };

    // Run servers with graceful shutdown
    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                error!("HTTP server error: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("Shutting down gracefully...");
        }
    }

    // Cancel MCP server if running
    if let Some(handle) = mcp_handle {
        handle.abort();
    }

    Ok(())
}


async fn start_process_cli(
    name: String,
    command: String,
    args: Vec<String>,
    tags: Vec<String>,
    pty: bool,
) -> anyhow::Result<()> {
    // Connect to daemon via HTTP API
    let client = reqwest::Client::new();
    
    // Check if daemon is running first
    if client
        .get("http://localhost:7337/health")
        .timeout(std::time::Duration::from_secs(1))
        .send()
        .await
        .is_err()
    {
        eprintln!("Error: APM daemon is not running. Start it with 'apm start'");
        std::process::exit(1);
    }
    
    // Get the current working directory for access control
    let access_group = match std::env::current_dir() {
        Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
        Err(e) => {
            eprintln!("Warning: Failed to get current directory: {}", e);
            None
        }
    };
    
    let config = agent_process_manager::process::ProcessConfig {
        name: name.clone(),
        command,
        args,
        cwd: None,
        env: std::collections::HashMap::new(),
        tags,
        pty,
        use_tmux: true, // Use tmux by default
        restart_policy: Default::default(),
        resources: Default::default(),
        access_group,
    };

    let response = client
        .post("http://localhost:7337/api/processes")
        .json(&config)
        .send()
        .await?;

    if response.status().is_success() {
        let info: serde_json::Value = response.json().await?;
        println!("Spawned process '{}' with ID: {}", name, info["data"]["id"]);
    } else {
        eprintln!("Failed to start process: {}", response.text().await?);
    }

    Ok(())
}

async fn list_processes_cli(show_all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Load config to check access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    let response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;

    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        
        // Get current working directory for filtering (unless --all is specified)
        let access_group = if !show_all {
            match std::env::current_dir() {
                Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    eprintln!("Warning: Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        println!("{:<20} {:<10} {:<10} {:<10}", "NAME", "STATUS", "PID", "UPTIME");
        println!("{}", "-".repeat(50));
        
        if let Some(processes) = data["data"].as_array() {
            for process in processes {
                // Filter by access group if not showing all
                if let Some(ref group) = access_group {
                    // Use new check_access function for read operation
                    if !agent_process_manager::utils::check_access(
                        Some(group),
                        process["access_group"].as_str(),
                        false,  // read operation
                        &config.access_control.effective_mode()
                    ) {
                        continue;
                    }
                }
                
                println!(
                    "{:<20} {:<10} {:<10} {:<10}",
                    process["name"].as_str().unwrap_or(""),
                    process["status"].as_str().unwrap_or(""),
                    process["pid"].as_u64().unwrap_or(0),
                    format!("{}s", process["uptime_seconds"].as_u64().unwrap_or(0))
                );
            }
        }
    } else {
        eprintln!("Failed to list processes: {}", response.text().await?);
    }

    Ok(())
}

async fn show_logs_cli(name: String, errors_only: bool, _follow: bool, show_all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Load config to check access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    // Get current working directory for filtering (unless --all is specified)
    let access_group = if !show_all {
        match std::env::current_dir() {
            Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                eprintln!("Warning: Failed to get current directory: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // First, get the process ID from the name
    let processes_response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;
    
    if !processes_response.status().is_success() {
        eprintln!("Failed to list processes: {}", processes_response.text().await?);
        return Ok(());
    }
    
    let processes: serde_json::Value = processes_response.json().await?;
    let process_info = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| {
                let name_match = p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name);
                if !name_match {
                    return false;
                }
                // Check access group if not showing all
                if let Some(ref group) = access_group {
                    // Use new check_access function for read operation
                    agent_process_manager::utils::check_access(
                        Some(group),
                        p["access_group"].as_str(),
                        false,  // read operation
                        &config.access_control.effective_mode()
                    )
                } else {
                    true
                }
            })
    } else {
        None
    };
    
    let Some(process) = process_info else {
        eprintln!("Error: Process '{}' not found or not accessible from this directory", name);
        if !show_all {
            eprintln!("Hint: Use --all flag to access processes from other directories");
        }
        std::process::exit(1);
    };
    
    let id = process["id"].as_str().unwrap();
    
    // Now get the logs
    let url = if errors_only {
        format!("http://localhost:7337/api/logs/{}?format=errors", id)
    } else {
        format!("http://localhost:7337/api/logs/{}/raw", id)
    };
    
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        if errors_only {
            // Parse JSON response for errors format
            let logs: serde_json::Value = response.json().await?;
            if let Some(data) = logs["data"].as_array() {
                for log in data {
                    if let Some(line) = log["clean_line"].as_str() {
                        println!("{}", line);
                    }
                }
            }
        } else {
            // Raw format is plain text
            print!("{}", response.text().await?);
        }
    } else {
        eprintln!("Failed to get logs: {}", response.text().await?);
    }

    Ok(())
}

async fn attach_to_process_cli(name: String, read_only: bool) -> anyhow::Result<()> {
    use std::process::Command;
    
    let client = reqwest::Client::new();
    
    // First, get the process ID from the name
    let processes_response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;
    
    if !processes_response.status().is_success() {
        eprintln!("Failed to list processes: {}", processes_response.text().await?);
        return Ok(());
    }
    
    let processes: serde_json::Value = processes_response.json().await?;
    let process_info = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name))
    } else {
        None
    };
    
    let Some(process) = process_info else {
        eprintln!("Error: Process '{}' not found", name);
        std::process::exit(1);
    };
    
    let id = process["id"].as_str().unwrap();
    let session_name = format!("apm-{}", id);
    
    // Check if tmux session exists
    let check_session = Command::new("tmux")
        .args(&["has-session", "-t", &session_name])
        .output()?;
    
    if !check_session.status.success() {
        eprintln!("Error: Process '{}' is not using tmux or session not found", name);
        std::process::exit(1);
    }
    
    println!("Attaching to process '{}'...", name);
    println!("Use tmux detach key (Ctrl+B, D by default) to detach");
    
    // Use tmux attach command
    let mut tmux_cmd = Command::new("tmux");
    
    if read_only {
        // For read-only mode, we can use a separate tmux client in read-only mode
        // This is a bit tricky with tmux, so for now we'll just warn
        println!("Note: Read-only mode is not fully supported with tmux attachment");
    }
    
    tmux_cmd.args(&["attach-session", "-t", &session_name]);
    
    // Execute tmux attach - this will take over the terminal
    let status = tmux_cmd.status()?;
    
    if !status.success() {
        eprintln!("Failed to attach to tmux session");
        std::process::exit(1);
    }
    
    println!("\r\nDetached from process '{}'", name);
    
    Ok(())
}

async fn show_status_cli() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Try to connect to the daemon
    match client
        .get("http://localhost:7337/health")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(_) => {
            println!("APM daemon is running on port 7337");
            
            // Get additional summary info
            if let Ok(response) = client
                .get("http://localhost:7337/api/agent/summary")
                .send()
                .await
            {
                if response.status().is_success() {
                    if let Ok(data) = response.json::<serde_json::Value>().await {
                        if let Some(summary) = data.get("data") {
                            if let Some(services) = summary.get("services") {
                                if let Some(obj) = services.as_object() {
                                    println!("{} services running", obj.len());
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            println!("APM daemon is not running");
        }
    }

    Ok(())
}

async fn stop_process_cli(name: String, show_all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Load config to check access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    // Get current working directory for filtering (unless --all is specified)
    let access_group = if !show_all {
        match std::env::current_dir() {
            Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                eprintln!("Warning: Failed to get current directory: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // First, get the process ID from the name
    let processes_response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;
    
    if !processes_response.status().is_success() {
        eprintln!("Failed to list processes: {}", processes_response.text().await?);
        return Ok(());
    }
    
    let processes: serde_json::Value = processes_response.json().await?;
    let process_info = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| {
                let name_match = p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name);
                if !name_match {
                    return false;
                }
                // Check access group if not showing all - WRITE operation always checks hierarchy
                if let Some(ref group) = access_group {
                    agent_process_manager::utils::check_access(
                        Some(group),
                        p["access_group"].as_str(),
                        true,   // write operation
                        &config.access_control.effective_mode()
                    )
                } else {
                    true
                }
            })
    } else {
        None
    };
    
    let Some(process) = process_info else {
        eprintln!("Error: Process '{}' not found or not accessible from this directory", name);
        if !show_all {
            eprintln!("Hint: Use --all flag to access processes from other directories");
        }
        std::process::exit(1);
    };
    
    let id = process["id"].as_str().unwrap();
    
    // Now stop the process
    let response = client
        .delete(&format!("http://localhost:7337/api/processes/{}", id))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Stopped process '{}'", name);
    } else {
        eprintln!("Failed to stop process: {}", response.text().await?);
    }

    Ok(())
}

async fn restart_process_cli(name: String, show_all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Load config to check access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    // Get current working directory for filtering (unless --all is specified)
    let access_group = if !show_all {
        match std::env::current_dir() {
            Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                eprintln!("Warning: Failed to get current directory: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // First, get the process ID from the name
    let processes_response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;
    
    if !processes_response.status().is_success() {
        eprintln!("Failed to list processes: {}", processes_response.text().await?);
        return Ok(());
    }
    
    let processes: serde_json::Value = processes_response.json().await?;
    let process_info = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| {
                let name_match = p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name);
                if !name_match {
                    return false;
                }
                // Check access group if not showing all - WRITE operation always checks hierarchy
                if let Some(ref group) = access_group {
                    agent_process_manager::utils::check_access(
                        Some(group),
                        p["access_group"].as_str(),
                        true,   // write operation
                        &config.access_control.effective_mode()
                    )
                } else {
                    true
                }
            })
    } else {
        None
    };
    
    let Some(process) = process_info else {
        eprintln!("Error: Process '{}' not found or not accessible from this directory", name);
        if !show_all {
            eprintln!("Hint: Use --all flag to access processes from other directories");
        }
        std::process::exit(1);
    };
    
    let id = process["id"].as_str().unwrap();
    
    // Now restart the process
    let response = client
        .post(&format!("http://localhost:7337/api/processes/{}/restart", id))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Restarted process '{}'", name);
    } else {
        eprintln!("Failed to restart process: {}", response.text().await?);
    }

    Ok(())
}

async fn run_mcp_bridge(host: String, port: u16) -> anyhow::Result<()> {
    // Try to connect to the MCP TCP server
    let addr = format!("{}:{}", host, port);
    let stream = match TcpStream::connect(&addr).await {
        Ok(stream) => stream,
        Err(e) => {
            eprintln!("Failed to connect to MCP server at {}: {}", addr, e);
            eprintln!("Make sure the APM daemon is running with MCP enabled");
            std::process::exit(1);
        }
    };

    // Split the TCP stream into read and write halves
    let (mut tcp_reader, mut tcp_writer) = stream.into_split();
    
    // Get stdin and stdout
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // Use tokio::select! to handle bidirectional copying
    tokio::select! {
        // Copy stdin -> TCP
        result = async {
            let mut buf = vec![0; 8192];
            loop {
                match stdin.read(&mut buf).await {
                    Ok(0) => break Ok(()), // EOF
                    Ok(n) => {
                        if let Err(e) = tcp_writer.write_all(&buf[..n]).await {
                            break Err(e);
                        }
                    }
                    Err(e) => break Err(e),
                }
            }
        } => {
            if let Err(e) = result {
                eprintln!("Error copying stdin to TCP: {}", e);
            }
        }
        
        // Copy TCP -> stdout
        result = async {
            let mut buf = vec![0; 8192];
            loop {
                match tcp_reader.read(&mut buf).await {
                    Ok(0) => break Ok(()), // EOF
                    Ok(n) => {
                        if let Err(e) = stdout.write_all(&buf[..n]).await {
                            break Err(e);
                        }
                        stdout.flush().await?;
                    }
                    Err(e) => break Err(e),
                }
            }
        } => {
            if let Err(e) = result {
                eprintln!("Error copying TCP to stdout: {}", e);
            }
        }
    }

    Ok(())
}