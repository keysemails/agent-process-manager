//! Log storage implementation with dual storage (raw + structured)

use crate::{Result, ApmError, process::{ProcessId, ProcessStatus, ProcessConfig}};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, Row};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use std::sync::Arc;
use super::patterns::{PatternDetector, DetectedPattern};
use super::search::LogSearchEngine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: i64,
    pub process_id: ProcessId,
    pub timestamp: DateTime<Utc>,
    pub raw_line: String,
    pub clean_line: String,
    pub patterns: Vec<DetectedPattern>,
    pub level: LogLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    pub process_id: ProcessId,
    pub format: LogFormat,
    pub lines: Option<usize>,
    pub search: Option<String>,
    pub level: Option<LogLevel>,
    pub since: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Raw,
    Summary,
    Errors,
    Json,
}

pub struct LogStorage {
    db: SqlitePool,
    pattern_detector: Arc<PatternDetector>,
    raw_buffers: Arc<RwLock<HashMap<ProcessId, VecDeque<String>>>>,
    search_engine: Option<Arc<LogSearchEngine>>,
}

use std::collections::HashMap;

impl LogStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_search(database_url, None).await
    }
    
    pub async fn new_with_search(database_url: &str, search_engine: Option<Arc<LogSearchEngine>>) -> Result<Self> {
        // Ensure SQLite creates the database file if it doesn't exist
        let db_url = if database_url.starts_with("sqlite:") && !database_url.contains("?") {
            format!("{}?mode=rwc", database_url)
        } else {
            database_url.to_string()
        };
        
        let db = SqlitePool::connect(&db_url).await?;
        
        // Set pragmas for better concurrent access
        // WAL mode is crucial for multiple processes accessing the same database
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&db)
            .await?;
        
        sqlx::query("PRAGMA busy_timeout = 5000")  // Wait up to 5 seconds if db is locked
            .execute(&db)
            .await?;
        
        sqlx::query("PRAGMA synchronous = NORMAL")  // Better performance while maintaining safety
            .execute(&db)
            .await?;
        
        // Create tables
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                process_id TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                raw_line TEXT NOT NULL,
                clean_line TEXT NOT NULL,
                patterns TEXT NOT NULL,
                level TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE INDEX IF NOT EXISTS idx_logs_process_timestamp 
            ON logs(process_id, timestamp DESC);
            
            CREATE INDEX IF NOT EXISTS idx_logs_level 
            ON logs(process_id, level);
            
            -- Process management table
            CREATE TABLE IF NOT EXISTS processes (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT,  -- JSON array
                status TEXT NOT NULL,
                started_at DATETIME NOT NULL,
                stopped_at DATETIME,
                session_pid INTEGER,    -- PID of the tmux session/shell
                process_pid INTEGER, -- PID of the actual application process
                process_name TEXT,   -- Name of the actual application process
                tmux_session TEXT,
                restart_count INTEGER DEFAULT 0,
                config TEXT NOT NULL,  -- Full ProcessConfig as JSON
                access_group TEXT,  -- Working directory based access group
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            
            CREATE INDEX IF NOT EXISTS idx_processes_status ON processes(status);
            CREATE INDEX IF NOT EXISTS idx_processes_name ON processes(name);
            CREATE INDEX IF NOT EXISTS idx_processes_tmux ON processes(tmux_session);
            CREATE INDEX IF NOT EXISTS idx_processes_access_group ON processes(access_group);
            
            -- Trigger to update updated_at on changes
            CREATE TRIGGER IF NOT EXISTS update_processes_timestamp 
            AFTER UPDATE ON processes
            BEGIN
                UPDATE processes SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
            END;
        "#)
        .execute(&db)
        .await?;

        // Add process_pid and process_name columns if they don't exist (for existing databases)
        let _ = sqlx::query("ALTER TABLE processes ADD COLUMN process_pid INTEGER")
            .execute(&db)
            .await;
        let _ = sqlx::query("ALTER TABLE processes ADD COLUMN process_name TEXT")
            .execute(&db)
            .await;
        
        // Rename old columns to new names for existing databases
        let _ = sqlx::query("ALTER TABLE processes RENAME COLUMN pid TO session_pid")
            .execute(&db)
            .await;
        let _ = sqlx::query("ALTER TABLE processes RENAME COLUMN actual_pid TO process_pid")
            .execute(&db)
            .await;
        let _ = sqlx::query("ALTER TABLE processes RENAME COLUMN actual_name TO process_name")
            .execute(&db)
            .await;

        Ok(Self {
            db,
            pattern_detector: Arc::new(PatternDetector::new()),
            raw_buffers: Arc::new(RwLock::new(HashMap::new())),
            search_engine,
        })
    }

    pub async fn store(&self, process_id: ProcessId, raw_line: String) -> Result<()> {
        // Strip ANSI codes for clean line
        let clean_line = String::from_utf8_lossy(&strip_ansi_escapes::strip(&raw_line)).to_string();

        // Detect patterns
        let patterns = self.pattern_detector.detect(&clean_line);
        
        // Determine log level
        let level = self.determine_level(&clean_line);

        // Store in raw buffer
        {
            let mut buffers = self.raw_buffers.write().await;
            let buffer = buffers.entry(process_id.clone()).or_insert_with(|| {
                VecDeque::with_capacity(10000)
            });
            
            // Keep last 10k lines in memory
            if buffer.len() >= 10000 {
                buffer.pop_front();
            }
            buffer.push_back(raw_line.clone());
        }

        // Store in database
        let patterns_json = serde_json::to_string(&patterns)?;
        let timestamp = Utc::now();
        
        let result = sqlx::query(r#"
            INSERT INTO logs (process_id, timestamp, raw_line, clean_line, patterns, level)
            VALUES (?, ?, ?, ?, ?, ?)
        "#)
        .bind(process_id.to_string())
        .bind(timestamp)
        .bind(&raw_line)
        .bind(&clean_line)
        .bind(patterns_json)
        .bind(format!("{:?}", level))
        .execute(&self.db)
        .await?;

        // Index the log entry if search engine is available
        if let Some(search_engine) = &self.search_engine {
            let log_entry = LogEntry {
                id: result.last_insert_rowid(),
                process_id: process_id.clone(),
                timestamp,
                raw_line: raw_line.clone(),
                clean_line,
                patterns,
                level,
            };
            
            // Index asynchronously - don't fail the store operation if indexing fails
            if let Err(e) = search_engine.index_log(&log_entry).await {
                tracing::warn!("Failed to index log entry: {}", e);
            }
        }

        Ok(())
    }

    fn determine_level(&self, line: &str) -> LogLevel {
        let lower = line.to_lowercase();
        if lower.contains("error") || lower.contains("failed") || lower.contains("exception") {
            LogLevel::Error
        } else if lower.contains("warn") || lower.contains("warning") {
            LogLevel::Warn
        } else if lower.contains("debug") {
            LogLevel::Debug
        } else {
            LogLevel::Info
        }
    }

    pub async fn query(&self, query: LogQuery) -> Result<Vec<LogEntry>> {
        // For raw format without filters, use memory buffer if available
        if matches!(query.format, LogFormat::Raw) && 
           query.level.is_none() && 
           query.search.is_none() && 
           query.since.is_none() {
            let buffers = self.raw_buffers.read().await;
            if let Some(buffer) = buffers.get(&query.process_id) {
                let lines: Vec<_> = buffer.iter()
                    .rev()
                    .take(query.lines.unwrap_or(100))
                    .rev()
                    .cloned()
                    .collect();
                
                let entries: Vec<_> = lines.into_iter().enumerate()
                    .map(|(i, line)| {
                        let clean_line = String::from_utf8_lossy(&strip_ansi_escapes::strip(&line)).to_string();
                        LogEntry {
                            id: i as i64,
                            process_id: query.process_id.clone(),
                            timestamp: Utc::now(), // Approximate
                            raw_line: line.clone(),
                            clean_line: clean_line.clone(),
                            patterns: vec![],
                            level: self.determine_level(&clean_line),
                        }
                    })
                    .collect();
                
                return Ok(entries);
            }
        }
        
        // Query from database
        use sqlx::sqlite::Sqlite;
        
        // Build dynamic query based on filters
        let rows = if let Some(level) = query.level {
            sqlx::query_as::<Sqlite, (i64, String, DateTime<Utc>, String, String, String, String)>(
                "SELECT id, process_id, timestamp, raw_line, clean_line, patterns, level 
                 FROM logs WHERE process_id = ? AND level = ? 
                 ORDER BY timestamp DESC LIMIT ?"
            )
            .bind(query.process_id.to_string())
            .bind(format!("{:?}", level))
            .bind(query.lines.unwrap_or(100) as i32)
            .fetch_all(&self.db)
            .await?
        } else {
            sqlx::query_as::<Sqlite, (i64, String, DateTime<Utc>, String, String, String, String)>(
                "SELECT id, process_id, timestamp, raw_line, clean_line, patterns, level 
                 FROM logs WHERE process_id = ? 
                 ORDER BY timestamp DESC LIMIT ?"
            )
            .bind(query.process_id.to_string())
            .bind(query.lines.unwrap_or(100) as i32)
            .fetch_all(&self.db)
            .await?
        };
        
        let entries: Vec<LogEntry> = rows.into_iter()
            .map(|(id, process_id, timestamp, raw_line, clean_line, patterns_json, level)| {
                let patterns: Vec<DetectedPattern> = serde_json::from_str(&patterns_json)
                    .unwrap_or_default();
                let level = match level.as_str() {
                    "Error" => LogLevel::Error,
                    "Warn" => LogLevel::Warn,
                    "Debug" => LogLevel::Debug,
                    _ => LogLevel::Info,
                };
                
                LogEntry {
                    id,
                    process_id: ProcessId(process_id.parse().unwrap()),
                    timestamp,
                    raw_line,
                    clean_line,
                    patterns,
                    level,
                }
            })
            .collect();
        
        Ok(entries)
    }

    pub async fn get_summary(&self, process_id: &ProcessId) -> Result<LogSummary> {
        // Query recent logs and generate summary
        let recent_logs = self.query(LogQuery {
            process_id: process_id.clone(),
            format: LogFormat::Summary,
            lines: Some(1000),
            search: None,
            level: None,
            since: Some(Utc::now() - chrono::Duration::minutes(5)),
        }).await?;

        let mut summary = LogSummary {
            total_lines: recent_logs.len(),
            error_count: 0,
            warning_count: 0,
            detected_urls: vec![],
            detected_ports: vec![],
            key_events: vec![],
        };

        for log in recent_logs {
            match log.level {
                LogLevel::Error => summary.error_count += 1,
                LogLevel::Warn => summary.warning_count += 1,
                _ => {}
            }

            for pattern in log.patterns {
                match pattern {
                    DetectedPattern::Url(url) => {
                        if !summary.detected_urls.contains(&url) {
                            summary.detected_urls.push(url);
                        }
                    }
                    DetectedPattern::Port(port) => {
                        if !summary.detected_ports.contains(&port) {
                            summary.detected_ports.push(port);
                        }
                    }
                    DetectedPattern::KeyEvent(event) => {
                        summary.key_events.push(event);
                    }
                    _ => {}
                }
            }
        }

        Ok(summary)
    }
    
    pub async fn get_raw_logs(&self, process_id: &ProcessId, limit: Option<usize>) -> Result<String> {
        let query = LogQuery {
            process_id: process_id.clone(),
            format: LogFormat::Raw,
            lines: limit,
            search: None,
            level: None,
            since: None,
        };
        
        let logs = self.query(query).await?;
        let raw_lines: Vec<String> = logs.into_iter()
            .map(|entry| entry.raw_line)
            .collect();
        
        Ok(raw_lines.join("\n"))
    }
    
    pub async fn clear_logs(&self, process_id: &ProcessId) -> Result<()> {
        // Clear from database
        sqlx::query("DELETE FROM logs WHERE process_id = ?")
            .bind(process_id.to_string())
            .execute(&self.db)
            .await?;
            
        // Clear from memory buffer
        let mut buffers = self.raw_buffers.write().await;
        buffers.remove(process_id);
        
        Ok(())
    }
    
    // Process management methods
    
    pub async fn store_process(&self, 
        id: &ProcessId, 
        name: &str,
        command: &str,
        args: &[String],
        status: ProcessStatus,
        config: &ProcessConfig,
        tmux_session: Option<&str>
    ) -> Result<()> {
        let args_json = serde_json::to_string(args)?;
        let config_json = serde_json::to_string(config)?;
        let status_str = serde_json::to_string(&status)?;
        let status_str = status_str.trim_matches('"');
        
        sqlx::query(r#"
            INSERT INTO processes (id, name, command, args, status, started_at, config, tmux_session, access_group)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                config = excluded.config,
                tmux_session = excluded.tmux_session,
                access_group = excluded.access_group,
                updated_at = CURRENT_TIMESTAMP
        "#)
        .bind(id.to_string())
        .bind(name)
        .bind(command)
        .bind(args_json)
        .bind(status_str)
        .bind(Utc::now())
        .bind(config_json)
        .bind(tmux_session)
        .bind(&config.access_group)
        .execute(&self.db)
        .await?;
        
        Ok(())
    }
    
    pub async fn update_process_status(&self, id: &ProcessId, status: ProcessStatus, pid: Option<u32>) -> Result<()> {
        let status_str = serde_json::to_string(&status)?;
        let status_str = status_str.trim_matches('"');
        
        if status == ProcessStatus::Stopped || status == ProcessStatus::Failed || status == ProcessStatus::Killed {
            sqlx::query(r#"
                UPDATE processes 
                SET status = ?, session_pid = ?, stopped_at = ?, updated_at = CURRENT_TIMESTAMP 
                WHERE id = ?
            "#)
            .bind(status_str)
            .bind(pid.map(|p| p as i64))
            .bind(Utc::now())
            .bind(id.to_string())
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(r#"
                UPDATE processes 
                SET status = ?, session_pid = ?, updated_at = CURRENT_TIMESTAMP 
                WHERE id = ?
            "#)
            .bind(status_str)
            .bind(pid.map(|p| p as i64))
            .bind(id.to_string())
            .execute(&self.db)
            .await?;
        }
        
        Ok(())
    }

    /// Update the actual application process PID (not the session PID) for a process
    pub async fn update_process_pid(&self, id: &ProcessId, process_pid: u32, process_name: Option<&str>) -> Result<()> {
        sqlx::query(r#"
            UPDATE processes 
            SET process_pid = ?, process_name = ?, updated_at = CURRENT_TIMESTAMP 
            WHERE id = ?
        "#)
        .bind(process_pid as i64)
        .bind(process_name)
        .bind(id.to_string())
        .execute(&self.db)
        .await?;
        
        Ok(())
    }
    
    pub async fn get_process(&self, id: &ProcessId) -> Result<Option<ProcessRecord>> {
        let row = sqlx::query(
            "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
             FROM processes WHERE id = ?"
        )
        .bind(id.to_string())
        .fetch_optional(&self.db)
        .await?;
        
        if let Some(row) = row {
            Ok(Some(ProcessRecord::from_row(row)?))
        } else {
            Ok(None)
        }
    }
    
    pub async fn list_processes(&self, status_filter: Option<ProcessStatus>) -> Result<Vec<ProcessRecord>> {
        let query = if let Some(status) = status_filter {
            let status_str = serde_json::to_string(&status)?;
            let status_str = status_str.trim_matches('"').to_string();
            sqlx::query(
                "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                 FROM processes WHERE status = ? ORDER BY started_at DESC"
            )
            .bind(status_str)
        } else {
            sqlx::query(
                "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                 FROM processes ORDER BY started_at DESC"
            )
        };
        
        let rows = query.fetch_all(&self.db).await?;
        let mut processes = Vec::new();
        
        for row in rows {
            processes.push(ProcessRecord::from_row(row)?);
        }
        
        Ok(processes)
    }
    
    pub async fn list_processes_with_access_filter(&self, access_group: Option<&str>, status_filter: Option<ProcessStatus>) -> Result<Vec<ProcessRecord>> {
        let query = match (access_group, status_filter) {
            (Some(group), Some(status)) => {
                let status_str = serde_json::to_string(&status)?;
                let status_str = status_str.trim_matches('"').to_string();
                sqlx::query(
                    "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                     FROM processes WHERE access_group = ? AND status = ? ORDER BY started_at DESC"
                )
                .bind(group)
                .bind(status_str)
            }
            (Some(group), None) => {
                sqlx::query(
                    "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                     FROM processes WHERE access_group = ? ORDER BY started_at DESC"
                )
                .bind(group)
            }
            (None, Some(status)) => {
                let status_str = serde_json::to_string(&status)?;
                let status_str = status_str.trim_matches('"').to_string();
                sqlx::query(
                    "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                     FROM processes WHERE status = ? ORDER BY started_at DESC"
                )
                .bind(status_str)
            }
            (None, None) => {
                sqlx::query(
                    "SELECT id, name, command, args, status, started_at, stopped_at, session_pid, process_pid, process_name, tmux_session, restart_count, config 
                     FROM processes ORDER BY started_at DESC"
                )
            }
        };
        
        let rows = query.fetch_all(&self.db).await?;
        let mut processes = Vec::new();
        
        for row in rows {
            processes.push(ProcessRecord::from_row(row)?);
        }
        
        Ok(processes)
    }
    
    pub async fn delete_process(&self, id: &ProcessId) -> Result<()> {
        sqlx::query("DELETE FROM processes WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.db)
            .await?;
        
        Ok(())
    }
    
    pub async fn clean_stopped_processes(
        &self,
        older_than_hours: Option<u64>,
        access_group: Option<&str>,
        keep_logs: bool,
    ) -> Result<Vec<(ProcessId, String)>> {
        // Build the query based on parameters
        let mut query = String::from("SELECT id, name FROM processes WHERE status = 'Stopped'");
        let mut bindings = vec![];
        
        // Add time filter if specified
        if let Some(hours) = older_than_hours {
            query.push_str(" AND stopped_at <= datetime('now', ?)");
            bindings.push(format!("-{} hours", hours));
        }
        
        // Add access group filter if specified
        if let Some(group) = access_group {
            query.push_str(" AND access_group = ?");
            bindings.push(group.to_string());
        }
        
        // Execute query to get processes to delete
        let mut sql_query = sqlx::query(&query);
        for binding in &bindings {
            sql_query = sql_query.bind(binding);
        }
        
        let rows = sql_query.fetch_all(&self.db).await?;
        let mut deleted = Vec::new();
        
        for row in rows {
            let id_str: String = row.try_get("id")?;
            let name: String = row.try_get("name")?;
            let process_id = ProcessId(id_str.parse().map_err(|e: uuid::Error| 
                ApmError::Process(format!("Invalid process ID: {}", e)))?);
            
            if !keep_logs {
                // Delete log entries for this process (ignore if table doesn't exist)
                let _ = sqlx::query("DELETE FROM log_entries WHERE process_id = ?")
                    .bind(id_str.clone())
                    .execute(&self.db)
                    .await;
            }
            
            // Delete the process record
            sqlx::query("DELETE FROM processes WHERE id = ?")
                .bind(id_str)
                .execute(&self.db)
                .await?;
                
            deleted.push((process_id, name));
        }
        
        Ok(deleted)
    }
    
    pub async fn increment_restart_count(&self, id: &ProcessId) -> Result<()> {
        sqlx::query(
            "UPDATE processes SET restart_count = restart_count + 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(id.to_string())
        .execute(&self.db)
        .await?;
        
        Ok(())
    }
    
    pub async fn recover_orphaned_tmux_sessions(&self) -> Result<Vec<ProcessId>> {
        use crate::tmux::TmuxManager;
        
        let mut recovered = Vec::new();
        
        // Get all known tmux sessions from DB
        let known_sessions: Vec<String> = sqlx::query_scalar(
            "SELECT tmux_session FROM processes WHERE tmux_session IS NOT NULL"
        )
        .fetch_all(&self.db)
        .await?;
        
        // List all tmux sessions
        if let Ok(tmux_sessions) = TmuxManager::list_sessions() {
            for session in tmux_sessions {
                if session.starts_with("apm-") && !known_sessions.contains(&session) {
                    // Extract process ID from session name
                    if let Some(uuid_str) = session.strip_prefix("apm-") {
                        if let Ok(uuid) = uuid_str.parse::<uuid::Uuid>() {
                            let process_id = ProcessId(uuid);
                            
                            // Try to get process info from tmux
                            let pid = TmuxManager::get_session_pid(&session).ok();
                            
                            // Try to recover metadata from tmux session
                            let name = TmuxManager::get_session_metadata(&session, "apm_process_name")
                                .unwrap_or_else(|_| format!("recovered-{}", &uuid_str[..8]));
                            let command = TmuxManager::get_session_metadata(&session, "apm_command")
                                .unwrap_or_else(|_| "unknown".to_string());
                            let args_json = TmuxManager::get_session_metadata(&session, "apm_args")
                                .unwrap_or_else(|_| "[]".to_string());
                            let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                            
                            // Create a process record with recovered metadata
                            let config = ProcessConfig {
                                name,
                                command,
                                args,
                                cwd: None,
                                env: std::collections::HashMap::new(),
                                tags: vec!["recovered".to_string()],
                                pty: false,
                                use_tmux: true,
                                restart_policy: Default::default(),
                                resources: Default::default(),
                                access_group: None, // No access group for recovered sessions
                            };
                            
                            // Store the recovered process
                            self.store_process(
                                &process_id,
                                &config.name,
                                &config.command,
                                &config.args,
                                ProcessStatus::Running,
                                &config,
                                Some(&session)
                            ).await?;
                            
                            if let Some(pid) = pid {
                                self.update_process_status(&process_id, ProcessStatus::Running, Some(pid)).await?;
                            }
                            
                            recovered.push(process_id);
                        }
                    }
                }
            }
        }
        
        Ok(recovered)
    }
    
    /// Commit the search index to make recent logs searchable
    pub async fn commit_search_index(&self) -> Result<()> {
        if let Some(search_engine) = &self.search_engine {
            search_engine.commit().await?;
        }
        Ok(())
    }
    
    /// Get statistics about the search index
    pub async fn get_search_stats(&self) -> Result<Option<super::search::IndexStats>> {
        if let Some(search_engine) = &self.search_engine {
            Ok(Some(search_engine.get_stats().await?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRecord {
    pub id: ProcessId,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: ProcessStatus,
    pub started_at: DateTime<Utc>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub session_pid: Option<u32>,
    pub process_pid: Option<u32>,
    pub process_name: Option<String>,
    pub tmux_session: Option<String>,
    pub restart_count: u32,
    pub config: ProcessConfig,
}

impl ProcessRecord {
    fn from_row(row: sqlx::sqlite::SqliteRow) -> Result<Self> {
        use crate::ApmError;
        
        let id_str: String = row.try_get("id")?;
        let id = ProcessId(id_str.parse().map_err(|_| ApmError::ProcessError("Invalid UUID".into()))?);
        
        let args_json: String = row.try_get("args")?;
        let args: Vec<String> = serde_json::from_str(&args_json)?;
        
        let status_str: String = row.try_get("status")?;
        let status: ProcessStatus = serde_json::from_str(&format!("\"{}\"", status_str))?;
        
        let config_json: String = row.try_get("config")?;
        let config: ProcessConfig = serde_json::from_str(&config_json)?;
        
        let session_pid: Option<i64> = row.try_get("session_pid")?;
        let process_pid: Option<i64> = row.try_get("process_pid")?;
        
        Ok(ProcessRecord {
            id,
            name: row.try_get("name")?,
            command: row.try_get("command")?,
            args,
            status,
            started_at: row.try_get("started_at")?,
            stopped_at: row.try_get("stopped_at")?,
            session_pid: session_pid.map(|p| p as u32),
            process_pid: process_pid.map(|p| p as u32),
            process_name: row.try_get("process_name")?,
            tmux_session: row.try_get("tmux_session")?,
            restart_count: row.try_get::<i64, _>("restart_count")? as u32,
            config,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSummary {
    pub total_lines: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub detected_urls: Vec<String>,
    pub detected_ports: Vec<u16>,
    pub key_events: Vec<String>,
}