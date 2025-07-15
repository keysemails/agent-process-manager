#!/usr/bin/env python3
"""
Test auto-cleanup functionality by:
1. Starting daemon with test config
2. Spawning some test processes
3. Stopping them
4. Restarting daemon to trigger auto-cleanup
"""

import subprocess
import time
import requests
import sys

def run_command(cmd):
    """Run a command and return success status"""
    print(f"Running: {cmd}")
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
        return False
    return True

def main():
    print("=== Testing Auto-Cleanup Feature ===")
    
    # Kill any existing daemon
    print("\n1. Stopping any existing daemon...")
    subprocess.run("pkill -f 'apm start'", shell=True)
    time.sleep(1)
    
    # Start daemon with test config
    print("\n2. Starting daemon with test config...")
    daemon_proc = subprocess.Popen(
        ["./target/debug/apm", "start", "--config", "test_auto_cleanup.yaml"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    time.sleep(2)
    
    # Check daemon is running
    try:
        resp = requests.get("http://localhost:7337/health")
        if resp.status_code != 200:
            print("Error: Daemon not responding")
            return 1
    except:
        print("Error: Cannot connect to daemon")
        return 1
    
    print("\n3. Spawning test processes...")
    # Spawn some test processes
    for i in range(3):
        run_command(f"./target/debug/apm spawn test-cleanup-{i} sleep 1")
        time.sleep(0.5)
    
    # Wait for processes to complete
    print("\n4. Waiting for processes to stop...")
    time.sleep(2)
    
    # List processes - should show stopped processes
    print("\n5. Listing processes (should show stopped):")
    run_command("./target/debug/apm list --all")
    
    # Stop daemon
    print("\n6. Stopping daemon...")
    daemon_proc.terminate()
    daemon_proc.wait()
    time.sleep(1)
    
    # Restart daemon - should trigger auto-cleanup
    print("\n7. Restarting daemon (should trigger auto-cleanup)...")
    daemon_proc = subprocess.Popen(
        ["./target/debug/apm", "start", "--config", "test_auto_cleanup.yaml"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT
    )
    
    # Read daemon output to see cleanup messages
    print("\n8. Daemon output (looking for cleanup messages):")
    for _ in range(10):
        line = daemon_proc.stdout.readline()
        if line:
            print(line.decode().strip())
            if "auto-cleanup" in line.decode():
                break
        time.sleep(0.1)
    
    time.sleep(2)
    
    # List processes - should be empty after cleanup
    print("\n9. Listing processes (should be empty after cleanup):")
    run_command("./target/debug/apm list --all")
    
    # Cleanup
    print("\n10. Cleaning up...")
    daemon_proc.terminate()
    daemon_proc.wait()
    subprocess.run("rm -f test_auto_cleanup.db*", shell=True)
    
    print("\n=== Test Complete ===")
    return 0

if __name__ == "__main__":
    sys.exit(main())