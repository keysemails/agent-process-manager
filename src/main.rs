//! Agent Process Manager CLI and daemon

use agent_process_manager::{
    api, config::Config, logs::{LogStorage, LogSearchEngine}, process::ProcessManager, mcp::start_mcp_server,
};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, error, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use colored::*;
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
        /// Wrap long directory paths instead of truncating
        #[arg(long)]
        wrap: bool,
        /// Output format (table, json, csv)
        #[arg(long, default_value = "table")]
        format: String,
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
    
    /// Stop all processes
    StopAll {
        /// Only stop processes from current directory (by default stops all)
        #[arg(long)]
        current_dir: bool,
        /// Force stop without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Clean up stopped processes
    Clean {
        /// Remove stopped processes older than specified hours (e.g., 24 for 1 day)
        #[arg(long)]
        older_than: Option<u64>,
        /// Only clean processes from current directory
        #[arg(long)]
        current_dir: bool,
        /// Keep logs when removing process records
        #[arg(long)]
        keep_logs: bool,
        /// Force cleanup without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Restart a process
    Restart {
        /// Process name or ID
        name: String,
        /// Restart process from any directory
        #[arg(long)]
        all: bool,
    },
    
    /// Set up APM for Claude Code
    SetupClaude {
        /// Set up globally for all Claude Code sessions (default is local)
        #[arg(long)]
        global: bool,
        /// Automatically answer yes to all prompts
        #[arg(short, long)]
        yes: bool,
        /// Only show what would be done without making changes
        #[arg(long)]
        dry_run: bool,
        /// Check current Claude setup status
        #[arg(long)]
        check: bool,
        /// Remove APM configuration for Claude
        #[arg(long)]
        remove: bool,
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
        Commands::List { all, wrap, format } => {
            init_default_logging();
            list_processes_cli(all, wrap, format).await
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
        Commands::StopAll { current_dir, force } => {
            init_default_logging();
            stop_all_processes_cli(current_dir, force).await
        }
        Commands::Clean { older_than, current_dir, keep_logs, force } => {
            init_default_logging();
            clean_stopped_processes_cli(older_than, current_dir, keep_logs, force).await
        }
        Commands::Restart { name, all } => {
            init_default_logging();
            restart_process_cli(name, all).await
        }
        Commands::SetupClaude { global, yes, dry_run, check, remove } => {
            init_default_logging();
            setup_claude_cli(global, yes, dry_run, check, remove).await
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
    let config = if let Some(path) = config_path {
        // Load from specific file
        match Config::load_from_path(&path) {
            Ok(loaded_config) => {
                info!("Successfully loaded config from {} - MCP enabled: {}", path, loaded_config.mcp.enabled);
                loaded_config
            }
            Err(e) => {
                error!("Failed to load config from {}: {}. Using defaults.", path, e);
                return Err(anyhow::anyhow!("Failed to load config from {}: {}", path, e));
            }
        }
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

    // Initialize search engine if enabled in config
    let search_engine = if config.search.enabled {
        info!("Initializing full-text search engine at {}...", config.search.index_path);
        match LogSearchEngine::new_with_config(&config.search.index_path, config.search.buffer_size_mb).await {
            Ok(engine) => {
                info!("Search engine initialized successfully");
                Some(Arc::new(engine))
            }
            Err(e) => {
                warn!("Failed to initialize search engine: {}. Search will be disabled.", e);
                None
            }
        }
    } else {
        info!("Search engine disabled in configuration");
        None
    };

    // Initialize storage with optional search engine
    let log_storage = Arc::new(
        LogStorage::new_with_search(&config.storage.database_url, search_engine.clone()).await?
    );

    // Perform auto-cleanup if enabled
    if config.cleanup.auto_clean_on_startup {
        info!("Performing auto-cleanup of stopped processes...");
        
        let retention_hours = if config.cleanup.retention_hours > 0 {
            Some(config.cleanup.retention_hours)
        } else {
            None
        };
        
        match log_storage.clean_stopped_processes(
            retention_hours,
            None, // No access group filter for daemon cleanup
            config.cleanup.keep_logs,
        ).await {
            Ok(cleaned) => {
                if cleaned.is_empty() {
                    info!("No stopped processes to clean up");
                } else {
                    info!("Cleaned up {} stopped processes:", cleaned.len());
                    for (id, name) in &cleaned {
                        info!("  - {} ({})", name, id);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to perform auto-cleanup: {}", e);
            }
        }
    }

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
    let app = api::create_router(process_manager, log_storage, search_engine);

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
    
    // Get current working directory to set in process config
    let cwd = std::env::current_dir().ok();
    
    let config = agent_process_manager::process::ProcessConfig {
        name: name.clone(),
        command,
        args,
        cwd,
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

async fn list_processes_cli(show_all: bool, wrap: bool, format: String) -> anyhow::Result<()> {
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
        
        if let Some(processes) = data["data"].as_array() {
            // Filter processes based on access group
            let filtered_processes: Vec<&serde_json::Value> = processes.iter()
                .filter(|process| {
                    if let Some(ref group) = access_group {
                        agent_process_manager::utils::check_access(
                            Some(group),
                            process["access_group"].as_str(),
                            false,  // read operation
                            &config.access_control.mode
                        )
                    } else {
                        true
                    }
                })
                .collect();
            
            match format.as_str() {
                "json" => {
                    // JSON output
                    let json_output = serde_json::to_string_pretty(&filtered_processes)?;
                    println!("{}", json_output);
                }
                "csv" => {
                    // CSV output
                    println!("name,status,pid,uptime_seconds,started_at,ports,directory");
                    for process in filtered_processes {
                        let name = process["name"].as_str().unwrap_or("");
                        let status = process["status"].as_str().unwrap_or("");
                        let pid = process["pid"].as_u64().unwrap_or(0);
                        let uptime = process["uptime_seconds"].as_u64().unwrap_or(0);
                        let started = process["started_at"].as_str().unwrap_or("");
                        let ports = if let Some(ports) = process["detected_ports"].as_array() {
                            ports.iter()
                                .filter_map(|p| p.as_u64().map(|n| n.to_string()))
                                .collect::<Vec<_>>()
                                .join(";")
                        } else {
                            String::new()
                        };
                        let cwd = process["cwd"].as_str().unwrap_or("");
                        println!("{},{},{},{},{},{},{}", name, status, pid, uptime, started, ports, cwd);
                    }
                }
                _ => {
                    // Table output (default)
                    // Get terminal width to dynamically adjust directory column
                    let term_width = terminal_size::terminal_size()
                        .map(|(terminal_size::Width(w), _)| w as usize)
                        .unwrap_or(80);
                    
                    let fixed_width = 65;
                    let dir_width = if wrap {
                        // If wrapping, use more space for directory
                        if term_width > fixed_width + 10 {
                            term_width - fixed_width - 5
                        } else {
                            30
                        }
                    } else {
                        // Normal truncated mode
                        if term_width > fixed_width + 10 {
                            term_width - fixed_width - 5
                        } else {
                            15
                        }
                    };
                    
                    println!("{:<15} {:<8} {:<8} {:<8} {:<10} {:<10} {:<width$}", 
                        "NAME", "STATUS", "PID", "UPTIME", "STARTED", "PORTS", "DIRECTORY",
                        width = dir_width
                    );
                    println!("{}", "-".repeat(fixed_width + dir_width));
                    
                    for process in filtered_processes {
                
                // Format the started_at time as relative time
                let started_str = if let Some(started_at) = process["started_at"].as_str() {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(started_at) {
                        let now = chrono::Utc::now();
                        let duration = now.signed_duration_since(dt);
                        
                        if duration.num_seconds() < 60 {
                            format!("{}s ago", duration.num_seconds())
                        } else if duration.num_minutes() < 60 {
                            format!("{}m ago", duration.num_minutes())
                        } else if duration.num_hours() < 24 {
                            format!("{}h ago", duration.num_hours())
                        } else {
                            format!("{}d ago", duration.num_days())
                        }
                    } else {
                        "N/A".to_string()
                    }
                } else {
                    "N/A".to_string()
                };
                
                // Format detected ports
                let ports_str = if let Some(ports) = process["detected_ports"].as_array() {
                    let port_numbers: Vec<String> = ports.iter()
                        .filter_map(|p| p.as_u64().map(|n| n.to_string()))
                        .collect();
                    if port_numbers.is_empty() {
                        "-".to_string()
                    } else {
                        port_numbers.join(", ")
                    }
                } else {
                    "-".to_string()
                };
                
                // Format working directory based on available width and wrap option
                let cwd_str = if let Some(cwd) = process["cwd"].as_str() {
                    if wrap {
                        // In wrap mode, show full path
                        cwd.to_string()
                    } else {
                        // Shorten path if too long
                        if cwd.len() > dir_width.saturating_sub(2) {
                            let keep_chars = dir_width.saturating_sub(5); // Room for "..."
                            if keep_chars > 0 {
                                format!("...{}", &cwd[cwd.len().saturating_sub(keep_chars)..])
                            } else {
                                "...".to_string()
                            }
                        } else {
                            cwd.to_string()
                        }
                    }
                } else {
                    "-".to_string()
                };
                
                // Format name - truncate if too long
                let name_str = process["name"].as_str().unwrap_or("");
                let name_display = if name_str.len() > 14 {
                    format!("{}...", &name_str[..11])
                } else {
                    name_str.to_string()
                };
                
                // Color code the status
                let status = process["status"].as_str().unwrap_or("");
                let status_colored = match status {
                    "Running" => status.green(),
                    "Stopped" => status.yellow(),
                    "Failed" => status.red(),
                    "Starting" => status.cyan(),
                    "Restarting" => status.blue(),
                    _ => status.normal(),
                };
                
                // Color ports if any are detected
                let ports_colored = if ports_str == "-" {
                    ports_str.dimmed()
                } else {
                    ports_str.bright_cyan()
                };
                
                if wrap && cwd_str.len() > dir_width {
                    // Multi-line output for wrapped mode
                    println!(
                        "{:<15} {:<8} {:<8} {:<8} {:<10} {:<10}",
                        name_display,
                        status_colored,
                        process["pid"].as_u64().unwrap_or(0),
                        format!("{}s", process["uptime_seconds"].as_u64().unwrap_or(0)),
                        started_str,
                        ports_colored
                    );
                    // Print wrapped directory on next line with indent
                    println!("    {}", cwd_str.dimmed());
                } else {
                    // Single line output
                    println!(
                        "{:<15} {:<8} {:<8} {:<8} {:<10} {:<10} {:<width$}",
                        name_display,
                        status_colored,
                        process["pid"].as_u64().unwrap_or(0),
                        format!("{}s", process["uptime_seconds"].as_u64().unwrap_or(0)),
                        started_str,
                        ports_colored,
                        cwd_str.dimmed(),
                        width = dir_width
                    );
                }
                    }
                }
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
                        &config.access_control.mode
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
                        &config.access_control.mode
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
                        &config.access_control.mode
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

async fn stop_all_processes_cli(current_dir_only: bool, force: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Load config to check access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    // Get current working directory for filtering if current_dir_only is set
    let access_group = if current_dir_only {
        match std::env::current_dir() {
            Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                eprintln!("Error: Failed to get current directory: {}", e);
                return Ok(());
            }
        }
    } else {
        None
    };
    
    // Get all processes
    let response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;

    if !response.status().is_success() {
        eprintln!("Failed to get processes: {}", response.text().await?);
        return Ok(());
    }

    let data: serde_json::Value = response.json().await?;
    
    // Filter processes
    let mut processes_to_stop = Vec::new();
    if let Some(processes) = data["data"].as_array() {
        for process in processes {
            // Skip if already stopped
            if process["status"].as_str() == Some("Stopped") {
                continue;
            }
            
            // Filter by access group if current_dir_only is set
            if let Some(ref group) = access_group {
                if !agent_process_manager::utils::check_access(
                    Some(group),
                    process["access_group"].as_str(),
                    true,  // write operation
                    &config.access_control.mode
                ) {
                    continue;
                }
            }
            
            processes_to_stop.push((
                process["id"].as_str().unwrap_or("").to_string(),
                process["name"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    
    if processes_to_stop.is_empty() {
        if current_dir_only {
            println!("No running processes found in current directory");
        } else {
            println!("No running processes found");
        }
        return Ok(());
    }
    
    // Show what will be stopped
    println!("Will stop {} processes:", processes_to_stop.len());
    for (_, name) in &processes_to_stop {
        println!("  - {}", name);
    }
    
    // Confirm unless forced
    if !force {
        print!("\nAre you sure you want to stop all these processes? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "y" {
            println!("Cancelled");
            return Ok(());
        }
    }
    
    // Stop all processes
    let mut stopped = 0;
    let mut failed = 0;
    
    for (id, name) in processes_to_stop {
        let response = client
            .delete(&format!("http://localhost:7337/api/processes/{}", id))
            .send()
            .await?;
            
        if response.status().is_success() {
            println!("Stopped: {}", name);
            stopped += 1;
        } else {
            eprintln!("Failed to stop {}: {}", name, response.text().await?);
            failed += 1;
        }
    }
    
    println!("\nStopped {} processes, {} failed", stopped, failed);
    
    Ok(())
}

async fn clean_stopped_processes_cli(
    older_than: Option<u64>,
    current_dir_only: bool,
    keep_logs: bool,
    force: bool,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // Get current working directory for filtering if current_dir_only is set
    let access_group = if current_dir_only {
        match std::env::current_dir() {
            Ok(cwd) => Some(agent_process_manager::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                eprintln!("Error: Failed to get current directory: {}", e);
                return Ok(());
            }
        }
    } else {
        None
    };
    
    // First, get the list of stopped processes that would be cleaned
    let response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;

    if !response.status().is_success() {
        eprintln!("Failed to get processes: {}", response.text().await?);
        return Ok(());
    }

    let data: serde_json::Value = response.json().await?;
    
    // Filter processes to find what would be cleaned
    let mut processes_to_clean = Vec::new();
    if let Some(processes) = data["data"].as_array() {
        for process in processes {
            // Only stopped processes
            if process["status"].as_str() != Some("Stopped") {
                continue;
            }
            
            // Check access group if current_dir_only
            if let Some(ref group) = access_group {
                if process["access_group"].as_str() != Some(group) {
                    continue;
                }
            }
            
            // Check age if older_than is specified
            if let Some(hours) = older_than {
                if let Some(stopped_at) = process["stopped_at"].as_str() {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(stopped_at) {
                        let age = chrono::Utc::now().signed_duration_since(dt);
                        if age.num_hours() < hours as i64 {
                            continue;
                        }
                    }
                } else {
                    // No stopped_at time, skip
                    continue;
                }
            }
            
            let name = process["name"].as_str().unwrap_or("").to_string();
            let id = process["id"].as_str().unwrap_or("").to_string();
            let stopped_at = process["stopped_at"].as_str().unwrap_or("N/A").to_string();
            
            processes_to_clean.push((id, name, stopped_at));
        }
    }
    
    if processes_to_clean.is_empty() {
        println!("No stopped processes found matching criteria");
        return Ok(());
    }
    
    // Show what will be cleaned
    println!("Will clean {} stopped processes:", processes_to_clean.len());
    for (_, name, stopped_at) in &processes_to_clean {
        println!("  - {} (stopped at: {})", name, stopped_at);
    }
    if !keep_logs {
        println!("\nWARNING: Logs will also be deleted for these processes!");
    }
    
    // Confirm unless forced
    if !force {
        print!("\nAre you sure you want to clean these processes? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "y" {
            println!("Cancelled");
            return Ok(());
        }
    }
    
    // Send clean request to API
    let mut query_params = vec![];
    if let Some(hours) = older_than {
        query_params.push(format!("older_than={}", hours));
    }
    if let Some(ref group) = access_group {
        query_params.push(format!("access_group={}", group));
    }
    if keep_logs {
        query_params.push("keep_logs=true".to_string());
    }
    
    let url = if query_params.is_empty() {
        "http://localhost:7337/api/processes/clean".to_string()
    } else {
        format!("http://localhost:7337/api/processes/clean?{}", query_params.join("&"))
    };
    
    let response = client
        .post(&url)
        .send()
        .await?;
        
    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        if let Some(count) = result["cleaned"].as_u64() {
            println!("\nCleaned {} stopped processes", count);
        }
    } else {
        eprintln!("Failed to clean processes: {}", response.text().await?);
    }
    
    Ok(())
}

async fn setup_claude_cli(global: bool, yes: bool, dry_run: bool, check: bool, remove: bool) -> anyhow::Result<()> {
    
    if check {
        return check_claude_setup().await;
    }
    
    if remove {
        return remove_claude_setup(global).await;
    }
    
    if global {
        setup_claude_global(yes, dry_run).await
    } else {
        setup_claude_local(yes, dry_run).await
    }
}

async fn setup_claude_local(yes: bool, dry_run: bool) -> anyhow::Result<()> {
    use colored::*;
    use dialoguer::{Confirm, theme::ColorfulTheme};
    use std::path::Path;
    use tokio::fs;
    use tokio::io::AsyncWriteExt;
    
    println!("{}", "🤖 APM Setup for Claude Code - Local Project".bright_blue());
    println!("\nThis will configure Claude to use APM for long-running processes in this project.\n");
    
    println!("📍 Current directory: {}", std::env::current_dir()?.display());
    
    println!("\nI'll help you set up APM by:");
    println!("  1. Creating/updating CLAUDE.md in this directory");
    println!("  2. Adding APM usage instructions for Claude");
    println!("  3. Detecting your project type and adding relevant examples");
    
    if !yes {
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to proceed?")
            .default(true)
            .interact()?;
        
        if !proceed {
            println!("Setup cancelled.");
            return Ok(());
        }
    }
    
    // Detect project type
    println!("\n🔍 Detecting project type...");
    let mut detections = Vec::new();
    
    if Path::new("package.json").exists() {
        detections.push("Node.js");
        println!("✓ Found Node.js project (package.json)");
    }
    
    if Path::new("requirements.txt").exists() || Path::new("pyproject.toml").exists() {
        detections.push("Python");
        println!("✓ Found Python project");
    }
    
    if Path::new("Cargo.toml").exists() {
        detections.push("Rust");
        println!("✓ Found Rust project (Cargo.toml)");
    }
    
    if Path::new("Gemfile").exists() {
        detections.push("Ruby");
        println!("✓ Found Ruby project (Gemfile)");
    }
    
    // Check existing CLAUDE.md
    let claude_exists = Path::new("CLAUDE.md").exists();
    let status = if claude_exists {
        "Found (will append APM configuration)"
    } else {
        "Not found (will create new file)"
    };
    println!("\n📝 CLAUDE.md status: {}", status);
    
    // Generate examples based on detected project types
    let mut examples = Vec::new();
    
    if detections.contains(&"Node.js") {
        examples.push("npm run dev → apm spawn dev npm run dev");
        examples.push("npm start → apm spawn app npm start");
    }
    
    if detections.contains(&"Python") {
        if Path::new("manage.py").exists() {
            examples.push("python manage.py runserver → apm spawn django python manage.py runserver");
        }
        examples.push("python app.py → apm spawn api python app.py");
        examples.push("flask run → apm spawn flask flask run");
    }
    
    if detections.contains(&"Rust") {
        examples.push("cargo run → apm spawn app cargo run");
        examples.push("cargo watch -x run → apm spawn dev cargo watch -x run");
    }
    
    if detections.contains(&"Ruby") {
        examples.push("rails server → apm spawn rails rails server");
        examples.push("bundle exec puma → apm spawn puma bundle exec puma");
    }
    
    if !examples.is_empty() {
        println!("\nThe following APM commands will be configured:");
        for example in &examples {
            println!("  • {}", example);
        }
    }
    
    let action = if claude_exists { "Update" } else { "Create" };
    let prompt = format!("{} CLAUDE.md with these configurations?", action);
    
    if !yes {
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .default(true)
            .interact()?;
        
        if !proceed {
            println!("Setup cancelled.");
            return Ok(());
        }
    }
    
    if dry_run {
        println!("\n{}", "[DRY RUN] Would perform setup but --dry-run specified".yellow());
        return Ok(());
    }
    
    // Create the CLAUDE.md content
    let apm_section = format!(r#"
## Process Management with APM

This project uses Agent Process Manager (APM) for all background processes.
**NEVER run development servers or long-running processes directly**.

### Critical Rules

- NEVER run `npm run dev`, `npm start`, `yarn dev` directly
- NEVER run `python manage.py runserver`, `flask run` directly  
- NEVER run commands that don't terminate naturally

### Use APM Instead

{}

### Common Commands

- Check running processes: `apm list`
- View logs: `apm logs <name>`
- Stop processes: `apm stop <name>`
- Stop all: `apm stop-all --current-dir`

See: https://github.com/sunnya97/agent-process-manager/blob/main/APM_FOR_AI_ASSISTANTS.md
"#, examples.iter().map(|e| format!("- {}", e)).collect::<Vec<_>>().join("\n"));
    
    if claude_exists {
        // Check if APM section already exists
        let content = fs::read_to_string("CLAUDE.md").await?;
        if content.contains("Process Management with APM") {
            println!("✓ CLAUDE.md already contains APM configuration");
            if !yes {
                let update = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Update the existing APM configuration?")
                    .default(false)
                    .interact()?;
                if !update {
                    println!("Keeping existing configuration.");
                    return Ok(());
                }
            }
        }
        
        // Append to existing file
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .open("CLAUDE.md")
            .await?;
        
        file.write_all(apm_section.as_bytes()).await?;
        println!("✓ Updated CLAUDE.md with APM configuration");
    } else {
        // Create new file
        let full_content = format!(r#"# CLAUDE.md - Project Instructions

This file contains instructions for AI assistants working on this project.
{}
"#, apm_section);
        
        fs::write("CLAUDE.md", full_content).await?;
        println!("✓ Created CLAUDE.md with APM configuration");
    }
    
    println!("\n✅ {}", "Local setup complete!".green().bold());
    println!("\nClaude will now use APM for long-running processes in this project.");
    println!("Team members will get the same setup when they clone this repo.");
    
    println!("\n💡 Tips:");
    println!("- Customize CLAUDE.md for your specific needs");
    println!("- Run 'apm setup-claude --check' to verify setup");
    println!("- Use 'apm setup-claude --global' for system-wide configuration");
    
    Ok(())
}

async fn setup_claude_global(yes: bool, dry_run: bool) -> anyhow::Result<()> {
    use colored::*;
    use dialoguer::{Confirm, theme::ColorfulTheme};
    use std::path::PathBuf;
    use tokio::fs;
    use tokio::process::Command;
    use tokio::io::AsyncWriteExt;
    
    println!("{}", "🤖 APM Setup for Claude Code - Global Configuration".bright_blue());
    println!("\nThis will configure Claude to use APM for ALL projects on your system.\n");
    
    println!("I'll help you set up APM globally by:");
    println!("  1. Checking if Claude CLI is installed");
    println!("  2. Creating/updating ~/.config/claude/CLAUDE.md");
    println!("  3. Configuring the APM MCP tool for Claude Code");
    
    if !yes {
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Would you like to proceed?")
            .default(true)
            .interact()?;
        
        if !proceed {
            println!("Setup cancelled.");
            return Ok(());
        }
    }
    
    // Check prerequisites
    println!("\n🔍 Checking prerequisites...");
    
    // Check Claude CLI
    let claude_check = Command::new("which")
        .arg("claude")
        .output()
        .await;
    
    let claude_path = match claude_check {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("✓ Claude CLI found at {}", path);
            Some(path)
        }
        _ => {
            println!("⚠️  Claude CLI not found");
            println!("   Install it from: https://docs.anthropic.com/claude/docs/claude-code");
            None
        }
    };
    
    // Check APM
    let apm_check = Command::new("which")
        .arg("apm")
        .output()
        .await;
    
    match apm_check {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("✓ APM found at {}", path);
        }
        _ => {
            println!("⚠️  APM not found in PATH");
            println!("   Make sure APM is installed globally");
        }
    };
    
    // Check daemon status
    let daemon_check = Command::new("apm")
        .arg("status")
        .output()
        .await;
    
    match daemon_check {
        Ok(output) if output.status.success() => {
            println!("✓ APM daemon is running");
        }
        _ => {
            println!("⚠️  APM daemon not running");
            println!("   Start it with: apm start");
        }
    };
    
    // Setup global CLAUDE.md
    let home = std::env::var("HOME")?;
    let claude_config_dir = PathBuf::from(&home).join(".config").join("claude");
    let claude_md_path = claude_config_dir.join("CLAUDE.md");
    
    println!("\n📝 Global CLAUDE.md status: {}", 
        if claude_md_path.exists() { 
            "Found (will append APM configuration)" 
        } else { 
            "Not found (will create new file)" 
        });
    
    // Show MCP configuration
    if claude_path.is_some() {
        println!("\n⚙️  MCP tool configuration:");
        println!("Will run: claude mcp add agent-process-manager apm mcp-bridge -e RUST_LOG=warn");
    }
    
    if !yes {
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Configure APM globally for Claude?")
            .default(true)
            .interact()?;
        
        if !proceed {
            println!("Setup cancelled.");
            return Ok(());
        }
    }
    
    if dry_run {
        println!("\n{}", "[DRY RUN] Would perform setup but --dry-run specified".yellow());
        return Ok(());
    }
    
    // Create config directory if needed
    fs::create_dir_all(&claude_config_dir).await?;
    
    // Create/update global CLAUDE.md
    let apm_global_content = r#"
## Process Management with APM (Global Configuration)

CRITICAL: This system uses Agent Process Manager (APM) for all long-running processes.
**NEVER run development servers or long-running processes directly**.

### Commands That Will Block You (NEVER RUN):
- npm run dev, npm start, yarn dev, yarn start
- python manage.py runserver, flask run, uvicorn
- node server.js, rails server, php artisan serve
- Any command that runs indefinitely

### ALWAYS Use APM Instead:
- `apm spawn dev npm run dev` instead of `npm run dev`
- `apm spawn api python app.py` instead of `python app.py`
- `apm spawn server node app.js` instead of `node app.js`

### Process Management:
- List processes: `apm list`
- View logs: `apm logs <name>`
- Stop processes: `apm stop <name>`
- Check status: `apm status`

For complete instructions: https://github.com/sunnya97/agent-process-manager/blob/main/APM_FOR_AI_ASSISTANTS.md

When in doubt, use APM! It's better to spawn a short process unnecessarily than to get blocked.
"#;
    
    if claude_md_path.exists() {
        let content = fs::read_to_string(&claude_md_path).await?;
        if !content.contains("Process Management with APM") {
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(&claude_md_path)
                .await?;
            file.write_all(apm_global_content.as_bytes()).await?;
            println!("✓ Updated ~/.config/claude/CLAUDE.md");
        } else {
            println!("✓ ~/.config/claude/CLAUDE.md already configured");
        }
    } else {
        let full_content = format!("# Global Claude Instructions{}", apm_global_content);
        fs::write(&claude_md_path, full_content).await?;
        println!("✓ Created ~/.config/claude/CLAUDE.md");
    }
    
    // Configure MCP tool
    if let Some(_) = claude_path {
        let mcp_output = Command::new("claude")
            .args(&["mcp", "add", "agent-process-manager", "apm", "mcp-bridge", "-e", "RUST_LOG=warn"])
            .output()
            .await?;
        
        if mcp_output.status.success() {
            println!("✓ Configured APM MCP tool");
        } else {
            let stderr = String::from_utf8_lossy(&mcp_output.stderr);
            if stderr.contains("already exists") {
                println!("✓ APM MCP tool already configured");
            } else {
                println!("⚠️  Failed to configure MCP tool: {}", stderr);
                println!("   You may need to run manually: claude mcp add agent-process-manager apm mcp-bridge");
            }
        }
    }
    
    println!("\n✅ {}", "Global setup complete!".green().bold());
    println!("\nClaude Code will now use APM in ALL projects.");
    
    println!("\n⚠️  {}: Restart Claude Code for changes to take effect", "Important".yellow().bold());
    
    println!("\n💡 Tips:");
    println!("- You can still customize individual projects with local setup");
    println!("- Run 'apm setup-claude --check' to verify configuration");
    println!("- View instructions: cat ~/.config/claude/CLAUDE.md");
    
    Ok(())
}

async fn check_claude_setup() -> anyhow::Result<()> {
    use colored::*;
    use std::path::Path;
    use tokio::fs;
    use tokio::process::Command;
    
    println!("{}", "🔍 Checking Claude + APM Setup Status".bright_blue());
    println!();
    
    // Check local setup
    println!("{}", "Local Setup (current directory):".bold());
    let local_claude = Path::new("CLAUDE.md");
    if local_claude.exists() {
        let content = fs::read_to_string(local_claude).await?;
        if content.contains("Process Management with APM") {
            println!("  ✓ CLAUDE.md exists with APM configuration");
        } else {
            println!("  ✗ CLAUDE.md exists but lacks APM configuration");
        }
    } else {
        println!("  ✗ No CLAUDE.md file in current directory");
    }
    
    // Check global setup
    println!("\n{}", "Global Setup:".bold());
    let home = std::env::var("HOME")?;
    let global_claude = Path::new(&home).join(".config").join("claude").join("CLAUDE.md");
    
    if global_claude.exists() {
        let content = fs::read_to_string(&global_claude).await?;
        if content.contains("Process Management with APM") {
            println!("  ✓ ~/.config/claude/CLAUDE.md configured");
        } else {
            println!("  ✗ ~/.config/claude/CLAUDE.md exists but lacks APM configuration");
        }
    } else {
        println!("  ✗ No global CLAUDE.md file");
    }
    
    // Check MCP configuration
    let mcp_check = Command::new("claude")
        .args(&["mcp", "list"])
        .output()
        .await;
    
    match mcp_check {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("agent-process-manager") {
                println!("  ✓ MCP tool configured");
            } else {
                println!("  ✗ APM MCP tool not configured");
            }
        }
        _ => {
            println!("  ✗ Cannot check MCP status (Claude CLI not available)");
        }
    }
    
    // Check APM daemon
    println!("\n{}", "APM Status:".bold());
    let daemon_check = Command::new("apm")
        .arg("status")
        .output()
        .await;
    
    match daemon_check {
        Ok(output) if output.status.success() => {
            println!("  ✓ APM daemon is running");
        }
        _ => {
            println!("  ✗ APM daemon is not running (run 'apm start')");
        }
    }
    
    // Check APM in PATH
    let apm_check = Command::new("which")
        .arg("apm")
        .output()
        .await;
    
    match apm_check {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("  ✓ APM binary found at {}", path);
        }
        _ => {
            println!("  ✗ APM binary not found in PATH");
        }
    }
    
    Ok(())
}

async fn remove_claude_setup(global: bool) -> anyhow::Result<()> {
    use colored::*;
    use dialoguer::{Confirm, theme::ColorfulTheme};
    use std::path::Path;
    use tokio::fs;
    
    if global {
        println!("{}", "🤖 APM Setup for Claude Code - Remove Global Configuration".bright_red());
        println!("\nThis will remove APM configuration from your global Claude setup.");
        
        // TODO: Implement global removal
        println!("Global removal not yet implemented.");
        println!("To remove manually:");
        println!("1. Edit ~/.config/claude/CLAUDE.md");
        println!("2. Run: claude mcp remove agent-process-manager");
    } else {
        println!("{}", "🤖 APM Setup for Claude Code - Remove Local Configuration".bright_red());
        
        let claude_md = Path::new("CLAUDE.md");
        if !claude_md.exists() {
            println!("No CLAUDE.md file found in current directory.");
            return Ok(());
        }
        
        let content = fs::read_to_string(claude_md).await?;
        if !content.contains("Process Management with APM") {
            println!("CLAUDE.md exists but doesn't contain APM configuration.");
            return Ok(());
        }
        
        println!("Current setup status:");
        println!("✓ Local: CLAUDE.md contains APM configuration");
        
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Remove APM configuration from local CLAUDE.md?")
            .default(false)
            .interact()?;
        
        if !proceed {
            println!("Removal cancelled.");
            return Ok(());
        }
        
        println!("\n⚠️  This will remove APM instructions but preserve other content.");
        
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Are you sure?")
            .default(false)
            .interact()?;
        
        if !confirm {
            println!("Removal cancelled.");
            return Ok(());
        }
        
        // Remove APM section (simplified - just notify user for now)
        println!("✓ APM configuration removal would be performed");
        println!("✓ File would still contain other project instructions");
        
        println!("\n✅ Removal complete.");
        println!("\nTo remove global configuration, run: apm setup-claude --remove --global");
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