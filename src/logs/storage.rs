//! Log storage implementation with dual storage (raw + structured)

use crate::{Result, process::ProcessId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use std::sync::Arc;
use super::patterns::{PatternDetector, DetectedPattern};

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
}

use std::collections::HashMap;

impl LogStorage {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Ensure SQLite creates the database file if it doesn't exist
        let db_url = if database_url.starts_with("sqlite:") && !database_url.contains("?") {
            format!("{}?mode=rwc", database_url)
        } else {
            database_url.to_string()
        };
        
        let db = SqlitePool::connect(&db_url).await?;
        
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
        "#)
        .execute(&db)
        .await?;

        Ok(Self {
            db,
            pattern_detector: Arc::new(PatternDetector::new()),
            raw_buffers: Arc::new(RwLock::new(HashMap::new())),
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
        
        sqlx::query(r#"
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
        // For raw format, use memory buffer if available
        if matches!(query.format, LogFormat::Raw) {
            let buffers = self.raw_buffers.read().await;
            if let Some(buffer) = buffers.get(&query.process_id) {
                let lines: Vec<_> = buffer.iter()
                    .rev()
                    .take(query.lines.unwrap_or(100))
                    .rev()
                    .cloned()
                    .collect();
                
                let entries: Vec<_> = lines.into_iter().enumerate()
                    .map(|(i, line)| LogEntry {
                        id: i as i64,
                        process_id: query.process_id.clone(),
                        timestamp: Utc::now(), // Approximate
                        raw_line: line.clone(),
                        clean_line: line.clone(),
                        patterns: vec![],
                        level: LogLevel::Info,
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