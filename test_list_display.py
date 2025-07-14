#!/usr/bin/env python3
"""
Test script to verify the new list display with working directory and ports.
"""

import subprocess
import time
import sys

def run_command(cmd, capture_output=True):
    """Run a command and return the result"""
    print(f"Running: {cmd}")
    result = subprocess.run(cmd, shell=True, capture_output=capture_output, text=True)
    if capture_output:
        print(f"Output: {result.stdout}")
        if result.stderr:
            print(f"Error: {result.stderr}")
    return result

def main():
    print("=== Testing List Display with Working Directory and Ports ===")
    
    # Kill any existing daemon
    print("\n1. Stopping any existing daemon...")
    subprocess.run("pkill -f 'apm start'", shell=True)
    time.sleep(1)
    
    # Start daemon
    print("\n2. Starting daemon...")
    daemon_proc = subprocess.Popen(
        ["./target/debug/apm", "start"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    time.sleep(2)
    
    # Test processes in different directories
    print("\n3. Spawning test processes...")
    
    # From current directory
    run_command("./target/debug/apm spawn web-server python3 -m http.server 8080")
    time.sleep(0.5)
    
    # From a subdirectory (if exists)
    run_command("mkdir -p test_subdir && cd test_subdir && ../target/debug/apm spawn sub-process echo 'hello from subdir'")
    time.sleep(0.5)
    
    # A process that uses a port
    run_command("./target/debug/apm spawn node-server node -e 'const http = require(\"http\"); http.createServer((req, res) => res.end(\"Hello\")).listen(3000, () => console.log(\"Server running on port 3000\"));'")
    time.sleep(2)
    
    # List processes to see the new display
    print("\n4. Listing processes with new display format:")
    result = run_command("./target/debug/apm list --all")
    
    print("\n5. Waiting for port detection...")
    time.sleep(3)
    
    # List again to see detected ports
    print("\n6. Listing processes again (should show detected ports):")
    result = run_command("./target/debug/apm list --all")
    
    # Stop all processes
    print("\n7. Stopping all processes...")
    run_command("./target/debug/apm stop-all --force")
    
    # Stop daemon
    print("\n8. Stopping daemon...")
    daemon_proc.terminate()
    daemon_proc.wait()
    
    # Cleanup
    subprocess.run("rm -rf test_subdir", shell=True)
    
    print("\n=== Test Complete ===")
    return 0

if __name__ == "__main__":
    sys.exit(main())