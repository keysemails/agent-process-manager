//! Log summarization for AI-friendly output

use super::{LogEntry, LogLevel};
use crate::process::{ProcessStatus, ProcessInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSummary {
    pub status: ProcessStatus,
    pub uptime: String,
    pub key_events: Vec<String>,
    pub recent_errors: Vec<String>,
    pub detected_urls: Vec<String>,
    pub detected_ports: Vec<u16>,
    pub resource_usage: ResourceUsage,
    pub error_patterns: Vec<ErrorPattern>,
    pub metrics: SummaryMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: String,
    pub memory_mb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPattern {
    pub pattern: String,
    pub count: usize,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetrics {
    pub total_logs: usize,
    pub error_rate: f64,  // Errors per minute
    pub warning_rate: f64, // Warnings per minute
    pub log_velocity: f64, // Logs per minute
    pub time_span_minutes: f64,
}

pub struct LogSummarizer;

impl LogSummarizer {
    pub fn new() -> Self {
        Self
    }

    pub fn summarize_logs(&self, logs: &[LogEntry], process_info: Option<&ProcessInfo>) -> ProcessSummary {
        let mut key_events = Vec::new();
        let mut recent_errors = Vec::new();
        let mut detected_urls = Vec::new();
        let mut detected_ports = Vec::new();
        let mut error_pattern_map: HashMap<String, (usize, DateTime<Utc>, DateTime<Utc>)> = HashMap::new();
        
        // Calculate time span
        let (earliest_time, latest_time) = if logs.is_empty() {
            (Utc::now(), Utc::now())
        } else {
            let earliest = logs.iter().map(|l| l.timestamp).min().unwrap_or_else(Utc::now);
            let latest = logs.iter().map(|l| l.timestamp).max().unwrap_or_else(Utc::now);
            (earliest, latest)
        };
        
        let time_span = latest_time - earliest_time;
        let time_span_minutes = time_span.num_seconds() as f64 / 60.0;
        
        // Count log levels
        let mut error_count = 0;
        let mut warning_count = 0;

        // Process logs in reverse chronological order
        for log in logs.iter().rev().take(1000) {
            let log_time = log.timestamp;
            
            // Count by level
            match log.level {
                LogLevel::Error => error_count += 1,
                LogLevel::Warn => warning_count += 1,
                _ => {}
            }
            
            // Extract patterns
            for pattern in &log.patterns {
                match pattern {
                    super::patterns::DetectedPattern::Url(url) => {
                        if !detected_urls.contains(url) {
                            detected_urls.push(url.clone());
                        }
                    }
                    super::patterns::DetectedPattern::Port(port) => {
                        if !detected_ports.contains(port) {
                            detected_ports.push(*port);
                        }
                    }
                    super::patterns::DetectedPattern::KeyEvent(event) => {
                        if key_events.len() < 10 {
                            key_events.push(event.clone());
                        }
                    }
                    super::patterns::DetectedPattern::Error(error) => {
                        if recent_errors.len() < 10 {
                            recent_errors.push(error.clone());
                        }
                        
                        // Track error patterns
                        let pattern_key = self.normalize_error_pattern(error);
                        error_pattern_map.entry(pattern_key)
                            .and_modify(|(count, _first, last)| {
                                *count += 1;
                                *last = log_time;
                            })
                            .or_insert((1, log_time, log_time));
                    }
                    _ => {}
                }
            }

            // Also check log level for errors
            if log.level == LogLevel::Error && recent_errors.len() < 10 {
                recent_errors.push(log.clean_line.clone());
            }
        }
        
        // Convert error patterns to sorted list
        let mut error_patterns: Vec<ErrorPattern> = error_pattern_map.into_iter()
            .map(|(pattern, (count, first_seen, last_seen))| ErrorPattern {
                pattern,
                count,
                first_seen,
                last_seen,
            })
            .collect();
        error_patterns.sort_by_key(|p| std::cmp::Reverse(p.count));
        error_patterns.truncate(5); // Keep top 5 patterns
        
        // Calculate metrics
        let metrics = SummaryMetrics {
            total_logs: logs.len(),
            error_rate: if time_span_minutes > 0.0 { error_count as f64 / time_span_minutes } else { 0.0 },
            warning_rate: if time_span_minutes > 0.0 { warning_count as f64 / time_span_minutes } else { 0.0 },
            log_velocity: if time_span_minutes > 0.0 { logs.len() as f64 / time_span_minutes } else { 0.0 },
            time_span_minutes,
        };
        
        // Get process info if available
        let (status, uptime, cpu_percent, memory_mb) = if let Some(info) = process_info {
            let uptime_duration = Duration::seconds(info.uptime_seconds as i64);
            let uptime_str = format_duration(uptime_duration);
            (
                info.status,
                uptime_str,
                info.cpu_percent.unwrap_or(0.0),
                info.memory_mb.unwrap_or(0) as f64,
            )
        } else {
            (ProcessStatus::Running, "unknown".to_string(), 0.0, 0.0)
        };

        ProcessSummary {
            status,
            uptime,
            key_events,
            recent_errors,
            detected_urls,
            detected_ports,
            resource_usage: ResourceUsage {
                cpu_percent: format!("{:.1}%", cpu_percent),
                memory_mb: format!("{:.1}MB", memory_mb),
            },
            error_patterns,
            metrics,
        }
    }
    
    fn normalize_error_pattern(&self, error: &str) -> String {
        // Normalize error messages to group similar errors
        let mut normalized = error.to_lowercase();
        
        // Remove timestamps
        normalized = regex::Regex::new(r"\d{4}-\d{2}-\d{2}[T\s]\d{2}:\d{2}:\d{2}")
            .unwrap()
            .replace_all(&normalized, "[timestamp]")
            .to_string();
            
        // Remove numbers that might be IDs or ports
        normalized = regex::Regex::new(r"\b\d{3,}\b")
            .unwrap()
            .replace_all(&normalized, "[number]")
            .to_string();
            
        // Remove file paths
        normalized = regex::Regex::new(r"[/\\][\w\-./\\]+")
            .unwrap()
            .replace_all(&normalized, "[path]")
            .to_string();
            
        // Truncate to reasonable length
        if normalized.len() > 100 {
            normalized.truncate(100);
            normalized.push_str("...");
        }
        
        normalized
    }

    pub fn generate_agent_summary(&self, processes: Vec<ProcessSummary>) -> AgentSummary {
        let mut total_errors = 0;
        let mut all_urls = Vec::new();
        let mut all_ports = Vec::new();

        for process in &processes {
            total_errors += process.recent_errors.len();
            all_urls.extend(process.detected_urls.clone());
            all_ports.extend(process.detected_ports.clone());
        }

        // Deduplicate
        all_urls.sort();
        all_urls.dedup();
        all_ports.sort();
        all_ports.dedup();

        AgentSummary {
            process_count: processes.len(),
            healthy_count: processes.iter().filter(|p| p.recent_errors.is_empty()).count(),
            total_errors,
            all_detected_urls: all_urls,
            all_detected_ports: all_ports,
            recommendations: self.generate_recommendations(&processes),
        }
    }

    fn generate_recommendations(&self, processes: &[ProcessSummary]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Analyze error patterns across all processes
        let mut global_error_patterns: HashMap<String, usize> = HashMap::new();
        let mut _total_error_rate = 0.0;
        let mut high_error_processes = Vec::new();
        
        for process in processes {
            // Track error rates
            if process.metrics.error_rate > 1.0 { // More than 1 error per minute
                high_error_processes.push((&process.status, process.metrics.error_rate));
            }
            _total_error_rate += process.metrics.error_rate;
            
            // Aggregate error patterns
            for pattern in &process.error_patterns {
                *global_error_patterns.entry(pattern.pattern.clone()).or_insert(0) += pattern.count;
            }
        }
        
        // High error rate recommendation
        if !high_error_processes.is_empty() {
            let process_list = high_error_processes.iter()
                .map(|(status, rate)| format!("{:?} ({:.1} errors/min)", status, rate))
                .collect::<Vec<_>>()
                .join(", ");
            recommendations.push(format!(
                "High error rates detected in: {}. Investigate stability issues.",
                process_list
            ));
        }
        
        // Common error pattern recommendation
        let mut top_patterns: Vec<_> = global_error_patterns.into_iter().collect();
        top_patterns.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        
        if let Some((pattern, count)) = top_patterns.first() {
            if *count > 10 {
                recommendations.push(format!(
                    "Frequent error pattern detected ({} occurrences): '{}'. This may indicate a systematic issue.",
                    count, pattern
                ));
            }
        }

        // Check for high resource usage
        let mut high_cpu_processes = Vec::new();
        let mut high_memory_processes = Vec::new();
        
        for process in processes {
            if let Ok(cpu) = process.resource_usage.cpu_percent.trim_end_matches('%').parse::<f32>() {
                if cpu > 80.0 {
                    high_cpu_processes.push((&process.status, cpu));
                }
            }
            
            if let Ok(memory) = process.resource_usage.memory_mb.trim_end_matches("MB").parse::<f32>() {
                if memory > 1024.0 { // Over 1GB
                    high_memory_processes.push((&process.status, memory));
                }
            }
        }
        
        if !high_cpu_processes.is_empty() {
            recommendations.push(format!(
                "High CPU usage detected: {}. Consider optimization or horizontal scaling.",
                high_cpu_processes.iter()
                    .map(|(status, cpu)| format!("{:?} ({:.1}%)", status, cpu))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        
        if !high_memory_processes.is_empty() {
            recommendations.push(format!(
                "High memory usage detected: {}. Check for memory leaks.",
                high_memory_processes.iter()
                    .map(|(status, mem)| format!("{:?} ({:.0}MB)", status, mem))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        
        // Port conflict detection
        let mut port_usage: HashMap<u16, Vec<&ProcessStatus>> = HashMap::new();
        for process in processes {
            for port in &process.detected_ports {
                port_usage.entry(*port).or_insert_with(Vec::new).push(&process.status);
            }
        }
        
        let conflicts: Vec<_> = port_usage.iter()
            .filter(|(_, processes)| processes.len() > 1)
            .collect();
            
        if !conflicts.is_empty() {
            for (port, processes) in conflicts {
                recommendations.push(format!(
                    "Port {} is used by multiple processes: {:?}. This may cause conflicts.",
                    port, processes
                ));
            }
        }
        
        // Log velocity analysis
        let high_velocity_processes: Vec<_> = processes.iter()
            .filter(|p| p.metrics.log_velocity > 100.0) // More than 100 logs/minute
            .collect();
            
        if !high_velocity_processes.is_empty() {
            recommendations.push(format!(
                "{} processes are generating excessive logs (>100/min). Consider adjusting log levels.",
                high_velocity_processes.len()
            ));
        }

        // If no issues found
        if recommendations.is_empty() {
            recommendations.push("All systems operating normally. No immediate issues detected.".to_string());
        }

        recommendations
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();
    
    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}m {}s", minutes, seconds)
    } else if total_seconds < 86400 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        format!("{}d {}h", days, hours)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub process_count: usize,
    pub healthy_count: usize,
    pub total_errors: usize,
    pub all_detected_urls: Vec<String>,
    pub all_detected_ports: Vec<u16>,
    pub recommendations: Vec<String>,
}