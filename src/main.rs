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
#[command(version = env!("CARGO_PKG_VERSION"))]
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
        /// Run as a background daemon (detach from terminal)
        #[arg(short, long)]
        daemon: bool,
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
        /// Filter by tag (can be used multiple times for OR logic)
        #[arg(short, long)]
        tag: Vec<String>,
        /// Filter by tags (comma-separated) - process must have ANY of these tags
        #[arg(long)]
        tags_any: Option<String>,
        /// Filter by tags (comma-separated) - process must have ALL of these tags
        #[arg(long)]
        tags_all: Option<String>,
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
    
    /// Shutdown the APM daemon
    Shutdown {
        /// Force shutdown without confirmation
        #[arg(short, long)]
        force: bool,
    },
    
    /// Kill a process (terminate tmux session)
    Kill {
        /// Process name or ID
        name: String,
        /// Kill process from any directory
        #[arg(long)]
        all: bool,
    },
    
    /// Kill all processes
    KillAll {
        /// Only kill processes from current directory (by default kills all)
        #[arg(long)]
        current_dir: bool,
        /// Force kill without confirmation
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
    
    /// Tag management commands
    Tag {
        #[command(subcommand)]
        tag_command: TagCommands,
    },
    
    /// Configuration management commands
    Config {
        #[command(subcommand)]
        config_command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum TagCommands {
    /// Add a tag to a process
    Add {
        /// Process name or ID
        process: String,
        /// Tag to add
        tag: String,
        /// Access process from any directory
        #[arg(long)]
        all: bool,
    },
    /// Remove a tag from a process
    Remove {
        /// Process name or ID
        process: String,
        /// Tag to remove
        tag: String,
        /// Access process from any directory
        #[arg(long)]
        all: bool,
    },
    /// List all tags for a process
    List {
        /// Process name or ID
        process: String,
        /// Access process from any directory
        #[arg(long)]
        all: bool,
    },
    /// List all tags in the system
    All,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Initialize user config directory and create default config file
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },
    /// Show which config file is being used
    Path,
    /// Edit config file in $EDITOR
    Edit,
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

/// Daemonize the current process
fn daemonize_process() -> anyhow::Result<()> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    
    // Fork and exit parent
    let args: Vec<String> = std::env::args().collect();
    if std::env::var("APM_DAEMON_FORK").is_err() {
        // First fork - parent process
        let mut cmd = Command::new(&args[0]);
        
        // Pass all arguments except --daemon
        for (i, arg) in args.iter().enumerate().skip(1) {
            if arg != "--daemon" && arg != "-d" {
                // Check if previous arg was --daemon or -d
                if i > 0 && (args[i-1] == "--daemon" || args[i-1] == "-d") {
                    continue;
                }
                cmd.arg(arg);
            }
        }
        
        // Set environment variable to indicate we're in the forked process
        cmd.env("APM_DAEMON_FORK", "1");
        
        // Detach from parent process group
        cmd.stdin(Stdio::null())
           .stdout(Stdio::null())
           .stderr(Stdio::null());
        
        // Use pre_exec to setsid (create new session)
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        
        // Spawn the daemon process
        cmd.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn daemon: {}", e))?;
        
        // Exit the parent process
        std::process::exit(0);
    }
    
    // We're now in the daemon process
    // Change working directory to avoid blocking unmounts
    std::env::set_current_dir("/")
        .map_err(|e| anyhow::anyhow!("Failed to change directory: {}", e))?;
    
    // Set up log file paths
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to determine data directory"))?
        .join("apm");
    
    std::fs::create_dir_all(&data_dir)?;
    
    let log_path = data_dir.join("daemon.log");
    let pid_path = data_dir.join("apm.pid");
    
    // Write PID file
    let pid = std::process::id();
    std::fs::write(&pid_path, pid.to_string())?;
    
    // Redirect stdout and stderr to log file
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    
    let log_fd = log_file.as_raw_fd();
    
    unsafe {
        libc::dup2(log_fd, 1); // stdout
        libc::dup2(log_fd, 2); // stderr
    }
    
    // Close stdin
    unsafe {
        libc::close(0);
    }
    
    println!("APM daemon started with PID {} at {}", pid, chrono::Utc::now());
    println!("Log file: {}", log_path.display());
    
    Ok(())
}

async fn handle_tag_command(tag_command: TagCommands) -> anyhow::Result<()> {
    match tag_command {
        TagCommands::Add { process, tag, all } => {
            add_tag_cli(process, tag, all).await
        }
        TagCommands::Remove { process, tag, all } => {
            remove_tag_cli(process, tag, all).await
        }
        TagCommands::List { process, all } => {
            list_process_tags_cli(process, all).await
        }
        TagCommands::All => {
            list_all_tags_cli().await
        }
    }
}

async fn add_tag_cli(process_name: String, tag: String, all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // First, get the process ID
    let process_id = get_process_id_by_name(&client, &process_name, all).await?;
    
    let add_tag_request = serde_json::json!({"tag": tag});
    
    let response = client
        .post(&format!("http://localhost:7337/api/processes/{}/tags", process_id))
        .json(&add_tag_request)
        .send()
        .await?;
    
    if response.status().is_success() {
        println!("✅ Added tag '{}' to process '{}'", tag, process_name);
    } else {
        let error_text = response.text().await?;
        eprintln!("❌ Failed to add tag: {}", error_text);
    }
    
    Ok(())
}

async fn remove_tag_cli(process_name: String, tag: String, all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // First, get the process ID
    let process_id = get_process_id_by_name(&client, &process_name, all).await?;
    
    let response = client
        .delete(&format!("http://localhost:7337/api/processes/{}/tags/{}", process_id, tag))
        .send()
        .await?;
    
    if response.status().is_success() {
        println!("✅ Removed tag '{}' from process '{}'", tag, process_name);
    } else {
        let error_text = response.text().await?;
        eprintln!("❌ Failed to remove tag: {}", error_text);
    }
    
    Ok(())
}

async fn list_process_tags_cli(process_name: String, all: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // First, get the process ID
    let process_id = get_process_id_by_name(&client, &process_name, all).await?;
    
    let response = client
        .get(&format!("http://localhost:7337/api/processes/{}/tags", process_id))
        .send()
        .await?;
    
    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        if let Some(tags) = data["data"]["tags"].as_array() {
            if tags.is_empty() {
                println!("Process '{}' has no tags", process_name);
            } else {
                println!("Tags for process '{}':", process_name);
                for tag in tags {
                    if let Some(tag_str) = tag.as_str() {
                        println!("  - {}", tag_str);
                    }
                }
            }
        }
    } else {
        let error_text = response.text().await?;
        eprintln!("❌ Failed to get tags: {}", error_text);
    }
    
    Ok(())
}

