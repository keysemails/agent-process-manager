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
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Serialize, Deserialize)]
struct StopArgs {
    process_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct QueryArgs {
    #[serde(rename = "type")]
    query_type: String,
    #[serde(default)]
    time_window: Option<String>,
}

impl McpServer {
    pub async fn new(
        process_manager: Arc<ProcessManager>,
        log_storage: Arc<LogStorage>,
    ) -> Result<Self> {
        let handler = McpServerHandler {
            process_manager,
            log_storage,
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

        let config = ProcessConfig {
            name: args.name.clone(),
            command: args.command,
            args: args.args,
            cwd: None,
            env: HashMap::new(),
            tags: vec![],
            pty: false,
            use_tmux: true,
            restart_policy: Default::default(),
            resources: Default::default(),
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

    async fn handle_list(&self) -> CallToolResult {
        debug!("MCP list tool called");

        match self.process_manager.list_processes().await {
            Ok(processes) => {
                let process_list: Vec<Value> = processes
                    .into_iter()
                    .map(|info| {
                        json!({
                            "id": info.id.to_string(),
                            "name": info.name,
                            "command": info.command,
                            "status": info.status,
                            "created_at": info.started_at,
                            "pid": info.pid,
                            "uptime_seconds": info.uptime_seconds,
                            "restart_count": info.restart_count,
                            "cpu_percent": info.cpu_percent,
                            "memory_mb": info.memory_mb
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

        let query = LogQuery {
            process_id: process_id.clone(),
            format: LogFormat::Json,
            lines: Some(args.limit),
            search: None,
            level: None,
            since: None,
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

    async fn handle_stop(&self, args: StopArgs) -> CallToolResult {
        debug!("MCP stop tool called: {:?}", args);

        let process_id = match args.process_id.parse() {
            Ok(id) => ProcessId(id),
            Err(_) => {
                return Self::create_error_result("Invalid process ID".to_string());
            }
        };

        match self.process_manager.stop_process(&process_id).await {
            Ok(()) => {
                let response = json!({
                    "success": true,
                    "message": format!("Process '{}' stopped successfully", process_id)
                });
                Self::create_success_result(response.to_string())
            }
            Err(e) => {
                error!("Failed to stop process: {}", e);
                Self::create_error_result(format!("Failed to stop process: {}", e))
            }
        }
    }

    async fn handle_query(&self, args: QueryArgs) -> CallToolResult {
        debug!("MCP query tool called: {:?}", args);

        let result = match args.query_type.as_str() {
            "system_overview" => self.query_system_overview().await,
            "process_errors" => self.query_process_errors(args.time_window).await,
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
    async fn query_system_overview(&self) -> Result<Value> {
        let processes = self.process_manager.list_processes().await?;
        let overview = json!({
            "total_processes": processes.len(),
            "running": processes.iter().filter(|p| p.status == crate::process::ProcessStatus::Running).count(),
            "stopped": processes.iter().filter(|p| p.status == crate::process::ProcessStatus::Stopped).count(),
            "processes": processes
        });
        Ok(overview)
    }

    async fn query_process_errors(&self, _time_window: Option<String>) -> Result<Value> {
        let mut all_errors = Vec::new();
        let processes = self.process_manager.list_processes().await?;

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
            instructions: Some("Agent Process Manager MCP server for managing background processes".into()),
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
                description: Some(Cow::Borrowed("List all processes")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {}
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
                        }
                    },
                    "required": ["process_id"]
                })).unwrap()),
                annotations: None,
            },
            Tool {
                name: Cow::Borrowed("stop"),
                description: Some(Cow::Borrowed("Stop a process")),
                input_schema: Arc::new(serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "process_id": {
                            "type": "string",
                            "description": "Process ID to stop"
                        }
                    },
                    "required": ["process_id"]
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
                        "filter": {
                            "type": "object",
                            "description": "Additional filters"
                        }
                    },
                    "required": ["type"]
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
            "list" => self.handle_list().await,
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
            "stop" => {
                let args: StopArgs = if let Some(args) = request.arguments {
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        McpError::invalid_params(format!("Invalid stop arguments: {}", e), None)
                    })?
                } else {
                    return Ok(Self::create_error_result("Missing stop arguments".to_string()));
                };
                self.handle_stop(args).await
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
            _ => Self::create_error_result(format!("Unknown tool: {}", request.name)),
        };

        Ok(result)
        }
    }
}