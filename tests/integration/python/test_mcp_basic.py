#!/usr/bin/env python3

import socket
import time

def test_connection(apm_daemon):
    """Test basic MCP connection to APM daemon."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    
    try:
        sock.connect(('localhost', 7338))
        print("Connected successfully!")
        
        # Send a proper initialize message with required parameters
        message = b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0"}}}\n'
        print(f"Sending: {message}")
        sock.send(message)
        
        # Try to receive with longer timeout
        sock.settimeout(5)
        response = sock.recv(1024)
        print(f"Received: {response}")
        assert len(response) > 0, "Should receive a response"
        
        # Don't close immediately, wait a bit
        time.sleep(1)
        
    finally:
        sock.close()

if __name__ == "__main__":
    test_connection()