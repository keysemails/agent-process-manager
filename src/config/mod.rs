//! Configuration management

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub ui: UiConfig,
    pub patterns: PatternConfig,
    pub mcp: McpConfig,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternConfig {
    pub custom: Vec<CustomPattern>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomPattern {
    pub name: String,
    pub pattern: String,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    pub enabled: bool,
    pub stdio: bool,
    pub tcp_host: Option<String>,
    pub tcp_port: Option<u16>,
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
                stdio: true,
                tcp_host: None,
                tcp_port: None,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .set_default("server.host", "0.0.0.0")?
            .set_default("server.port", 7337)?
            .set_default("storage.database_url", "sqlite:apm.db")?
            .set_default("storage.log_retention_days", 7)?
            .set_default("storage.max_log_size_mb", 1000)?
            .set_default("ui.theme", "dark")?
            .set_default("ui.dashboard_auth", "none")?
            .set_default("mcp.enabled", false)?
            .set_default("mcp.stdio", true)?
            .add_source(config::File::with_name("apm").required(false))
            .add_source(config::Environment::with_prefix("APM"))
            .build()?;

        config.try_deserialize()
    }
}