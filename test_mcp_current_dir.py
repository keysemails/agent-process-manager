#!/usr/bin/env python3
"""
Test MCP current_dir parameter functionality
"""

import socket
import json
import sys

def send_mcp_request(sock, method, params=None):
    """Send an MCP request and get response"""
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params or {}
    }
    
    message = json.dumps(request)
    print(f"Sending: {message}")
    sock.sendall(message.encode() + b'\n')
    
    response = sock.recv(4096).decode()
    print(f"Received: '{response}'")
    if not response.strip():
        print("Empty response received!")
        return None
    return json.loads(response.strip())

def main():
    print("=== Testing MCP current_dir Parameter ===\n")
    
    # Connect to MCP server
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.connect(('localhost', 7338))
        print("Connected to MCP server on port 7338")
    except Exception as e:
        print(f"Failed to connect: {e}")
        print("Make sure the daemon is running with MCP enabled")
        return 1
    
    # Initialize connection
    print("\n1. Initializing MCP connection...")
    response = send_mcp_request(sock, "initialize", {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    })
    if response is None:
        print("Failed to get initialize response")
        return 1
    print(f"Initialize response: {response.get('result', {}).get('serverInfo', {})}")
    
    # Send initialized notification (required by MCP protocol)
    print("\n1.5. Sending initialized notification...")
    notification = {
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }
    message = json.dumps(notification)
    print(f"Sending: {message}")
    sock.sendall(message.encode() + b'\n')
    
    # Test list without current_dir (should show all)
    print("\n2. Testing list without current_dir (show all)...")
    response = send_mcp_request(sock, "tools/call", {
        "name": "list",
        "arguments": {}
    })
    
    result = response.get('result', {})
    if 'content' in result and len(result['content']) > 0:
        processes = json.loads(result['content'][0]['text'])
        print(f"Found {len(processes)} total processes")
        for p in processes[:3]:  # Show first 3
            print(f"  - {p['name']} (dir: {p.get('access_group', 'none')})")
    
    # Test list with current_dir=true
    print("\n3. Testing list with current_dir=true...")
    response = send_mcp_request(sock, "tools/call", {
        "name": "list",
        "arguments": {"current_dir": True}
    })
    
    result = response.get('result', {})
    if 'content' in result and len(result['content']) > 0:
        processes = json.loads(result['content'][0]['text'])
        print(f"Found {len(processes)} processes in current directory")
        for p in processes:
            print(f"  - {p['name']} (dir: {p.get('access_group', 'none')})")
    
    # Test query with current_dir
    print("\n4. Testing query with current_dir=true...")
    response = send_mcp_request(sock, "tools/call", {
        "name": "query",
        "arguments": {
            "type": "system_overview",
            "current_dir": True
        }
    })
    
    result = response.get('result', {})
    if 'content' in result and len(result['content']) > 0:
        overview = json.loads(result['content'][0]['text'])
        print(f"System overview (current dir): {overview}")
    
    sock.close()
    print("\n=== Test Complete ===")
    return 0

if __name__ == "__main__":
    sys.exit(main())