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
    
    /// Legacy field for backwards compatibility
    /// When present, overrides the mode setting
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolate_read_access: Option<bool>,
}

impl AccessControlConfig {
    /// Get the effective access control mode, handling backwards compatibility
    pub fn effective_mode(&self) -> AccessControlMode {
        // Legacy field takes precedence for backwards compatibility
        if let Some(isolate_read_access) = self.isolate_read_access {
            if isolate_read_access {
                AccessControlMode::Strict
            } else {
                AccessControlMode::Open
            }
        } else {
            self.mode.clone()
        }
    }
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            mode: AccessControlMode::default(),
            isolate_read_access: None,
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

fn default_isolate_read_access() -> bool {
    false  // Default: open read access for all agents
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
}