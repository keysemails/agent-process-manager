//! Agent Process Manager CLI and daemon

use agent_process_manager::{
    api, config::Config, logs::LogStorage, process::ProcessManager,
};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
    List,
    
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
    },
    
    /// Restart a process
    Restart {
        /// Process name or ID
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agent_process_manager=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config } => start_daemon(config).await,
        Commands::StartProcess { name, command, args, tag, pty } => {
            start_process_cli(name, command, args, tag, pty).await
        }
        Commands::List => list_processes_cli().await,
        Commands::Logs { name, errors, follow } => {
            show_logs_cli(name, errors, follow).await
        }
        Commands::Attach { name, read_only } => {
            attach_to_process_cli(name, read_only).await
        }
        Commands::Status => show_status_cli().await,
        Commands::Stop { name } => stop_process_cli(name).await,
        Commands::Restart { name } => restart_process_cli(name).await,
    }
}

async fn start_daemon(config_path: Option<String>) -> anyhow::Result<()> {
    info!("Starting Agent Process Manager daemon...");

    // Load configuration
    let config = if let Some(_path) = config_path {
        // Load from specific file
        todo!("Load config from file")
    } else {
        Config::load().unwrap_or_default()
    };

    // Initialize storage
    let log_storage = Arc::new(
        LogStorage::new(&config.storage.database_url).await?
    );

    // Create log channel
    let (log_tx, mut log_rx) = tokio::sync::mpsc::channel(1000);

    // Initialize process manager
    let process_manager = Arc::new(ProcessManager::new(log_tx));

    // Start log processing task
    let storage = log_storage.clone();
    tokio::spawn(async move {
        while let Some((process_id, line)) = log_rx.recv().await {
            if let Err(e) = storage.store(process_id, line).await {
                error!("Failed to store log: {}", e);
            }
        }
    });

    // Create router
    let app = api::create_router(process_manager, log_storage);

    // Start server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("APM daemon listening on {}", addr);
    info!("Dashboard available at http://{}/dashboard", addr);
    
    axum::serve(listener, app).await?;

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
    
    let config = agent_process_manager::process::ProcessConfig {
        name: name.clone(),
        command,
        args,
        cwd: None,
        env: std::collections::HashMap::new(),
        tags,
        pty,
        restart_policy: Default::default(),
        resources: Default::default(),
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

async fn list_processes_cli() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:7337/api/processes")
        .send()
        .await?;

    if response.status().is_success() {
        let data: serde_json::Value = response.json().await?;
        
        println!("{:<20} {:<10} {:<10} {:<10}", "NAME", "STATUS", "PID", "UPTIME");
        println!("{}", "-".repeat(50));
        
        if let Some(processes) = data["data"].as_array() {
            for process in processes {
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

async fn show_logs_cli(name: String, errors_only: bool, _follow: bool) -> anyhow::Result<()> {
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
    let process_id = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name))
            .and_then(|p| p["id"].as_str())
    } else {
        None
    };
    
    let Some(id) = process_id else {
        eprintln!("Error: Process '{}' not found", name);
        std::process::exit(1);
    };
    
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

async fn attach_to_process_cli(name: String, _read_only: bool) -> anyhow::Result<()> {
    println!("Attaching to process '{}'...", name);
    println!("Press Ctrl+B, D to detach");
    
    // TODO: Implement WebSocket connection for terminal attachment
    // This would open a WebSocket to /api/attach/{id}
    // And bridge it to the local terminal
    
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

async fn stop_process_cli(name: String) -> anyhow::Result<()> {
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
    let process_id = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name))
            .and_then(|p| p["id"].as_str())
    } else {
        None
    };
    
    let Some(id) = process_id else {
        eprintln!("Error: Process '{}' not found", name);
        std::process::exit(1);
    };
    
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

async fn restart_process_cli(name: String) -> anyhow::Result<()> {
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
    let process_id = if let Some(data) = processes["data"].as_array() {
        data.iter()
            .find(|p| p["name"].as_str() == Some(&name) || p["id"].as_str() == Some(&name))
            .and_then(|p| p["id"].as_str())
    } else {
        None
    };
    
    let Some(id) = process_id else {
        eprintln!("Process '{}' not found", name);
        return Ok(());
    };
    
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