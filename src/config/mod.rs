//! Configuration management

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub ui: UiConfig,
    #[serde(default)]
    pub patterns: PatternConfig,
    pub mcp: McpConfig,
    #[serde(default)]
    pub access_control: AccessControlConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub cleanup: CleanupConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub database_url: String,
    pub log_retention_days: u32,
    pub max_log_size_mb: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiConfig {
    pub theme: String,
    pub dashboard_auth: AuthMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    None,
    Basic,
    Oauth,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PatternConfig {
    #[serde(default)]
    pub custom: Vec<CustomPattern>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Tcp,
    UnixSocket,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    pub enabled: bool,
    #[serde(default = "default_mcp_transport")]
    pub transport: McpTransport,
    #[serde(default = "default_tcp_host")]
    pub tcp_host: String,
    #[serde(default = "default_tcp_port")]
    pub tcp_port: u16,
    #[serde(default = "default_unix_socket")]
    pub unix_socket: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AccessControlMode {
    /// Open read access, hierarchical write access (default)
    Open,
    /// Both read and write require hierarchical access
    Strict,
    /// Full read/write access to all processes regardless of directory
    Unrestricted,
}

impl Default for AccessControlMode {
    fn default() -> Self {
        AccessControlMode::Open
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AccessControlConfig {
    /// Access control mode
    #[serde(default = "default_access_control_mode")]
    pub mode: AccessControlMode,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            mode: AccessControlMode::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CleanupConfig {
    /// Automatically clean stopped processes on startup
    #[serde(default)]
    pub auto_clean_on_startup: bool,
    /// Retention period in hours for stopped processes (0 = keep forever)
    #[serde(default = "default_retention_hours")]
    pub retention_hours: u64,
    /// Keep logs when auto-cleaning
    #[serde(default = "default_keep_logs")]
    pub keep_logs: bool,
    /// Keep processes that failed/crashed (non-zero exit code)
    #[serde(default = "default_keep_failed")]
    pub keep_failed: bool,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            auto_clean_on_startup: false,
            retention_hours: default_retention_hours(),
            keep_logs: default_keep_logs(),
            keep_failed: default_keep_failed(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchConfig {
    /// Enable full-text search indexing
    #[serde(default = "default_search_enabled")]
    pub enabled: bool,
    /// Path to search index directory
    #[serde(default = "default_search_index_path")]
    pub index_path: String,
    /// How often to commit search index (seconds)
    #[serde(default = "default_search_commit_interval")]
    pub commit_interval_seconds: u64,
    /// Index writer buffer size in MB
    #[serde(default = "default_search_buffer_size")]
    pub buffer_size_mb: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,  // Default to enabled
            index_path: "./apm_search_index".to_string(),
            commit_interval_seconds: 5,
            buffer_size_mb: 50,
        }
    }
}

fn default_mcp_transport() -> McpTransport {
    McpTransport::Tcp
}

fn default_tcp_host() -> String {
    "127.0.0.1".to_string()
}

fn default_tcp_port() -> u16 {
    7338
}

fn default_unix_socket() -> String {
    "/tmp/apm.sock".to_string()
}

fn default_access_control_mode() -> AccessControlMode {
    AccessControlMode::Open
}

fn default_search_enabled() -> bool {
    true
}

fn default_search_index_path() -> String {
    "./apm_search_index".to_string()
}

fn default_search_commit_interval() -> u64 {
    5
}

fn default_search_buffer_size() -> usize {
    50
}

fn default_retention_hours() -> u64 {
    168 // 7 days
}

fn default_keep_logs() -> bool {
    false
}

fn default_keep_failed() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 7337,
            },
            storage: StorageConfig {
                database_url: "sqlite:apm.db".to_string(),
                log_retention_days: 7,
                max_log_size_mb: 1000,
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                dashboard_auth: AuthMode::None,
            },
            patterns: PatternConfig {
                custom: vec![],
            },
            mcp: McpConfig {
                enabled: false,
                transport: McpTransport::Tcp,
                tcp_host: "127.0.0.1".to_string(),
                tcp_port: 7338,
                unix_socket: "/tmp/apm.sock".to_string(),
            },
            access_control: AccessControlConfig::default(),
            search: SearchConfig::default(),
            cleanup: CleanupConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        // Check if config file exists
        if std::path::Path::new("apm.yaml").exists() {
            eprintln!("Found apm.yaml in current directory");
        } else {
            eprintln!("apm.yaml not found in current directory");
        }
        
        let builder = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 7337)?
            .set_default("storage.database_url", "sqlite:apm.db")?
            .set_default("storage.log_retention_days", 7)?
            .set_default("storage.max_log_size_mb", 1000)?
            .set_default("ui.theme", "dark")?
            .set_default("ui.dashboard_auth", "none")?
            .set_default("mcp.enabled", false)?
            .set_default("mcp.transport", "tcp")?
            .set_default("mcp.tcp_host", "127.0.0.1")?
            .set_default("mcp.tcp_port", 7338)?
            .set_default("mcp.unix_socket", "/tmp/apm.sock")?
            .set_default("access_control.mode", "open")?
            .set_default("search.enabled", true)?
            .set_default("search.index_path", "./apm_search_index")?
            .set_default("search.commit_interval_seconds", 5)?
            .set_default("search.buffer_size_mb", 50)?
            .set_default("cleanup.auto_clean_on_startup", false)?
            .set_default("cleanup.retention_hours", 168)?
            .set_default("cleanup.keep_logs", false)?
            .set_default("cleanup.keep_failed", true)?
            .add_source(config::File::from(std::path::Path::new("apm.yaml")).required(false))
            .add_source(config::Environment::with_prefix("APM"))
            .build()?;

        let config = builder;
        
        // Debug what we got
        if let Ok(mcp_enabled) = config.get_bool("mcp.enabled") {
            eprintln!("Config has mcp.enabled = {}", mcp_enabled);
        }

        config.try_deserialize()
    }

    pub fn load_from_path(path: &str) -> Result<Self, config::ConfigError> {
        eprintln!("Loading config from: {}", path);
        
        let builder = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 7337)?
            .set_default("storage.database_url", "sqlite:apm.db")?
            .set_default("storage.log_retention_days", 7)?
            .set_default("storage.max_log_size_mb", 1000)?
            .set_default("ui.theme", "dark")?
            .set_default("ui.dashboard_auth", "none")?
            .set_default("mcp.enabled", false)?
            .set_default("mcp.transport", "tcp")?
            .set_default("mcp.tcp_host", "127.0.0.1")?
            .set_default("mcp.tcp_port", 7338)?
            .set_default("mcp.unix_socket", "/tmp/apm.sock")?
            .set_default("access_control.mode", "open")?
            .set_default("search.enabled", true)?
            .set_default("search.index_path", "./apm_search_index")?
            .set_default("search.commit_interval_seconds", 5)?
            .set_default("search.buffer_size_mb", 50)?
            .set_default("cleanup.auto_clean_on_startup", false)?
            .set_default("cleanup.retention_hours", 168)?
            .set_default("cleanup.keep_logs", false)?
            .set_default("cleanup.keep_failed", true)?
            .add_source(config::File::from(std::path::Path::new(path)).required(true))
            .add_source(config::Environment::with_prefix("APM"))
            .build()?;

        config::Config::try_deserialize(builder)
    }
}