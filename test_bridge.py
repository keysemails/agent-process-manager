#!/usr/bin/env python3
"""
Test the MCP bridge functionality
"""

import subprocess
import json
import time

def test_mcp_bridge():
    print("=== Testing APM MCP Bridge ===\n")
    
    # Start the bridge process
    bridge_process = subprocess.Popen(
        ['./target/debug/apm', 'mcp-bridge'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    try:
        # Send initialize message
        init_message = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-bridge-client",
                    "version": "1.0"
                }
            }
        }
        
        print("Sending initialize message...")
        bridge_process.stdin.write(json.dumps(init_message) + '\n')
        bridge_process.stdin.flush()
        
        # Read response
        response_line = bridge_process.stdout.readline()
        print(f"Received: {response_line.strip()}")
        
        if response_line:
            response = json.loads(response_line.strip())
            if 'result' in response:
                print("✅ Bridge working! Server info:", response['result'].get('serverInfo', {}))
                
                # Send initialized notification
                notification = {
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized"
                }
                bridge_process.stdin.write(json.dumps(notification) + '\n')
                bridge_process.stdin.flush()
                
                # Test list command
                list_command = {
                    "jsonrpc": "2.0",
                    "id": 2,
                    "method": "tools/call",
                    "params": {
                        "name": "list",
                        "arguments": {"current_dir": True}
                    }
                }
                
                print("\nTesting list command...")
                bridge_process.stdin.write(json.dumps(list_command) + '\n')
                bridge_process.stdin.flush()
                
                list_response = bridge_process.stdout.readline()
                print(f"List response: {list_response.strip()[:100]}...")
                
                return True
            else:
                print("❌ No result in response")
                return False
        else:
            print("❌ No response received")
            return False
            
    except Exception as e:
        print(f"❌ Error: {e}")
        return False
    finally:
        bridge_process.terminate()
        bridge_process.wait()

if __name__ == "__main__":
    success = test_mcp_bridge()
    if success:
        print("\n🎉 Bridge test successful! Ready for Claude Code integration.")
    else:
        print("\n💥 Bridge test failed.")