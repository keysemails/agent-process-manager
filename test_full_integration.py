#!/usr/bin/env python3
"""
Full integration test simulating Claude Code agent usage
"""

import subprocess
import json
import time
import os

class APMIntegrationTest:
    def __init__(self):
        self.bridge_process = None
        
    def start_bridge(self):
        """Start the MCP bridge process"""
        print("🚀 Starting APM MCP bridge...")
        self.bridge_process = subprocess.Popen(
            ['./target/debug/apm', 'mcp-bridge'],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        time.sleep(0.5)  # Let it initialize
        
    def send_message(self, message):
        """Send a message through the bridge"""
        if not self.bridge_process:
            raise RuntimeError("Bridge not started")
            
        self.bridge_process.stdin.write(json.dumps(message) + '\n')
        self.bridge_process.stdin.flush()
        
        # Read response
        response_line = self.bridge_process.stdout.readline()
        if response_line:
            return json.loads(response_line.strip())
        return None
        
    def initialize(self):
        """Initialize MCP connection"""
        print("📡 Initializing MCP connection...")
        init_msg = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "claude-code-agent",
                    "version": "1.0"
                }
            }
        }
        
        response = self.send_message(init_msg)
        if response and 'result' in response:
            print(f"✅ Connected to {response['result']['serverInfo']['name']} v{response['result']['serverInfo']['version']}")
            
            # Send initialized notification
            notification = {
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }
            self.bridge_process.stdin.write(json.dumps(notification) + '\n')
            self.bridge_process.stdin.flush()
            return True
        return False
        
    def test_process_management(self):
        """Test typical Claude Code agent workflow"""
        print("\n🔧 Testing process management workflow...")
        
        # 1. List current processes
        print("1. Checking current processes...")
        list_msg = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "list",
                "arguments": {"current_dir": True}
            }
        }
        
        response = self.send_message(list_msg)
        if response and 'result' in response:
            processes = json.loads(response['result']['content'][0]['text'])
            print(f"   Found {len(processes)} processes in current directory")
            
        # 2. Spawn a new process
        print("2. Spawning a development server...")
        spawn_msg = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "spawn",
                "arguments": {
                    "name": "dev-server",
                    "command": "python3",
                    "args": ["-m", "http.server", "8081"]
                }
            }
        }
        
        response = self.send_message(spawn_msg)
        if response and 'result' in response:
            result = json.loads(response['result']['content'][0]['text'])
            if result.get('success'):
                process_id = result.get('process_id')
                print(f"   ✅ Server started with ID: {process_id}")
                
                # 3. Get system overview
                print("3. Getting system overview...")
                query_msg = {
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "query",
                        "arguments": {
                            "type": "system_overview",
                            "current_dir": True
                        }
                    }
                }
                
                response = self.send_message(query_msg)
                if response and 'result' in response:
                    overview = json.loads(response['result']['content'][0]['text'])
                    print(f"   System: {overview['total_processes']} total, {overview['running']} running")
                    
                # 4. Stop the process
                print("4. Stopping the development server...")
                stop_msg = {
                    "jsonrpc": "2.0",
                    "id": 5,
                    "method": "tools/call",
                    "params": {
                        "name": "stop",
                        "arguments": {
                            "process_id": process_id
                        }
                    }
                }
                
                response = self.send_message(stop_msg)
                if response and 'result' in response:
                    result = json.loads(response['result']['content'][0]['text'])
                    if result.get('success'):
                        print(f"   ✅ Server stopped successfully")
                        return True
                        
        return False
        
    def cleanup(self):
        """Clean up the bridge process"""
        if self.bridge_process:
            self.bridge_process.terminate()
            self.bridge_process.wait()
            
    def run_full_test(self):
        """Run the complete integration test"""
        print("🧪 APM <-> Claude Code Integration Test")
        print("=" * 50)
        
        try:
            self.start_bridge()
            
            if not self.initialize():
                print("❌ Failed to initialize MCP connection")
                return False
                
            if not self.test_process_management():
                print("❌ Process management test failed")
                return False
                
            print("\n🎉 All tests passed! APM is ready for Claude Code integration.")
            print("\nNext steps:")
            print("1. Copy examples/claude-code-config.json to ~/.config/claude/claude_code_config.json")
            print("2. Restart Claude Code")
            print("3. Claude Code agents will now have access to APM tools!")
            return True
            
        except Exception as e:
            print(f"❌ Test failed with error: {e}")
            return False
        finally:
            self.cleanup()

if __name__ == "__main__":
    # Check if we're in the right directory
    if not os.path.exists('./target/debug/apm'):
        print("❌ Please run this from the APM project root directory")
        print("   Make sure you've built the project with: cargo build")
        exit(1)
        
    test = APMIntegrationTest()
    success = test.run_full_test()
    exit(0 if success else 1)