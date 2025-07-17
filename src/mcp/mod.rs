use anyhow::Result;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Content, ListToolsResult,
        PaginatedRequestParam, Tool,
    },
    service::{RequestContext, RoleServer, ServiceExt},
    transport::stdio,
    Error as McpError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::logs::{LogFormat, LogLevel, LogQuery, LogStorage};
use crate::process::{ProcessConfig, ProcessId, ProcessManager};

use std::future::Future;

mod tcp_server;
pub use tcp_server::start_mcp_server;

/// MCP server implementation for Agent Process Manager
pub struct McpServer {
    handler: McpServerHandler,
}

/// Internal handler that implements the MCP protocol
#[derive(Clone)]
pub(crate) struct McpServerHandler {
    pub process_manager: Arc<ProcessManager>,
    pub log_storage: Arc<LogStorage>,
    pub config: crate::config::Config,
    pub search_engine: Option<Arc<crate::logs::LogSearchEngine>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpawnArgs {
    name: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogsArgs {
    process_id: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    since: Option<String>,
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Serialize, Deserialize)]
struct KillArgs {
    process_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RestartArgs {
    process_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ListArgs {
    #[serde(default)]
    current_dir: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryArgs {
    #[serde(rename = "type")]
    query_type: String,
    #[serde(default)]
    time_window: Option<String>,
    #[serde(default)]
    current_dir: bool,
    #[serde(default)]
    process_filter: Option<Vec<String>>,
    #[serde(default)]
    include_urls: Option<bool>,
    #[serde(default)]
    metrics: Option<Vec<String>>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    event_types: Option<Vec<String>>,
    #[serde(default)]
    min_severity: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KillMultipleArgs {
    #[serde(default)]
    current_dir: bool,
    #[serde(default)]
    names: Option<Vec<String>>,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CleanArgs {
    #[serde(default)]
    older_than: Option<u64>,
    #[serde(default)]
    keep_logs: bool,
    #[serde(default)]
    current_dir: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchArgs {
    query: String,
    #[serde(default)]
    process_id: Option<String>,
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    until: Option<String>,
    #[serde(default = "default_search_limit")]
    limit: usize,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    highlight: Option<bool>,
}

fn default_search_limit() -> usize {
    50
}

impl McpServer {
    pub async fn new(
        process_manager: Arc<ProcessManager>,
        log_storage: Arc<LogStorage>,
        config: crate::config::Config,
        search_engine: Option<Arc<crate::logs::LogSearchEngine>>,
    ) -> Result<Self> {
        let handler = McpServerHandler {
            process_manager,
            log_storage,
            config,
            search_engine,
        };
        Ok(Self { handler })
    }

    pub async fn run(self) -> Result<()> {
        info!("Starting MCP server on stdin/stdout");

        // Start the server with stdio transport
        let service = self.handler
            .serve(stdio())
            .await
            .map_err(|e| anyhow::anyhow!("MCP server initialization error: {:?}", e))?;

        // Wait for the service to complete
        service
            .waiting()
            .await
            .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?;

        Ok(())
    }
}

impl McpServerHandler {
    fn create_text_content(text: String) -> Content {
        Content::text(text)
    }
    

    fn create_error_result(message: String) -> CallToolResult {
        CallToolResult {
            content: vec![Self::create_text_content(message)],
            is_error: Some(true),
        }
    }

    fn create_success_result(content: String) -> CallToolResult {
        CallToolResult {
            content: vec![Self::create_text_content(content)],
            is_error: None,
        }
    }

    async fn handle_spawn(&self, args: SpawnArgs) -> CallToolResult {
        debug!("MCP spawn tool called: {:?}", args);

        // Get the current working directory for access control and process config
        let (access_group, cwd) = match std::env::current_dir() {
            Ok(cwd) => (Some(crate::utils::access_group_from_dir(&cwd)), Some(cwd)),
            Err(e) => {
                error!("Failed to get current directory: {}", e);
                (None, None)
            }
        };

        let config = ProcessConfig {
            name: args.name.clone(),
            command: args.command,
            args: args.args,
            cwd,
            env: HashMap::new(),
            tags: vec![],
            pty: false,
            use_tmux: true,
            restart_policy: Default::default(),
            resources: Default::default(),
            access_group,
        };

        match self.process_manager.spawn_process(config).await {
            Ok(process_info) => {
                let response = json!({
                    "success": true,
                    "process_id": process_info.id.to_string(),
                    "message": format!("Process '{}' spawned successfully", args.name)
                });
                Self::create_success_result(response.to_string())
            }
            Err(e) => {
                error!("Failed to spawn process: {}", e);
                Self::create_error_result(format!("Failed to spawn process: {}", e))
            }
        }
    }

    async fn handle_list(&self, args: ListArgs) -> CallToolResult {
        debug!("MCP list tool called: {:?}", args);

        match self.process_manager.list_processes().await {
            Ok(processes) => {
                // Filter processes by access group if current_dir is true
                let filtered_processes: Vec<_> = if args.current_dir {
                    // Get the current working directory for access control
                    let access_group = match std::env::current_dir() {
                        Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                        Err(e) => {
                            error!("Failed to get current directory: {}", e);
                            None
                        }
                    };
                    
                    processes
                        .into_iter()
                        .filter(|p| {
                            if let Some(ref group) = access_group {
                                crate::utils::check_access(
                                    Some(group),
                                    p.access_group.as_deref(),
                                    false,  // read operation
                                    &self.config.access_control.mode
                                )
                            } else {
                                true
                            }
                        })
                        .collect()
                } else {
                    // Show all processes when current_dir is false
                    processes
                };
                
                let process_list: Vec<Value> = filtered_processes
                    .into_iter()
                    .map(|info| {
                        json!({
                            "id": info.id.to_string(),
                            "name": info.name,
                            "command": info.command,
                            "status": info.status,
                            "created_at": info.started_at,
                            "pid": info.pid,
                            "actual_pid": info.actual_pid,
                            "actual_name": info.actual_name,
                            "uptime_seconds": info.uptime_seconds,
                            "restart_count": info.restart_count,
                            "cpu_percent": info.cpu_percent,
                            "memory_mb": info.memory_mb,
                            "cwd": info.cwd,
                            "detected_ports": info.detected_ports
                        })
                    })
                    .collect();

                Self::create_success_result(
                    serde_json::to_string_pretty(&process_list).unwrap_or_else(|_| "[]".to_string()),
                )
            }
            Err(e) => {
                error!("Failed to list processes: {}", e);
                Self::create_error_result(format!("Failed to list processes: {}", e))
            }
        }
    }

    async fn handle_logs(&self, args: LogsArgs) -> CallToolResult {
        debug!("MCP logs tool called: {:?}", args);

        let process_id = match args.process_id.parse() {
            Ok(id) => ProcessId(id),
            Err(_) => {
                return Self::create_error_result("Invalid process ID".to_string());
            }
        };

        // Get the current working directory for access control
        let access_group = match std::env::current_dir() {
            Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                error!("Failed to get current directory: {}", e);
                None
            }
        };

        // Check if process exists and is accessible
        match self.process_manager.get_process(&process_id).await {
            Ok(process_info) => {
                // Check access permissions using configurable read access
                if !crate::utils::check_access(
                    access_group.as_deref(),
                    process_info.access_group.as_deref(),
                    false,  // read operation
                    &self.config.access_control.mode
                ) {
                    return Self::create_error_result(format!("Access denied: process '{}' is not accessible from this directory", process_id));
                }
                
                // Parse log level if provided
                let level = args.level.as_ref().and_then(|l| match l.to_lowercase().as_str() {
                    "debug" => Some(crate::logs::LogLevel::Debug),
                    "info" => Some(crate::logs::LogLevel::Info),
                    "warn" => Some(crate::logs::LogLevel::Warn),
                    "error" => Some(crate::logs::LogLevel::Error),
                    _ => None,
                });

                // Parse since timestamp if provided
                let since = args.since.as_ref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .ok()
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                });

                let query = LogQuery {
                    process_id: process_id.clone(),
                    format: LogFormat::Json,
                    lines: Some(args.limit),
                    search: args.search,
                    level,
                    since,
                };

                match self.log_storage.query(query).await {
                    Ok(logs) => {
                        let log_entries: Vec<Value> = logs
                            .into_iter()
                            .map(|entry| {
                                json!({
                                    "timestamp": entry.timestamp,
                                    "line": entry.raw_line,
                                    "level": entry.level,
                                    "patterns": entry.patterns
                                })
                            })
                            .collect();

                        Self::create_success_result(
                            serde_json::to_string_pretty(&log_entries).unwrap_or_else(|_| "[]".to_string()),
                        )
                    }
                    Err(e) => {
                        error!("Failed to get logs: {}", e);
                        Self::create_error_result(format!("Failed to get logs: {}", e))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get process info: {}", e);
                Self::create_error_result(format!("Process not found: {}", e))
            }
        }
    }

    async fn handle_kill(&self, args: KillArgs) -> CallToolResult {
        debug!("MCP kill tool called: {:?}", args);

        let process_id = match args.process_id.parse() {
            Ok(id) => ProcessId(id),
            Err(_) => {
                return Self::create_error_result("Invalid process ID".to_string());
            }
        };

        // Get the current working directory for access control
        let access_group = match std::env::current_dir() {
            Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                error!("Failed to get current directory: {}", e);
                None
            }
        };

        // Check if process exists and is accessible
        match self.process_manager.get_process(&process_id).await {
            Ok(process_info) => {
                // Check access permissions - WRITE operation always requires hierarchical access
                if !crate::utils::check_access(
                    access_group.as_deref(),
                    process_info.access_group.as_deref(),
                    true,   // write operation
                    &self.config.access_control.mode
                ) {
                    return Self::create_error_result(format!("Access denied: process '{}' is not accessible from this directory", process_id));
                }
                
                // Kill the process
                match self.process_manager.kill_process(&process_id).await {
                    Ok(()) => {
                        let response = json!({
                            "success": true,
                            "message": format!("Process '{}' killed successfully", process_id)
                        });
                        Self::create_success_result(response.to_string())
                    }
                    Err(e) => {
                        error!("Failed to kill process: {}", e);
                        Self::create_error_result(format!("Failed to kill process: {}", e))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get process info: {}", e);
                Self::create_error_result(format!("Process not found: {}", e))
            }
        }
    }

    async fn handle_restart(&self, args: RestartArgs) -> CallToolResult {
        debug!("MCP restart tool called: {:?}", args);

        let process_id = match args.process_id.parse() {
            Ok(id) => ProcessId(id),
            Err(_) => {
                return Self::create_error_result("Invalid process ID".to_string());
            }
        };

        // Get the current working directory for access control
        let access_group = match std::env::current_dir() {
            Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                error!("Failed to get current directory: {}", e);
                None
            }
        };

        // Check if process exists and is accessible
        match self.process_manager.get_process(&process_id).await {
            Ok(process_info) => {
                // Check access permissions - WRITE operation always requires hierarchical access
                if !crate::utils::check_access(
                    access_group.as_deref(),
                    process_info.access_group.as_deref(),
                    true,   // write operation
                    &self.config.access_control.mode
                ) {
                    return Self::create_error_result(format!("Access denied: process '{}' is not accessible from this directory", process_id));
                }
                
                // Restart the process
                match self.process_manager.restart_process(&process_id).await {
                    Ok(new_info) => {
                        let response = json!({
                            "success": true,
                            "process_id": new_info.id.to_string(),
                            "message": format!("Process '{}' restarted successfully", new_info.name),
                            "restart_count": new_info.restart_count
                        });
                        Self::create_success_result(response.to_string())
                    }
                    Err(e) => {
                        error!("Failed to restart process: {}", e);
                        Self::create_error_result(format!("Failed to restart process: {}", e))
                    }
                }
            }
            Err(e) => {
                error!("Failed to get process info: {}", e);
                Self::create_error_result(format!("Process not found: {}", e))
            }
        }
    }

    async fn handle_kill_multiple(&self, args: KillMultipleArgs) -> CallToolResult {
        debug!("MCP kill_multiple tool called: {:?}", args);

        // Get the current working directory for access control
        let access_group = match std::env::current_dir() {
            Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
            Err(e) => {
                error!("Failed to get current directory: {}", e);
                None
            }
        };

        // Get all processes
        let all_processes = match self.process_manager.list_processes().await {
            Ok(processes) => processes,
            Err(e) => {
                return Self::create_error_result(format!("Failed to list processes: {}", e));
            }
        };

        // Filter processes based on criteria
        let mut processes_to_kill = Vec::new();
        
        for process in all_processes {
            // Apply name filter if provided
            if let Some(ref names) = args.names {
                if !names.contains(&process.name) {
                    continue;
                }
            }
            
            // Apply current_dir filter if requested
            if args.current_dir {
                if !crate::utils::check_access(
                    access_group.as_deref(),
                    process.access_group.as_deref(),
                    true,   // write operation
                    &self.config.access_control.mode
                ) {
                    continue;
                }
            }
            
            processes_to_kill.push((process.id.clone(), process.name.clone()));
        }

        if processes_to_kill.is_empty() {
            return Self::create_success_result(json!({
                "success": true,
                "killed": 0,
                "failed": 0,
                "message": "No processes matched the criteria"
            }).to_string());
        }

        // Kill all matching processes
        let mut killed = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        for (id, name) in processes_to_kill {
            match self.process_manager.kill_process(&id).await {
                Ok(()) => killed += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(json!({
                        "process": name,
                        "error": e.to_string()
                    }));
                }
            }
        }

        let response = json!({
            "success": failed == 0,
            "killed": killed,
            "failed": failed,
            "errors": errors,
            "message": format!("Killed {} processes, {} failed", killed, failed)
        });
        
        Self::create_success_result(response.to_string())
    }

    async fn handle_clean(&self, args: CleanArgs) -> CallToolResult {
        debug!("MCP clean tool called: {:?}", args);

        // Get access group for filtering if current_dir is true
        let access_group = if args.current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };

        match self.log_storage.clean_stopped_processes(
            args.older_than,
            access_group.as_deref(),
            args.keep_logs,
        ).await {
            Ok(cleaned) => {
                let count = cleaned.len();
                let names: Vec<String> = cleaned.into_iter().map(|(_, name)| name).collect();
                let response = json!({
                    "success": true,
                    "cleaned": count,
                    "processes": names,
                    "message": format!("Cleaned {} stopped processes", count)
                });
                Self::create_success_result(response.to_string())
            }
            Err(e) => {
                error!("Failed to clean processes: {}", e);
                Self::create_error_result(format!("Failed to clean processes: {}", e))
            }
        }
    }

    async fn handle_search(&self, args: SearchArgs) -> CallToolResult {
        debug!("MCP search tool called: {:?}", args);

        // Check if search engine is available
        let search_engine = match &self.search_engine {
            Some(engine) => engine,
            None => {
                return Self::create_error_result(
                    "Full-text search is not enabled. Please enable search in the configuration.\n\n\
                    Alternatives:\n\
                    • Use 'logs' tool with search parameter for basic filtering within a single process\n\
                    • Use 'query' tool with type='log_search' for simple pattern matching across processes\n\
                    • Ask the user to enable search by setting 'search.enabled: true' in the config".to_string()
                );
            }
        };

        // Parse process ID if provided
        let process_id = if let Some(pid_str) = args.process_id {
            match pid_str.parse() {
                Ok(id) => Some(ProcessId(id)),
                Err(_) => {
                    return Self::create_error_result("Invalid process ID format".to_string());
                }
            }
        } else {
            None
        };

        // Parse log level if provided
        let level = args.level.as_ref().and_then(|l| match l.to_lowercase().as_str() {
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            _ => None,
        });

        // Parse timestamps if provided
        let since = args.since.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });

        let until = args.until.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });

