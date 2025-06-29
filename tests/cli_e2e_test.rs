use std::process::{Command, Stdio};
use std::time::Duration;
use serial_test::serial;
use tempfile::TempDir;
use tokio::time::sleep;

fn apm_cmd() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(&["run", "--bin", "apm", "--"]);
    cmd
}

fn kill_daemon() {
    // Ensure no daemon is running before tests
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("apm start")
        .output();
    std::thread::sleep(Duration::from_millis(500));
}

#[tokio::test]
#[serial]
async fn test_daemon_lifecycle() {
    kill_daemon();
    
    // Start daemon
    let mut start_cmd = apm_cmd();
    start_cmd.arg("start");
    start_cmd.stdout(Stdio::null());
    start_cmd.stderr(Stdio::null());
    
    let mut daemon = start_cmd.spawn().expect("Failed to start daemon");
    
    // Wait for daemon to start
    sleep(Duration::from_secs(2)).await;
    
    // Check status
    let status_output = apm_cmd()
        .arg("status")
        .output()
        .expect("Failed to run status command");
    
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    assert!(status_str.contains("running") || status_str.contains("APM daemon is running"));
    
    // Kill daemon
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_spawn_and_list_processes() {
    kill_daemon();
    
    // Start daemon in background
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Spawn a process
    let spawn_output = apm_cmd()
        .args(&["spawn", "test-echo", "echo", "hello world"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(spawn_output.status.success());
    let spawn_str = String::from_utf8_lossy(&spawn_output.stdout);
    assert!(spawn_str.contains("Spawned process") || spawn_str.contains("test-echo"));
    
    // List processes
    let list_output = apm_cmd()
        .arg("list")
        .output()
        .expect("Failed to list processes");
    
    assert!(list_output.status.success());
    let list_str = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_str.contains("test-echo"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_view_logs() {
    kill_daemon();
    
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Spawn a process that generates output
    let spawn_output = apm_cmd()
        .args(&["spawn", "test-logs", "sh", "--", "-c", "echo 'Log line 1'; echo 'Log line 2'; echo 'Error: test error'"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(spawn_output.status.success());
    
    // Wait for logs to be processed
    sleep(Duration::from_millis(500)).await;
    
    // View logs
    let logs_output = apm_cmd()
        .args(&["logs", "test-logs"])
        .output()
        .expect("Failed to get logs");
    
    assert!(logs_output.status.success());
    let logs_str = String::from_utf8_lossy(&logs_output.stdout);
    assert!(logs_str.contains("Log line 1"));
    assert!(logs_str.contains("Log line 2"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_stop_process() {
    kill_daemon();
    
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Spawn a long-running process
    let spawn_output = apm_cmd()
        .args(&["spawn", "test-sleep", "sleep", "60"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(spawn_output.status.success());
    
    // Verify it's running
    let list_output = apm_cmd()
        .arg("list")
        .output()
        .expect("Failed to list processes");
    
    let list_str = String::from_utf8_lossy(&list_output.stdout);
    assert!(list_str.contains("test-sleep"));
    assert!(list_str.contains("running") || list_str.contains("Running"));
    
    // Stop the process
    let stop_output = apm_cmd()
        .args(&["stop", "test-sleep"])
        .output()
        .expect("Failed to stop process");
    
    assert!(stop_output.status.success());
    let stop_str = String::from_utf8_lossy(&stop_output.stdout);
    assert!(stop_str.contains("Stopped") || stop_str.contains("stopped"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_restart_process() {
    kill_daemon();
    
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Spawn a process
    let spawn_output = apm_cmd()
        .args(&["spawn", "test-restart", "echo", "initial"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(spawn_output.status.success());
    
    // Wait for it to complete
    sleep(Duration::from_millis(500)).await;
    
    // Restart the process
    let restart_output = apm_cmd()
        .args(&["restart", "test-restart"])
        .output()
        .expect("Failed to restart process");
    
    assert!(restart_output.status.success());
    let restart_str = String::from_utf8_lossy(&restart_output.stdout);
    assert!(restart_str.contains("Restarted") || restart_str.contains("restarted"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_process_with_args() {
    kill_daemon();
    
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Spawn process with multiple arguments
    let spawn_output = apm_cmd()
        .args(&["spawn", "test-args", "echo", "--", "arg1", "arg2", "arg3 with spaces"])
        .output()
        .expect("Failed to spawn process");
    
    assert!(spawn_output.status.success());
    
    // Check logs to verify arguments were passed correctly
    sleep(Duration::from_millis(500)).await;
    
    let logs_output = apm_cmd()
        .args(&["logs", "test-args"])
        .output()
        .expect("Failed to get logs");
    
    let logs_str = String::from_utf8_lossy(&logs_output.stdout);
    assert!(logs_str.contains("arg1"));
    assert!(logs_str.contains("arg2"));
    assert!(logs_str.contains("arg3 with spaces"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_invalid_commands() {
    kill_daemon();
    
    let mut daemon = apm_cmd()
        .arg("start")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start daemon");
    
    sleep(Duration::from_secs(2)).await;
    
    // Try to stop non-existent process
    let stop_output = apm_cmd()
        .args(&["stop", "non-existent"])
        .output()
        .expect("Failed to run stop command");
    
    assert!(!stop_output.status.success());
    let error_str = String::from_utf8_lossy(&stop_output.stderr);
    assert!(error_str.contains("not found") || error_str.contains("Not found"));
    
    // Try to get logs for non-existent process
    let logs_output = apm_cmd()
        .args(&["logs", "non-existent"])
        .output()
        .expect("Failed to run logs command");
    
    assert!(!logs_output.status.success() || 
            String::from_utf8_lossy(&logs_output.stdout).contains("No logs"));
    
    daemon.kill().expect("Failed to kill daemon");
}

#[tokio::test]
#[serial]
async fn test_daemon_not_running() {
    kill_daemon();
    
    // Try commands without daemon running
    let status_output = apm_cmd()
        .arg("status")
        .output()
        .expect("Failed to run status command");
    
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    assert!(status_str.contains("not running") || status_str.contains("No daemon"));
    
    // Try to spawn without daemon
    let spawn_output = apm_cmd()
        .args(&["spawn", "test", "echo", "test"])
        .output()
        .expect("Failed to run spawn command");
    
    assert!(!spawn_output.status.success());
    let error_str = String::from_utf8_lossy(&spawn_output.stderr);
    assert!(error_str.contains("daemon") || error_str.contains("not running"));
}

#[tokio::test]
#[serial]
async fn test_help_command() {
    let help_output = apm_cmd()
        .arg("help")
        .output()
        .expect("Failed to run help command");
    
    assert!(help_output.status.success());
    let help_str = String::from_utf8_lossy(&help_output.stdout);
    
    // Check that help includes main commands
    assert!(help_str.contains("start"));
    assert!(help_str.contains("spawn"));
    assert!(help_str.contains("list"));
    assert!(help_str.contains("logs"));
    assert!(help_str.contains("stop"));
}

#[tokio::test]
#[serial]
async fn test_version_command() {
    let version_output = apm_cmd()
        .arg("--version")
        .output()
        .expect("Failed to run version command");
    
    assert!(version_output.status.success());
    let version_str = String::from_utf8_lossy(&version_output.stdout);
    assert!(version_str.contains("apm") || version_str.contains("0.1.0"));
}