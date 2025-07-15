#!/usr/bin/env python3
"""Full MCP server test with proper message handling"""

import json
import subprocess
import sys
import time
import os

def send_and_receive(proc, request):
    """Send a JSON-RPC request and wait for response"""
    request_str = json.dumps(request) + '\n'
    print(f"\n→ Sending: {request_str.strip()}")
    proc.stdin.write(request_str.encode())
    proc.stdin.flush()
    
    # For notifications, we don't expect a response
    if 'id' not in request:
        time.sleep(0.1)
        return None
    
    # Read response
    response_line = proc.stdout.readline()
    if response_line:
        try:
            response_str = response_line.decode().strip()
            if response_str:
                response = json.loads(response_str)
                print(f"← Response: {json.dumps(response, indent=2)}")
                return response
        except json.JSONDecodeError as e:
            print(f"← Error parsing response: {e}")
            print(f"  Raw: {response_line}")
    return None

def main():
    # Set environment to enable MCP
    env = os.environ.copy()
    env['APM_MCP_ENABLED'] = '1'
    env['RUST_LOG'] = 'agent_process_manager=debug'
    
    # Start the MCP server
    print("Starting MCP server...")
    proc = subprocess.Popen(
        ['cargo', 'run', '--bin', 'apm', '--', 'start', '--mcp'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
        text=False  # Use binary mode
    )
    
    try:
        # Wait a bit for server to start
        time.sleep(1)
        
        # Test 1: Initialize
        print("\n=== Test 1: Initialize ===")
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
        init_response = send_and_receive(proc, init_request)
        
        if init_response and 'result' in init_response:
            print("✓ Initialize successful")
        
        # Send initialized notification
        print("\n=== Test 2: Initialized Notification ===")
        initialized_notif = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }
        send_and_receive(proc, initialized_notif)
        print("✓ Initialized notification sent")
        
        # Test 3: List tools
        print("\n=== Test 3: List Tools ===")
        list_tools_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }
        tools_response = send_and_receive(proc, list_tools_request)
        
        if tools_response and 'result' in tools_response:
            tools = tools_response['result'].get('tools', [])
            print(f"✓ Found {len(tools)} tools:")
            for tool in tools:
                print(f"  - {tool['name']}: {tool.get('description', 'No description')}")
        
        # Test 4: Call list tool
        print("\n=== Test 4: Call Tool 'list' ===")
        list_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list"
            }
        }
        list_response = send_and_receive(proc, list_request)
        
        if list_response and 'result' in list_response:
            print("✓ List tool called successfully")
        
        # Test 5: Spawn a test process
        print("\n=== Test 5: Spawn Test Process ===")
        spawn_request = {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "spawn",
                "arguments": {
                    "name": "test-sleep",
                    "command": "sleep",
                    "args": ["2"]
                }
            }
        }
        spawn_response = send_and_receive(proc, spawn_request)
        
        if spawn_response and 'result' in spawn_response:
            print("✓ Process spawned successfully")
            
            # Extract process ID if available
            result = spawn_response['result']
            if 'content' in result and len(result['content']) > 0:
                content = result['content'][0].get('text', '{}')
                try:
                    spawn_data = json.loads(content)
                    process_id = spawn_data.get('process_id')
                    print(f"  Process ID: {process_id}")
                    
                    # Test 6: Get logs for the spawned process
                    if process_id:
                        print("\n=== Test 6: Get Process Logs ===")
                        logs_request = {
                            "jsonrpc": "2.0",
                            "id": 5,
                            "method": "tools/call",
                            "params": {
                                "name": "logs",
                                "arguments": {
                                    "process_id": process_id,
                                    "limit": 10
                                }
                            }
                        }
                        logs_response = send_and_receive(proc, logs_request)
                        
                        if logs_response and 'result' in logs_response:
                            print("✓ Logs retrieved successfully")
                    
                        # Test 7: Stop the process
                        print("\n=== Test 7: Stop Process ===")
                        stop_request = {
                            "jsonrpc": "2.0",
                            "id": 6,
                            "method": "tools/call",
                            "params": {
                                "name": "stop",
                                "arguments": {
                                    "process_id": process_id
                                }
                            }
                        }
                        stop_response = send_and_receive(proc, stop_request)
                        
                        if stop_response and 'result' in stop_response:
                            print("✓ Process stopped successfully")
                except:
                    pass
        
        # Test 8: Query system overview
        print("\n=== Test 8: Query System Overview ===")
        query_request = {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "query",
                "arguments": {
                    "type": "system_overview"
                }
            }
        }
        query_response = send_and_receive(proc, query_request)
        
        if query_response and 'result' in query_response:
            print("✓ Query executed successfully")
        
        print("\n=== All tests completed ===")
        
        # Give some time for any final messages
        time.sleep(0.5)
        
        # Check for any stderr output
        proc.terminate()
        proc.wait(timeout=2)
        
        stderr_output = proc.stderr.read().decode()
        if stderr_output:
            print("\n=== Debug output ===")
            for line in stderr_output.split('\n')[:20]:  # First 20 lines
                if line.strip():
                    print(f"[DEBUG] {line}")
        
    except Exception as e:
        print(f"\nError during testing: {e}")
        proc.terminate()
        proc.wait()
        raise
    finally:
        if proc.poll() is None:
            proc.terminate()
            proc.wait()

if __name__ == "__main__":
    main()