        // Create search query
        let search_query = crate::logs::SearchQuery {
            query: args.query,
            process_id,
            level,
            since,
            until,
            patterns: None,
            limit: Some(args.limit),
            offset: args.offset,
            highlight: args.highlight,
        };

        // Execute search
        match search_engine.search(search_query).await {
            Ok(response) => {
                // Convert search results to MCP format
                let results: Vec<Value> = response.results
                    .into_iter()
                    .map(|result| {
                        json!({
                            "log_id": result.log_id,
                            "score": result.score,
                            "process_id": result.process_id.to_string(),
                            "timestamp": result.timestamp,
                            "level": format!("{:?}", result.level),
                            "line": result.raw_line,
                            "clean_line": result.clean_line,
                            "snippet": result.snippet,
                            "patterns": result.patterns
                        })
                    })
                    .collect();

                let mcp_response = json!({
                    "success": true,
                    "results": results,
                    "total_hits": response.total_hits,
                    "query_time_ms": response.query_time_ms,
                    "query": response.query
                });

                Self::create_success_result(serde_json::to_string_pretty(&mcp_response).unwrap_or_else(|_| "{}".to_string()))
            }
            Err(e) => {
                error!("Search failed: {}", e);
                Self::create_error_result(format!("Search failed: {}", e))
            }
        }
    }

