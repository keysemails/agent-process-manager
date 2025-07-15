#[cfg(test)]
pub mod test_utils {
    use crate::logs::{LogStorage, LogEntry, LogLevel};
    use crate::process::{ProcessConfig, ProcessInfo, ProcessStatus, ProcessId};
    use crate::process::supervisor::{RestartPolicy, ResourceLimits};
    use anyhow::Result;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Creates a test LogStorage instance
    pub async fn create_test_storage() -> Result<(Arc<LogStorage>, TempDir)> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite:{}", db_path.display());
        
        let storage = LogStorage::new(&db_url).await?;
        Ok((Arc::new(storage), temp_dir))
    }

    /// Creates a sample ProcessInfo for testing
    pub fn create_test_process_info(name: &str) -> ProcessInfo {
        ProcessInfo {
            id: ProcessId::new(),
            name: name.to_string(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            status: ProcessStatus::Running,
            pid: Some(12345),
            started_at: Utc::now(),
            uptime_seconds: 0,
            restart_count: 0,
            tags: vec![],
            cpu_percent: Some(0.0),
            memory_mb: Some(0),
            access_group: None,
            cwd: None,
            detected_ports: vec![],
        }
    }

    /// Creates a sample ProcessConfig for testing
    pub fn create_test_process_config(name: &str) -> ProcessConfig {
        ProcessConfig {
            name: name.to_string(),
            command: "echo".to_string(),
            args: vec!["test".to_string()],
            cwd: None,
            env: HashMap::new(),
            tags: vec![],
            pty: false,
            use_tmux: false,
            restart_policy: RestartPolicy::default(),
            resources: ResourceLimits::default(),
            access_group: None,
        }
    }

    /// Creates sample log entries for testing
    pub fn create_test_log_entries(_process_id: &str, count: usize) -> Vec<LogEntry> {
        let pid = ProcessId::new();
        (0..count)
            .map(|i| LogEntry {
                id: i as i64,
                process_id: pid.clone(),
                timestamp: Utc::now(),
                raw_line: format!("Test log line {}", i),
                clean_line: format!("Test log line {}", i),
                level: LogLevel::Info,
                patterns: vec![],
            })
            .collect()
    }

    /// Helper to wait for async condition with timeout
    pub async fn wait_for_condition<F, Fut>(
        mut condition: F,
        timeout_ms: u64,
        check_interval_ms: u64,
    ) -> Result<()>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let interval = std::time::Duration::from_millis(check_interval_ms);

        while start.elapsed() < timeout {
            if condition().await {
                return Ok(());
            }
            tokio::time::sleep(interval).await;
        }

        anyhow::bail!("Condition not met within timeout")
    }

    /// Mock process that generates predictable output
    pub struct MockProcess {
        pub output_lines: Vec<String>,
        pub delay_ms: u64,
        pub exit_code: i32,
    }

    impl MockProcess {
        pub fn new(output_lines: Vec<String>) -> Self {
            Self {
                output_lines,
                delay_ms: 10,
                exit_code: 0,
            }
        }

        pub async fn run(&self) -> Result<i32> {
            for line in &self.output_lines {
                println!("{}", line);
                tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;
            }
            Ok(self.exit_code)
        }
    }

    /// Test fixture for setting up a complete test environment
    pub struct TestFixture {
        pub storage: Arc<LogStorage>,
        pub _temp_dir: TempDir,
    }

    impl TestFixture {
        pub async fn new() -> Result<Self> {
            let (storage, temp_dir) = create_test_storage().await?;
            Ok(Self {
                storage,
                _temp_dir: temp_dir,
            })
        }
    }
}