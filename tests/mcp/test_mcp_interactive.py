#!/usr/bin/env python3
"""Interactive MCP server test script"""

import json
import subprocess
import sys
import time

def send_request(proc, request):
    """Send a JSON-RPC request and get response"""
    request_str = json.dumps(request) + '\n'
    proc.stdin.write(request_str.encode())
    proc.stdin.flush()
    
    # Read response
    response_line = proc.stdout.readline().decode()
    if response_line:
        return json.loads(response_line)
    return None

def main():
    # Start the MCP server
    print("Starting MCP server...")
    proc = subprocess.Popen(
        ['cargo', 'run', '--bin', 'apm', '--', 'mcp'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    try:
        # Test 1: Initialize
        print("\n1. Testing initialize...")
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        }
        response = send_request(proc, init_request)
        print(f"Response: {json.dumps(response, indent=2)}")
        
        # Send initialized notification
        print("\n2. Sending initialized notification...")
        initialized_notif = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }
        proc.stdin.write((json.dumps(initialized_notif) + '\n').encode())
        proc.stdin.flush()
        time.sleep(0.5)
        
        # Test 2: List tools
        print("\n3. Testing tools/list...")
        list_tools_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }
        response = send_request(proc, list_tools_request)
        print(f"Response: {json.dumps(response, indent=2)}")
        
        # Test 3: List processes
        print("\n4. Testing call tool 'list'...")
        list_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list"
            }
        }
        response = send_request(proc, list_request)
        print(f"Response: {json.dumps(response, indent=2)}")
        
        # Test 4: Spawn a process
        print("\n5. Testing call tool 'spawn'...")
        spawn_request = {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "spawn",
                "arguments": {
                    "name": "test-echo",
                    "command": "echo",
                    "args": ["Hello from MCP test"]
                }
            }
        }
        response = send_request(proc, spawn_request)
        print(f"Response: {json.dumps(response, indent=2)}")
        
        # Check stderr for any errors
        time.sleep(0.5)
        stderr_output = proc.stderr.read(1024).decode() if proc.stderr else ""
        if stderr_output:
            print(f"\nStderr output:\n{stderr_output}")
        
    except Exception as e:
        print(f"Error: {e}")
    finally:
        proc.terminate()
        proc.wait()

if __name__ == "__main__":
    main()