    async fn handle_query(&self, args: QueryArgs) -> CallToolResult {
        debug!("MCP query tool called: {:?}", args);

        let result = match args.query_type.as_str() {
            "system_overview" => self.query_system_overview(args.current_dir).await,
            "process_errors" => self.query_process_errors(args.process_filter, args.time_window, args.min_severity, args.current_dir).await,
            "port_mapping" => self.query_port_mapping(args.include_urls.unwrap_or(false), args.current_dir).await,
            "performance_metrics" => self.query_performance_metrics(args.process_filter, args.metrics.unwrap_or_else(|| vec!["cpu".to_string(), "memory".to_string()]), args.current_dir).await,
            "log_search" => {
                if let Some(pattern) = args.pattern {
                    self.query_log_search(pattern, args.process_filter, args.limit, args.current_dir).await
                } else {
                    return Self::create_error_result("log_search requires 'pattern' parameter".to_string());
                }
            }
            "event_correlation" => {
                if let Some(event_types) = args.event_types {
                    self.query_event_correlation(event_types, args.time_window, args.current_dir).await
                } else {
                    return Self::create_error_result("event_correlation requires 'event_types' parameter".to_string());
                }
            }
            _ => {
                return Self::create_error_result(format!("Unknown query type: {}", args.query_type));
            }
        };

        match result {
            Ok(data) => Self::create_success_result(
                serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string()),
            ),
            Err(e) => {
                error!("Query failed: {}", e);
                Self::create_error_result(format!("Query failed: {}", e))
            }
        }
    }

    // Helper methods
    async fn query_system_overview(&self, current_dir: bool) -> Result<Value> {
        // Get the current working directory for access control if current_dir is true
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let all_processes = self.process_manager.list_processes().await?;
        
        // Filter processes by access group if current_dir is true
        let processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,  // read operation
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            // Show all processes when current_dir is false
            all_processes
        };
        
        let overview = json!({
            "total_processes": processes.len(),
            "running": processes.iter().filter(|p| p.status == crate::process::ProcessStatus::Running).count(),
            "stopped": processes.iter().filter(|p| p.status == crate::process::ProcessStatus::Stopped).count(),
            "processes": processes
        });
        Ok(overview)
    }

    async fn query_process_errors(&self, process_filter: Option<Vec<String>>, _time_window: Option<String>, _min_severity: Option<String>, current_dir: bool) -> Result<Value> {
        // Get the current working directory for access control if current_dir is true
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let mut all_errors = Vec::new();
        let all_processes = self.process_manager.list_processes().await?;
        
        // Filter processes by access group if current_dir is true
        let mut processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,  // read operation
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            // Show all processes when current_dir is false
            all_processes
        };
        
        // Apply process filter if provided
        if let Some(filter) = &process_filter {
            processes = processes.into_iter()
                .filter(|p| filter.contains(&p.name))
                .collect();
        }

        for process in processes {
            let query = LogQuery {
                process_id: process.id.clone(),
                format: LogFormat::Json,
                lines: Some(1000),
                search: None,
                level: Some(LogLevel::Error),
                since: None,
            };
            let logs = self.log_storage.query(query).await?;
            let errors: Vec<_> = logs
                .into_iter()
                .map(|log| {
                    json!({
                        "process_id": process.id.to_string(),
                        "process_name": process.name,
                        "timestamp": log.timestamp,
                        "message": log.raw_line
                    })
                })
                .collect();
            all_errors.extend(errors);
        }

        Ok(json!({ "errors": all_errors }))
    }

    async fn query_port_mapping(&self, include_urls: bool, current_dir: bool) -> Result<Value> {
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let all_processes = self.process_manager.list_processes().await?;
        let processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            all_processes
        };
        
        let mut port_map = serde_json::Map::new();
        let mut url_map = serde_json::Map::new();
        
        for process in &processes {
            if let Ok(summary) = self.log_storage.get_summary(&process.id).await {
                for port in &summary.detected_ports {
                    port_map.insert(
                        port.to_string(),
                        json!({
                            "process": process.name,
                            "status": format!("{:?}", process.status).to_lowercase(),
                        })
                    );
                }
                
                if include_urls {
                    for url in &summary.detected_urls {
                        url_map.insert(
                            url.clone(),
                            json!({
                                "process": process.name,
                                "status": format!("{:?}", process.status).to_lowercase(),
                            })
                        );
                    }
                }
            }
        }
        
        let data = if include_urls {
            json!({
                "ports": port_map,
                "urls": url_map,
            })
        } else {
            json!({
                "ports": port_map,
            })
        };
        
        Ok(data)
    }

    async fn query_performance_metrics(&self, process_filter: Option<Vec<String>>, metrics: Vec<String>, current_dir: bool) -> Result<Value> {
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let all_processes = self.process_manager.list_processes().await?;
        let mut processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            all_processes
        };
        
        // Apply process filter if provided
        if let Some(filter) = &process_filter {
            processes = processes.into_iter()
                .filter(|p| filter.contains(&p.name))
                .collect();
        }
        
        let include_cpu = metrics.contains(&"cpu".to_string());
        let include_memory = metrics.contains(&"memory".to_string());
        
        let mut metrics_data = serde_json::Map::new();
        let mut high_cpu_processes = Vec::new();
        let mut high_memory_processes = Vec::new();
        
        for process in &processes {
            let mut process_metrics = serde_json::Map::new();
            
            if include_cpu {
                process_metrics.insert("cpu_percent".to_string(), process.cpu_percent.into());
                if process.cpu_percent.unwrap_or(0.0) > 80.0 {
                    high_cpu_processes.push(&process.name);
                }
            }
            
            if include_memory {
                process_metrics.insert("memory_mb".to_string(), process.memory_mb.into());
                if process.memory_mb.unwrap_or(0) > 1000 {
                    high_memory_processes.push(&process.name);
                }
            }
            
            process_metrics.insert("uptime_seconds".to_string(), process.uptime_seconds.into());
            metrics_data.insert(process.name.clone(), process_metrics.into());
        }
        
        Ok(json!({
            "metrics": metrics_data,
            "alerts": {
                "high_cpu": high_cpu_processes,
                "high_memory": high_memory_processes,
            }
        }))
    }

    async fn query_log_search(&self, pattern: String, process_filter: Option<Vec<String>>, limit: Option<usize>, current_dir: bool) -> Result<Value> {
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let all_processes = self.process_manager.list_processes().await?;
        let mut processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            all_processes
        };
        
        // Apply process filter if provided
        if let Some(filter) = &process_filter {
            processes = processes.into_iter()
                .filter(|p| filter.contains(&p.name))
                .collect();
        }
        
        let mut matches = Vec::new();
        let search_limit = limit.unwrap_or(50);
        
        for process in &processes {
            let query = LogQuery {
                process_id: process.id.clone(),
                format: LogFormat::Raw,
                lines: Some(1000),
                search: Some(pattern.clone()),
                level: None,
                since: None,
            };
            
            if let Ok(logs) = self.log_storage.query(query).await {
                for log in logs.into_iter().take(search_limit.saturating_sub(matches.len())) {
                    matches.push(json!({
                        "process": process.name,
                        "timestamp": log.timestamp,
                        "line": log.clean_line,
                    }));
                    
                    if matches.len() >= search_limit {
                        break;
                    }
                }
            }
            
            if matches.len() >= search_limit {
                break;
            }
        }
        
        Ok(json!({
            "pattern": pattern,
            "matches": matches,
            "total_found": matches.len(),
        }))
    }

    async fn query_event_correlation(&self, event_types: Vec<String>, time_window: Option<String>, current_dir: bool) -> Result<Value> {
        let access_group = if current_dir {
            match std::env::current_dir() {
                Ok(cwd) => Some(crate::utils::access_group_from_dir(&cwd)),
                Err(e) => {
                    error!("Failed to get current directory: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let all_processes = self.process_manager.list_processes().await?;
        let processes: Vec<_> = if let Some(ref group) = access_group {
            all_processes
                .into_iter()
                .filter(|p| {
                    crate::utils::check_access(
                        Some(group),
                        p.access_group.as_deref(),
                        false,
                        &self.config.access_control.mode
                    )
                })
                .collect()
        } else {
            all_processes
        };
        
        let mut all_events = Vec::new();
        
        for process in &processes {
            if let Ok(summary) = self.log_storage.get_summary(&process.id).await {
                // Add key events if requested
                if event_types.contains(&"key_event".to_string()) {
                    for event in &summary.key_events {
                        all_events.push(json!({
                            "type": "key_event",
                            "process": process.name,
                            "message": event,
                            "timestamp": chrono::Utc::now().timestamp() // Would need actual timestamp
                        }));
                    }
                }
                
                // Add error count if requested
                if event_types.contains(&"error".to_string()) && summary.error_count > 0 {
                    all_events.push(json!({
                        "type": "error_summary",
                        "process": process.name,
                        "error_count": summary.error_count,
                        "message": format!("{} errors detected", summary.error_count),
                        "timestamp": chrono::Utc::now().timestamp() // Would need actual timestamp
                    }));
                }
            }
        }
        
        // Sort by timestamp (would need actual timestamps)
        // all_events.sort_by_key(|e| e["timestamp"].as_i64().unwrap_or(0));
        
        Ok(json!({
            "events": all_events,
            "event_types": event_types,
            "time_window": time_window.as_deref().unwrap_or("all_time"),
        }))
    }
}

impl ServerHandler for McpServerHandler {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::LATEST,
            capabilities: rmcp::model::ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability::default()),
                ..Default::default()
            },
            server_info: rmcp::model::Implementation {
                name: "agent-process-manager".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(r#"Agent Process Manager MCP server for managing background processes.

When searching for logs, you have three options:

1. **logs** tool - For basic filtering within a SINGLE known process:
   - Use when you have a specific process_id
   - Good for recent logs, simple text filtering
   - Real-time streaming available
   - Limited to 1000 logs per query

2. **search** tool - For powerful full-text search across ALL processes:
   - Use when you DON'T know which process has the logs
   - Use for complex queries with AND/OR/NOT logic
   - Use when you need fuzzy matching for typos
   - Use for finding exact phrases with quotes
   - Use for time-based searches (last hour, specific date range)
   - Much faster than logs tool for large searches
   - Returns relevance-ranked results with scores

3. **query** tool with type "log_search" - Fallback when search is disabled:
   - Simple pattern matching only
   - Use only if search tool returns "not enabled" error

Common search patterns:
- User asks "find all errors" → search(query: "error")
- User asks "find database connection issues" → search(query: "database AND connection")
- User asks "what errors happened in the last hour" → search(query: "error", since: <1 hour ago>)
- User asks "find timeout errors but not retries" → search(query: "timeout AND error NOT retry")
- User asks "search for connection (might be misspelled)" → search(query: "conection~")
- User asks "find exact phrase 'connection refused'" → search(query: "\"connection refused\"")
- User asks "find all warnings in process 12345" → search(query: "*", level: "warn", process_id: "12345")
- User asks "debug why app crashed" → search(query: "error OR exception OR crashed OR failed", limit: 100)"#.into()),
        }
    }
    fn list_tools<'a>(
        &'a self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + 'a {
        async move {
        let tools = vec![
            Tool {
                name: Cow::Borrowed("spawn"),
                description: Some(Cow::Borrowed("Spawn a new process")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Process name"
                        },
                        "command": {
                            "type": "string",
                            "description": "Command to execute"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Command arguments"
                        }
                    },
                    "required": ["name", "command"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("list"),
                description: Some(Cow::Borrowed("List processes with health metrics. Returns: id, name, command, status, pid (shell), actual_pid (real process), actual_name (real command), cpu_percent, memory_mb, detected_ports. The actual_pid and actual_name fields show the real running process inside tmux sessions (e.g., 'node', 'python') rather than just the shell.")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "current_dir": {
                            "type": "boolean",
                            "description": "Filter to only show processes from current directory",
                            "default": false
                        }
                    }
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("logs"),
                description: Some(Cow::Borrowed("Get process logs")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "process_id": {
                            "type": "string",
                            "description": "Process ID"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of log entries"
                        },
                        "search": {
                            "type": "string",
                            "description": "Search pattern to filter logs"
                        },
                        "level": {
                            "type": "string",
                            "enum": ["debug", "info", "warn", "error"],
                            "description": "Filter by log level"
                        },
                        "since": {
                            "type": "string",
                            "description": "RFC3339 timestamp to get logs since"
                        }
                    },
                    "required": ["process_id"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("kill"),
                description: Some(Cow::Borrowed("Kill a process (terminate tmux session)")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "process_id": {
                            "type": "string",
                            "description": "Process ID to kill"
                        }
                    },
                    "required": ["process_id"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("restart"),
                description: Some(Cow::Borrowed("Restart a process")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "process_id": {
                            "type": "string",
                            "description": "Process ID to restart"
                        }
                    },
                    "required": ["process_id"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("kill_multiple"),
                description: Some(Cow::Borrowed("Kill multiple processes based on filters")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "current_dir": {
                            "type": "boolean",
                            "description": "Only kill processes from current directory",
                            "default": false
                        },
                        "names": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of process names to kill"
                        },
                        "force": {
                            "type": "boolean",
                            "description": "Skip confirmation",
                            "default": false
                        }
                    }
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("clean"),
                description: Some(Cow::Borrowed("Clean stopped processes")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "older_than": {
                            "type": "integer",
                            "description": "Clean processes stopped more than N hours ago"
                        },
                        "keep_logs": {
                            "type": "boolean",
                            "description": "Keep logs when cleaning processes",
                            "default": false
                        },
                        "current_dir": {
                            "type": "boolean",
                            "description": "Only clean processes from current directory",
                            "default": false
                        }
                    }
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("query"),
                description: Some(Cow::Borrowed("Execute structured queries for AI agents")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["system_overview", "process_errors", "port_mapping", "performance_metrics", "log_search", "event_correlation"],
                            "description": "Query type"
                        },
                        "time_window": {
                            "type": "string",
                            "description": "Time window (e.g., '5m', '1h')"
                        },
                        "process_filter": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Filter by process names"
                        },
                        "include_urls": {
                            "type": "boolean",
                            "description": "Include URLs in port_mapping query"
                        },
                        "metrics": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Metrics to include (cpu, memory)"
                        },
                        "pattern": {
                            "type": "string",
                            "description": "Search pattern for log_search query"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Limit results for log_search query"
                        },
                        "event_types": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Event types for event_correlation query"
                        },
                        "min_severity": {
                            "type": "string",
                            "description": "Minimum severity for process_errors query"
                        },
                        "current_dir": {
                            "type": "boolean",
                            "description": "Filter to only show processes from current directory",
                            "default": false
                        }
                    },
                    "required": ["type"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("search"),
                description: Some(Cow::Borrowed(r#"Powerful full-text search across all process logs with advanced capabilities:
• Find logs across ALL processes or filter by specific process_id
• Search for exact phrases: "connection timeout"
• Boolean queries: error AND timeout, database OR cache, error NOT retry
• Fuzzy search for typos: databse~ finds "database"
• Wildcard patterns: time*, *error, dat?base
• Filter by log level (error, warn, info, debug)
• Time range filtering with since/until timestamps
• Returns relevance-ranked results with scores
• Much faster than grep for large-scale searches
• Supports complex queries like: (timeout OR refused) AND database NOT "retry succeeded"
Example: Find all database connection errors in the last hour across all processes"#)),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query with rich syntax: 'error' (simple), 'database AND connection' (boolean), '\"exact phrase\"' (phrases), 'databse~' (fuzzy), 'time*' (wildcard), '(timeout OR refused) NOT retry' (complex)"
                        },
                        "process_id": {
                            "type": "string",
                            "description": "Filter results to a specific process ID (e.g., '12345'). Omit to search across ALL processes"
                        },
                        "level": {
                            "type": "string",
                            "enum": ["debug", "info", "warn", "error"],
                            "description": "Filter by log level. Useful for finding only errors or warnings"
                        },
                        "since": {
                            "type": "string",
                            "description": "Start time for search range (RFC3339 format, e.g., '2024-01-01T10:00:00Z'). Great for recent events"
                        },
                        "until": {
                            "type": "string",
                            "description": "End time for search range (RFC3339 format). Use with 'since' to search specific time windows"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results (default: 50, max: 1000)",
                            "default": 50
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Offset for pagination"
                        },
                        "highlight": {
                            "type": "boolean",
                            "description": "Include highlighted snippets with search terms emphasized"
                        }
                    },
                    "required": ["query"]
                })).unwrap()),
                annotations: None,
            },
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
        }
    }

    fn call_tool<'a>(
        &'a self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + 'a {
        async move {
        debug!("MCP tool called: {} with args: {:?}", request.name, request.arguments);

        let result = match request.name.as_ref() {
            "spawn" => {
                let args: SpawnArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid spawn arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing spawn arguments".to_string()));
                };
                self.handle_spawn(args).await
            }
            "list" => {
                let args: ListArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid list arguments: {}", e), None)
                    })?
                } else {
                    ListArgs { current_dir: false }
                };
                self.handle_list(args).await
            }
            "logs" => {
                let args: LogsArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid logs arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing logs arguments".to_string()));
                };
                self.handle_logs(args).await
            }
            "kill" => {
                let args: KillArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid kill arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing kill arguments".to_string()));
                };
                self.handle_kill(args).await
            }
            "restart" => {
                let args: RestartArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid restart arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing restart arguments".to_string()));
                };
                self.handle_restart(args).await
            }
            "kill_multiple" => {
                let args: KillMultipleArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid kill_multiple arguments: {}", e), None)
                    })?
                } else {
                    KillMultipleArgs { current_dir: false, names: None, force: false }
                };
                self.handle_kill_multiple(args).await
            }
            "clean" => {
                let args: CleanArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid clean arguments: {}", e), None)
                    })?
                } else {
                    CleanArgs { older_than: None, keep_logs: false, current_dir: false }
                };
                self.handle_clean(args).await
            }
            "query" => {
                let args: QueryArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid query arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing query arguments".to_string()));
                };
                self.handle_query(args).await
            }
            "search" => {
                let args: SearchArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid search arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing search arguments".to_string()));
                };
                self.handle_search(args).await
            }
            _ => Self::create_error_result(format!("Unknown tool: {}", request.name)),
        };

        Ok(result)
        }
    }
}