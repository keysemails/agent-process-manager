"""
Pytest configuration and fixtures for APM integration tests.
"""
import os
import sys
import pytest
import subprocess
import time
import tempfile
import shutil
from pathlib import Path

# Add src to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "src"))

@pytest.fixture(scope="session")
def apm_binary():
    """Path to the APM binary."""
    project_root = Path(__file__).parent.parent.parent.parent
    binary_path = project_root / "target" / "release" / "apm"
    
    if not binary_path.exists():
        # Try debug build
        binary_path = project_root / "target" / "debug" / "apm"
    
    if not binary_path.exists():
        pytest.skip("APM binary not found. Run 'cargo build' first.")
    
    return str(binary_path)

@pytest.fixture(scope="session")
def apm_daemon(apm_binary):
    """Start APM daemon for tests."""
    # Stop any existing daemon
    subprocess.run([apm_binary, "stop-all", "--force"], 
                  capture_output=True, check=False)
    
    # Start daemon
    proc = subprocess.Popen([apm_binary, "start"], 
                           stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE)
    
    # Wait for daemon to start
    time.sleep(2)
    
    # Verify daemon is running
    result = subprocess.run([apm_binary, "status"], 
                           capture_output=True, text=True)
    if result.returncode != 0:
        pytest.skip("Failed to start APM daemon")
    
    yield apm_binary
    
    # Cleanup: stop daemon
    subprocess.run([apm_binary, "stop-all", "--force"], 
                  capture_output=True, check=False)

@pytest.fixture
def temp_workspace():
    """Create a temporary workspace for tests."""
    with tempfile.TemporaryDirectory() as tmpdir:
        old_cwd = os.getcwd()
        os.chdir(tmpdir)
        try:
            yield Path(tmpdir)
        finally:
            os.chdir(old_cwd)

@pytest.fixture
def test_config():
    """Test configuration file."""
    config_content = """
api:
  host: "127.0.0.1"
  port: 7337

mcp:
  enabled: true
  transport: "tcp"
  tcp_host: "127.0.0.1"
  tcp_port: 7338

cleanup:
  auto_clean_on_startup: false
  retention_hours: 24
  keep_logs: true
  keep_failed: true

access_control:
  mode: "open"
"""
    
    with tempfile.NamedTemporaryFile(mode='w', suffix='.yaml', delete=False) as f:
        f.write(config_content)
        config_path = f.name
    
    yield config_path
    
    # Cleanup
    os.unlink(config_path)

def wait_for_process_status(apm_binary, process_name, expected_status, timeout=10):
    """Wait for a process to reach expected status."""
    start_time = time.time()
    while time.time() - start_time < timeout:
        result = subprocess.run([apm_binary, "list"], 
                               capture_output=True, text=True)
        if result.returncode == 0:
            for line in result.stdout.split('\n'):
                if process_name in line and expected_status in line:
                    return True
        time.sleep(0.5)
    return False

def cleanup_process(apm_binary, process_name):
    """Clean up a test process."""
    subprocess.run([apm_binary, "stop", process_name], 
                  capture_output=True, check=False)
    time.sleep(1)