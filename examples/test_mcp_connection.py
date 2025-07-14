#!/usr/bin/env python3
"""
Simple script to test MCP connection to APM.

This demonstrates basic MCP protocol communication with APM's TCP server.

Usage:
1. Start APM with MCP enabled:
   APM_MCP_ENABLED=1 cargo run -- start

2. Run this script:
   python examples/test_mcp_connection.py
"""

import json
import socket
import sys

def send_json_rpc(sock, method, params, msg_id=1):
    """Send a JSON-RPC message and receive response."""
    message = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": msg_id
    }
    
    # Send message
    data = json.dumps(message) + "\n"
    sock.sendall(data.encode())
    
    # Receive response
    response = ""
    while True:
        chunk = sock.recv(1024).decode()
        response += chunk
        if "\n" in response:
            break
    
    # Parse first complete JSON response
    lines = response.split("\n")
    if lines[0]:
        return json.loads(lines[0])
    return None

def test_mcp_connection():
    """Test basic MCP operations."""
    # Connect to APM's MCP server
    HOST = '127.0.0.1'
    PORT = 7338  # Default MCP port
    
    print(f"Connecting to MCP server at {HOST}:{PORT}...")
    
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.connect((HOST, PORT))
            print("Connected!")
            
            # 1. Initialize
            print("\n1. Initializing MCP connection...")
            response = send_json_rpc(sock, "initialize", {
                "protocolVersion": "0.1.0",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }, msg_id=1)
            print(f"Response: {json.dumps(response, indent=2)}")
            
            # Send initialized notification (required by MCP protocol)
            initialized_msg = {
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            }
            sock.sendall((json.dumps(initialized_msg) + "\n").encode())
            print("\nSent initialized notification")
            
            # 2. List tools
            print("\n2. Listing available tools...")
            response = send_json_rpc(sock, "tools/list", {}, msg_id=2)
            print(f"Response: {json.dumps(response, indent=2)}")
            
            # 3. Spawn a test process
            print("\n3. Spawning a test process...")
            response = send_json_rpc(sock, "tools/call", {
                "name": "spawn",
                "arguments": {
                    "name": "test-echo",
                    "command": "echo",
                    "args": ["Hello from Python MCP client!"]
                }
            }, msg_id=3)
            print(f"Response: {json.dumps(response, indent=2)}")
            
            # 4. List processes
            print("\n4. Listing processes...")
            response = send_json_rpc(sock, "tools/call", {
                "name": "list",
                "arguments": {}
            }, msg_id=4)
            print(f"Response: {json.dumps(response, indent=2)}")
            
            # 5. Query system overview
            print("\n5. Querying system overview...")
            response = send_json_rpc(sock, "tools/call", {
                "name": "query",
                "arguments": {
                    "type": "system_overview"
                }
            }, msg_id=5)
            print(f"Response: {json.dumps(response, indent=2)}")
            
            print("\nMCP connection test completed successfully!")
            
    except ConnectionRefusedError:
        print(f"ERROR: Could not connect to MCP server at {HOST}:{PORT}")
        print("Make sure APM is running with MCP enabled:")
        print("  APM_MCP_ENABLED=1 cargo run -- start")
        sys.exit(1)
    except Exception as e:
        print(f"ERROR: {e}")
        sys.exit(1)

if __name__ == "__main__":
    test_mcp_connection()