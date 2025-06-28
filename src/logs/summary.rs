//! Log summarization for AI-friendly output

use super::{LogEntry, LogLevel};
use crate::process::ProcessStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSummary {
    pub status: ProcessStatus,
    pub uptime: String,
    pub key_events: Vec<String>,
    pub recent_errors: Vec<String>,
    pub detected_urls: Vec<String>,
    pub detected_ports: Vec<u16>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: String,
    pub memory_mb: String,
}

pub struct LogSummarizer;

impl LogSummarizer {
    pub fn new() -> Self {
        Self
    }

    pub fn summarize_logs(&self, logs: &[LogEntry]) -> ProcessSummary {
        let mut key_events = Vec::new();
        let mut recent_errors = Vec::new();
        let mut detected_urls = Vec::new();
        let mut detected_ports = Vec::new();

        // Process logs in reverse chronological order
        for log in logs.iter().rev().take(1000) {
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
                        if recent_errors.len() < 5 {
                            recent_errors.push(error.clone());
                        }
                    }
                    _ => {}
                }
            }

            // Also check log level
            if log.level == LogLevel::Error && recent_errors.len() < 5 {
                recent_errors.push(log.clean_line.clone());
            }
        }

        ProcessSummary {
            status: ProcessStatus::Running, // This should come from process info
            uptime: "5m 23s".to_string(), // This should be calculated
            key_events,
            recent_errors,
            detected_urls,
            detected_ports,
            resource_usage: ResourceUsage {
                cpu_percent: "12%".to_string(),
                memory_mb: "234MB".to_string(),
            },
        }
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

        // Check for errors
        let error_count: usize = processes.iter().map(|p| p.recent_errors.len()).sum();
        if error_count > 0 {
            recommendations.push(format!(
                "Found {} errors across processes. Consider investigating the error logs.",
                error_count
            ));
        }

        // Check for high resource usage
        for (i, process) in processes.iter().enumerate() {
            if let Ok(cpu) = process.resource_usage.cpu_percent.trim_end_matches('%').parse::<f32>() {
                if cpu > 80.0 {
                    recommendations.push(format!(
                        "Process {} is using high CPU ({}%). Consider optimizing or scaling.",
                        i, cpu
                    ));
                }
            }
        }

        recommendations
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