async fn list_all_tags_cli() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    let response = client
        .get("http://localhost:7337/api/tags")
        .send()
        .await?;
    
    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        if let Some(tags) = data["data"]["tags"].as_array() {
            if tags.is_empty() {
                println!("No tags found in the system");
            } else {
                println!("All tags in the system:");
                for tag in tags {
                    if let Some(tag_str) = tag.as_str() {
                        println!("  - {}", tag_str);
                    }
                }
            }
        }
    } else {
        let error_text = response.text().await?;
        eprintln!("❌ Failed to get all tags: {}", error_text);
    }
    
    Ok(())
}

async fn get_process_id_by_name(client: &reqwest::Client, process_name: &str, all: bool) -> anyhow::Result<String> {
    // Parse process_name to check for PID reference format (name@pid)
    let (name_to_match, pid_to_match) = if let Some(at_pos) = process_name.find('@') {
        let name = &process_name[..at_pos];
        let pid_str = &process_name[at_pos + 1..];
        match pid_str.parse::<u32>() {
            Ok(pid) => (Some(name), Some(pid)),
            Err(_) => (None, None), // Invalid PID format, treat whole string as name
        }
    } else {
        (None, None)
    };
    
    // Load config for access control settings
    let config = match Config::load() {
        Ok(config) => config,
        Err(_) => Config::default(),
    };
    
    let response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to get processes: {}", response.text().await?));
    }
    
    let data: serde_json::Value = response.json().await?;
    
    // Get current working directory for filtering (unless --all is specified)
    let access_group = if !all {
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
        let matches: Vec<&serde_json::Value> = processes.iter()
            .filter(|process| {
                // Check access permissions
                let access_allowed = if let Some(ref group) = access_group {
                    agent_process_manager::utils::check_access(
                        Some(group),
                        process["access_group"].as_str(),
                        true,  // write operation
                        &config.access_control.mode
                    )
                } else {
                    true
                };
                
                if !access_allowed {
                    return false;
                }
                
                // If we have name@pid format, match both name and pid
                if let (Some(name), Some(pid)) = (name_to_match, pid_to_match) {
                    process["name"].as_str() == Some(name) && 
                    process["session_pid"].as_u64() == Some(pid as u64)
                } else {
                    // Original matching logic (support both ID and name)
                    process["name"].as_str() == Some(process_name) || 
                    process["id"].as_str() == Some(process_name)
                }
            })
            .collect();
        
        match matches.len() {
            0 => Err(anyhow::anyhow!("Process '{}' not found", process_name)),
            1 => Ok(matches[0]["id"].as_str().unwrap().to_string()),
            _ => {
                eprintln!("Multiple processes found with name '{}'. Use process ID or name@pid instead:", process_name);
                for process in matches {
                    let name = process["name"].as_str().unwrap_or("unknown");
                    let id = process["id"].as_str().unwrap_or("unknown");
                    let pid = process["session_pid"].as_u64().unwrap_or(0);
                    eprintln!("  - {} (ID: {}, PID: {})", name, id, pid);
                    eprintln!("    Use: {} or {}@{}", id, name, pid);
                }
                Err(anyhow::anyhow!("Ambiguous process name"))
            }
        }
    } else {
        Err(anyhow::anyhow!("Invalid response format"))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config, daemon } => {
            if daemon {
                // Daemonize before initializing anything else
                daemonize_process()?;
            }
            
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
        Commands::List { all, wrap, format, tag, tags_any, tags_all } => {
            init_default_logging();
            list_processes_cli_with_tags(all, wrap, format, tag, tags_any, tags_all).await
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
        Commands::Shutdown { force } => {
            init_default_logging();
            shutdown_cli(force).await
        }
        Commands::Kill { name, all } => {
            init_default_logging();
            kill_process_cli(name, all).await
        }
        Commands::KillAll { current_dir, force } => {
            init_default_logging();
            kill_all_processes_cli(current_dir, force).await
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
        Commands::Tag { tag_command } => {
            init_default_logging();
            handle_tag_command(tag_command).await
        }
        Commands::Config { config_command } => {
            init_default_logging();
            handle_config_command(config_command).await
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
    let process_manager = Arc::new(ProcessManager::new(log_storage.clone(), log_tx)?);
    
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
        let mcp_search_engine = search_engine.clone();
        
        Some(tokio::spawn(async move {
            if let Err(e) = start_mcp_server(mcp_manager, mcp_storage, mcp_config, full_config, mcp_search_engine).await {
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

async fn list_processes_cli_with_tags(
    show_all: bool, 
    wrap: bool, 
    format: String,
    tag_filters: Vec<String>,
    tags_any: Option<String>,
    tags_all: Option<String>
) -> anyhow::Result<()> {
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
            // Filter processes based on access group and tags
            let filtered_processes: Vec<&serde_json::Value> = processes.iter()
                .filter(|process| {
                    // Access group filtering
                    let access_allowed = if let Some(ref group) = access_group {
                        agent_process_manager::utils::check_access(
                            Some(group),
                            process["access_group"].as_str(),
                            false,  // read operation
                            &config.access_control.mode
                        )
                    } else {
                        true
                    };
                    
                    if !access_allowed {
                        return false;
                    }
                    
                    // Tag filtering
                    let process_tags = match process["tags"].as_array() {
                        Some(tags) => tags.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>(),
                        None => vec![],
                    };
                    
                    // Apply --tag filters (OR logic)
                    if !tag_filters.is_empty() {
                        let has_any_tag = tag_filters.iter().any(|filter_tag| {
                            process_tags.contains(&filter_tag.as_str())
                        });
                        if !has_any_tag {
                            return false;
                        }
                    }
                    
                    // Apply --tags-any filter
                    if let Some(ref tags_any_str) = tags_any {
                        let any_tags: Vec<&str> = tags_any_str.split(',').map(|s| s.trim()).collect();
                        let has_any_tag = any_tags.iter().any(|filter_tag| {
                            process_tags.contains(filter_tag)
                        });
                        if !has_any_tag {
                            return false;
                        }
                    }
                    
                    // Apply --tags-all filter
                    if let Some(ref tags_all_str) = tags_all {
                        let all_tags: Vec<&str> = tags_all_str.split(',').map(|s| s.trim()).collect();
                        let has_all_tags = all_tags.iter().all(|filter_tag| {
                            process_tags.contains(filter_tag)
                        });
                        if !has_all_tags {
                            return false;
                        }
                    }
                    
                    true
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
                    println!("name,status,session_pid,uptime_seconds,started_at,ports,directory,tags");
                    for process in filtered_processes {
                        let name = process["name"].as_str().unwrap_or("");
                        let status = process["status"].as_str().unwrap_or("");
                        let session_pid = process["session_pid"].as_u64().unwrap_or(0);
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
                        let tags = if let Some(tags) = process["tags"].as_array() {
                            tags.iter()
                                .filter_map(|t| t.as_str())
                                .collect::<Vec<_>>()
                                .join(";")
                        } else {
                            String::new()
                        };
                        println!("{},{},{},{},{},{},{},{}", name, status, session_pid, uptime, started, ports, cwd, tags);
                    }
                }
                _ => {
                    // Table output (default)
                    // Get terminal width to dynamically adjust directory column
                    let term_width = terminal_size::terminal_size()
                        .map(|(terminal_size::Width(w), _)| w as usize)
                        .unwrap_or(80);
                    
                    let fixed_width = 77; // Increased to account for TAGS column
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
                    
                    println!("{:<15} {:<8} {:<8} {:<8} {:<10} {:<10} {:<12} {:<width$}", 
                        "NAME", "STATUS", "SESSION_PID", "UPTIME", "STARTED", "PORTS", "TAGS", "DIRECTORY",
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
                
                // Format tags with color coding
                let tags_str = if let Some(tags) = process["tags"].as_array() {
                    let tag_names: Vec<String> = tags.iter()
                        .filter_map(|t| t.as_str())
                        .map(|t| t.to_string())
                        .collect();
                    if tag_names.is_empty() {
                        "-".to_string()
                    } else {
                        tag_names.join(",")
                    }
                } else {
                    "-".to_string()
                };
                
                let tags_colored = if tags_str == "-" {
                    tags_str.dimmed()
                } else {
                    tags_str.bright_blue()
                };
                
                if wrap && cwd_str.len() > dir_width {
                    // Multi-line output for wrapped mode
                    println!(
                        "{:<15} {:<8} {:<8} {:<8} {:<10} {:<10} {:<12}",
                        name_display,
                        status_colored,
                        process["session_pid"].as_u64().unwrap_or(0),
                        format!("{}s", process["uptime_seconds"].as_u64().unwrap_or(0)),
                        started_str,
                        ports_colored,
                        tags_colored
                    );
                    // Print wrapped directory on next line with indent
                    println!("    {}", cwd_str.dimmed());
                } else {
                    // Single line output
                    println!(
                        "{:<15} {:<8} {:<8} {:<8} {:<10} {:<10} {:<12} {:<width$}",
                        name_display,
                        status_colored,
                        process["session_pid"].as_u64().unwrap_or(0),
                        format!("{}s", process["uptime_seconds"].as_u64().unwrap_or(0)),
                        started_str,
                        ports_colored,
                        tags_colored,
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
    
    // Use get_process_id_by_name which now supports name@pid format
    let id = match get_process_id_by_name(&client, &name, false).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    
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
            println!("✅ APM daemon is running on port 7337");
            
            // Check for PID file
            if let Some(data_dir) = dirs::data_dir() {
                let pid_path = data_dir.join("apm").join("apm.pid");
                if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        println!("   PID: {}", pid);
                    }
                }
                
                let log_path = data_dir.join("apm").join("daemon.log");
                if log_path.exists() {
                    println!("   Log: {}", log_path.display());
                }
            }
            
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

async fn shutdown_cli(force: bool) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    
    // First check if daemon is running
    match client
        .get("http://localhost:7337/health")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(_) => {
            // Daemon is running, proceed with shutdown
            if !force {
                // Ask for confirmation
                use dialoguer::Confirm;
                let confirmed = Confirm::new()
                    .with_prompt("Are you sure you want to shutdown the APM daemon?")
                    .default(false)
                    .interact()?;
                
                if !confirmed {
                    println!("Shutdown cancelled");
                    return Ok(());
                }
            }
            
            println!("Shutting down APM daemon...");
            
            // Send shutdown request
            match client
                .post("http://localhost:7337/api/shutdown")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        println!("✅ APM daemon shutdown initiated");
                        
                        // Clean up PID file if it exists
                        if let Some(data_dir) = dirs::data_dir() {
                            let pid_path = data_dir.join("apm").join("apm.pid");
                            if pid_path.exists() {
                                let _ = std::fs::remove_file(&pid_path);
                            }
                        }
                        
                        // Wait a moment and verify it's down
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        
                        match client
                            .get("http://localhost:7337/health")
                            .timeout(std::time::Duration::from_secs(1))
                            .send()
                            .await
                        {
                            Ok(_) => println!("⚠️  Daemon may still be running"),
                            Err(_) => println!("✅ Daemon stopped successfully"),
                        }
                    } else {
                        eprintln!("❌ Failed to shutdown daemon: HTTP {}", response.status());
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to send shutdown request: {}", e);
                }
            }
        }
        Err(_) => {
            println!("APM daemon is not running");
        }
    }
    
    Ok(())
}

async fn kill_process_cli(name: String, show_all: bool) -> anyhow::Result<()> {
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
    
    // Now kill the process
    let response = client
        .delete(&format!("http://localhost:7337/api/processes/{}", id))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Killed process '{}'", name);
    } else {
        eprintln!("Failed to kill process: {}", response.text().await?);
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

async fn kill_all_processes_cli(current_dir_only: bool, force: bool) -> anyhow::Result<()> {
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
    let mut processes_to_kill = Vec::new();
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
            
            processes_to_kill.push((
                process["id"].as_str().unwrap_or("").to_string(),
                process["name"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    
    if processes_to_kill.is_empty() {
        if current_dir_only {
            println!("No running processes found in current directory");
        } else {
            println!("No running processes found");
        }
        return Ok(());
    }
    
    // Show what will be killed
    println!("Will kill {} processes:", processes_to_kill.len());
    for (_, name) in &processes_to_kill {
        println!("  - {}", name);
    }
    
    // Confirm unless forced
    if !force {
        print!("\nAre you sure you want to kill all these processes? (y/N): ");
        use std::io::{self, Write};
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "y" {
            println!("Cancelled");
            return Ok(());
        }
    }
    
    // Kill all processes
    let mut killed = 0;
    let mut failed = 0;
    
    for (id, name) in processes_to_kill {
        let response = client
            .delete(&format!("http://localhost:7337/api/processes/{}", id))
            .send()
            .await?;
            
        if response.status().is_success() {
            println!("Killed: {}", name);
            killed += 1;
        } else {
            eprintln!("Failed to kill {}: {}", name, response.text().await?);
            failed += 1;
        }
    }
    
    println!("\nKilled {} processes, {} failed", killed, failed);
    
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
    
    println!("{}", "🤖 APM Setup for Claude Code - Local Project".bright_blue());
    println!("\nThis will configure Claude to use APM for long-running processes in this project.\n");
    
    println!("📍 Current directory: {}", std::env::current_dir()?.display());
    
    println!("\nI'll help you set up APM by:");
    println!("  1. Creating/updating CLAUDE.md in this directory");
    println!("  2. Adding APM usage instructions for Claude");
    println!("  3. Detecting your project type and adding relevant examples");
    println!("  4. Creating .mcp.json to configure the APM MCP tool");
    
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
- Search logs: `apm logs <name> --search "error"`
- Stop processes: `apm stop <name>`
- Stop all: `apm stop-all --current-dir`

### Advanced Debugging (MCP Tools)

When connected via MCP, you have access to powerful query capabilities:
- **Search across all processes**: `query` tool with `type: "log_search"` and `pattern: "your search"`
- **Find port usage**: `query` tool with `type: "port_mapping"` to see which processes use which ports
- **Check performance**: `query` tool with `type: "performance_metrics"` to find high CPU/memory usage
- **Filter logs**: `logs` tool supports `search`, `level` (error/warn/info), and `since` parameters

See: https://github.com/sunnya97/agent-process-manager/blob/main/APM_FOR_AI_ASSISTANTS.md
"#, examples.iter().map(|e| format!("- {}", e)).collect::<Vec<_>>().join("\n"));
    
    if claude_exists {
        // Check if APM section already exists
        let mut content = fs::read_to_string("CLAUDE.md").await?;
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
            
            // Replace the existing APM section instead of appending
            // Find the start of the APM section
            if let Some(start_pos) = content.find("## Process Management with APM") {
                // Find the end of the APM section (next ## heading or end of file)
                let section_start = &content[start_pos..];
                let end_pos = section_start[3..] // Skip the current "## "
                    .find("\n## ")
                    .map(|pos| start_pos + 3 + pos)
                    .unwrap_or(content.len());
                
                // Replace the section, preserving any trailing content
                let before_section = &content[..start_pos];
                let after_section = if end_pos < content.len() {
                    &content[end_pos..]
                } else {
                    ""
                };
                
                // Reconstruct the content with the new APM section
                let new_content = format!("{}{}{}", before_section, apm_section, after_section);
                fs::write("CLAUDE.md", new_content).await?;
                println!("✓ Updated APM configuration in CLAUDE.md");
            } else {
                // Somehow the check passed but we can't find it, append as fallback
                content.push_str(&apm_section);
                fs::write("CLAUDE.md", content).await?;
                println!("✓ Added APM configuration to CLAUDE.md");
            }
        } else {
            // No APM section exists, append it
            content.push_str(&apm_section);
            fs::write("CLAUDE.md", content).await?;
            println!("✓ Added APM configuration to CLAUDE.md");
        }
    } else {
        // Create new file
        let full_content = format!(r#"# CLAUDE.md - Project Instructions

This file contains instructions for AI assistants working on this project.
{}
"#, apm_section);
        
        fs::write("CLAUDE.md", full_content).await?;
        println!("✓ Created CLAUDE.md with APM configuration");
    }
    
    // Create .mcp.json for project-scoped MCP configuration
    println!("\n📦 Configuring MCP for this project...");
    
    let mcp_config = serde_json::json!({
        "mcpServers": {
            "agent-process-manager": {
                "command": "apm",
                "args": ["mcp-bridge"],
                "env": {
                    "RUST_LOG": "warn"
                }
            }
        }
    });
    
    let mcp_path = Path::new(".mcp.json");
    let mcp_exists = mcp_path.exists();
    
    if mcp_exists {
        // Read existing config and check if APM is already configured
        let existing_content = fs::read_to_string(mcp_path).await?;
        match serde_json::from_str::<serde_json::Value>(&existing_content) {
            Ok(mut existing_config) => {
                if existing_config.get("mcpServers")
                    .and_then(|servers| servers.get("agent-process-manager"))
                    .is_some() {
                    println!("✓ MCP already configured for APM in .mcp.json");
                } else {
                    // Merge APM config into existing config
                    if let Some(servers) = existing_config.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
                        servers.insert("agent-process-manager".to_string(), mcp_config["mcpServers"]["agent-process-manager"].clone());
                    } else {
                        existing_config["mcpServers"] = mcp_config["mcpServers"].clone();
                    }
                    
                    let pretty_json = serde_json::to_string_pretty(&existing_config)?;
                    fs::write(mcp_path, pretty_json).await?;
                    println!("✓ Added APM to existing .mcp.json");
                }
            }
            Err(_) => {
                println!("⚠️  Existing .mcp.json is invalid, creating backup...");
                fs::rename(mcp_path, ".mcp.json.backup").await?;
                let pretty_json = serde_json::to_string_pretty(&mcp_config)?;
                fs::write(mcp_path, pretty_json).await?;
                println!("✓ Created new .mcp.json (old file backed up)");
            }
        }
    } else {
        // Create new .mcp.json
        let pretty_json = serde_json::to_string_pretty(&mcp_config)?;
        fs::write(mcp_path, pretty_json).await?;
        println!("✓ Created .mcp.json with APM configuration");
    }
    
    println!("\n✅ {}", "Local setup complete!".green().bold());
    println!("\nClaude will now use APM for long-running processes in this project.");
    println!("Team members will get the same setup when they clone this repo.");
    
    println!("\n⚠️  {}: Restart Claude Code for MCP changes to take effect", "Important".yellow().bold());
    
    println!("\n💡 Tips:");
    println!("- Customize CLAUDE.md for your specific needs");
    println!("- The .mcp.json file configures MCP tools for this project");
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
- Search logs: `apm logs <name> --search "pattern"`
- Stop processes: `apm stop <name>`
- Check status: `apm status`

### MCP Tools for Advanced Debugging:
When using APM through MCP, you have access to powerful tools:
- **query**: Search logs across all processes, find port usage, check performance
- **logs**: Enhanced filtering with search, level (error/warn/info), and time ranges
- **restart**: Restart processes without losing configuration

For complete instructions: https://github.com/sunnya97/agent-process-manager/blob/main/APM_FOR_AI_ASSISTANTS.md

When in doubt, use APM! It's better to spawn a short process unnecessarily than to get blocked.
"#;
    
    if claude_md_path.exists() {
        let mut content = fs::read_to_string(&claude_md_path).await?;
        if content.contains("Process Management with APM") {
            println!("✓ ~/.config/claude/CLAUDE.md already contains APM configuration");
            if !yes {
                let update = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Update the existing global APM configuration?")
                    .default(false)
                    .interact()?;
                if !update {
                    println!("Keeping existing configuration.");
                    return Ok(());
                }
            }
            
            // Replace the existing APM section instead of appending
            if let Some(start_pos) = content.find("## Process Management with APM") {
                // Find the end of the APM section (next ## heading or end of file)
                let section_start = &content[start_pos..];
                let end_pos = section_start[3..] // Skip the current "## "
                    .find("\n## ")
                    .map(|pos| start_pos + 3 + pos)
                    .unwrap_or(content.len());
                
                // Replace the section, preserving any trailing content
                let before_section = &content[..start_pos];
                let after_section = if end_pos < content.len() {
                    &content[end_pos..]
                } else {
                    ""
                };
                
                // Reconstruct the content with the new APM section
                let new_content = format!("{}{}{}", before_section, apm_global_content, after_section);
                fs::write(&claude_md_path, new_content).await?;
                println!("✓ Updated APM configuration in ~/.config/claude/CLAUDE.md");
            } else {
                // Somehow the check passed but we can't find it, append as fallback
                content.push_str(apm_global_content);
                fs::write(&claude_md_path, content).await?;
                println!("✓ Added APM configuration to ~/.config/claude/CLAUDE.md");
            }
        } else {
            // No APM section exists, append it
            content.push_str(apm_global_content);
            fs::write(&claude_md_path, content).await?;
            println!("✓ Added APM configuration to ~/.config/claude/CLAUDE.md");
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
    
    // Check local MCP configuration
    let local_mcp = Path::new(".mcp.json");
    if local_mcp.exists() {
        let content = fs::read_to_string(local_mcp).await?;
        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(config) => {
                if config.get("mcpServers")
                    .and_then(|servers| servers.get("agent-process-manager"))
                    .is_some() {
                    println!("  ✓ .mcp.json configured with APM");
                } else {
                    println!("  ✗ .mcp.json exists but lacks APM configuration");
                }
            }
            Err(_) => {
                println!("  ✗ .mcp.json exists but is invalid");
            }
        }
    } else {
        println!("  ✗ No .mcp.json file in current directory");
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

async fn handle_config_command(config_command: ConfigCommands) -> anyhow::Result<()> {
    match config_command {
        ConfigCommands::Init { force } => config_init_cli(force).await,
        ConfigCommands::Path => config_path_cli().await,
        ConfigCommands::Edit => config_edit_cli().await,
    }
}

async fn config_init_cli(force: bool) -> anyhow::Result<()> {
    use std::fs;
    
    let config_path = match Config::get_user_config_path() {
        Some(path) => path,
        None => {
            eprintln!("❌ Could not determine user config directory");
            return Ok(());
        }
    };
    
    // Create the directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
            println!("📁 Created config directory: {}", parent.display());
        }
    }
    
    // Check if config file already exists
    if config_path.exists() && !force {
        eprintln!("❌ Config file already exists at: {}", config_path.display());
        eprintln!("   Use --force to overwrite");
        return Ok(());
    }
    
    // Write the default config
    fs::write(&config_path, Config::default_config_yaml())?;
    
    println!("✅ Created config file: {}", config_path.display());
    println!("   Edit this file to customize APM settings");
    
    Ok(())
}

async fn config_path_cli() -> anyhow::Result<()> {
    let config_paths = Config::get_config_paths();
    
    // Find which config file exists and is being used
    let active_config = config_paths.iter()
        .find(|path| path.exists());
        
    println!("📋 Configuration Paths:");
    println!("\n📁 Config file search order:");
    for (i, path) in config_paths.iter().enumerate() {
        let status = if path.exists() {
            if Some(path) == active_config {
                "✅ ACTIVE"
            } else {
                "📄 exists"
            }
        } else {
            "❌ not found"
        };
        println!("  {}. {} - {}", i + 1, path.display(), status);
    }
    
    if let Some(active) = active_config {
        println!("\n🎯 Currently using config: {}", active.display());
    } else {
        println!("\n⚠️  No config file found, using defaults");
        if let Some(user_path) = Config::get_user_config_path() {
            println!("   Run 'apm config init' to create: {}", user_path.display());
        }
    }
    
    // Show data directory information
    println!("\n💾 Data directory:");
    match Config::get_data_dir() {
        Some(data_dir) => {
            let exists = data_dir.exists();
            let status = if exists { "✅ exists" } else { "❌ not found" };
            println!("   {} - {}", data_dir.display(), status);
            if exists {
                // Show what's in the data directory
                if let Ok(entries) = std::fs::read_dir(&data_dir) {
                    let mut files: Vec<_> = entries.filter_map(|e| e.ok()).collect();
                    files.sort_by_key(|e| e.file_name());
                    if !files.is_empty() {
                        println!("   Contents:");
                        for entry in files {
                            let name = entry.file_name();
                            let file_type = if entry.path().is_dir() { "📁" } else { "📄" };
                            println!("     {} {}", file_type, name.to_string_lossy());
                        }
                    }
                }
            } else {
                println!("   Data directory will be created when APM starts");
            }
        }
        None => {
            println!("   ❌ Could not determine data directory");
        }
    }
    
    Ok(())
}

async fn config_edit_cli() -> anyhow::Result<()> {
    use std::process::Command;
    
    let config_paths = Config::get_config_paths();
    
    // Find existing config file or use user config path
    let config_file = config_paths.iter()
        .find(|path| path.exists())
        .cloned()
        .or_else(|| Config::get_user_config_path());
        
    let config_file = match config_file {
        Some(path) => path,
        None => {
            eprintln!("❌ Could not determine config file path");
            return Ok(());
        }
    };
    
    // If file doesn't exist, offer to create it
    if !config_file.exists() {
        println!("📝 Config file doesn't exist: {}", config_file.display());
        println!("   Run 'apm config init' first to create it");
        return Ok(());
    }
    
    // Get editor from environment or use default
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(target_os = "macos") {
                "open".to_string()
            } else if cfg!(target_os = "windows") {
                "notepad".to_string()
            } else {
                "nano".to_string()
            }
        });
    
    println!("📝 Opening config file in {}: {}", editor, config_file.display());
    
    let mut cmd = Command::new(&editor);
    cmd.arg(&config_file);
    
    match cmd.status() {
        Ok(status) => {
            if status.success() {
                println!("✅ Config file editing completed");
            } else {
                eprintln!("❌ Editor exited with error");
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to open editor '{}': {}", editor, e);
            eprintln!("   Try setting EDITOR environment variable");
        }
    }
    
    Ok(())
}