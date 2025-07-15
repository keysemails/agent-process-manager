#!/usr/bin/env python3

import socket
import time

def test_connection():
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect(('localhost', 7338))
        print("Connected successfully!")
        
        # Send a proper initialize message with required parameters
        message = b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-client","version":"1.0"}}}\n'
        print(f"Sending: {message}")
        sock.send(message)
        
        # Try to receive with longer timeout
        sock.settimeout(5)
        try:
            response = sock.recv(1024)
            print(f"Received: {response}")
        except socket.timeout:
            print("No response received within timeout")
        except Exception as e:
            print(f"Error receiving: {e}")
        
        # Don't close immediately, wait a bit
        time.sleep(1)
        sock.close()
        return True
        
    except Exception as e:
        print(f"Error: {e}")
        return False

if __name__ == "__main__":
    test_connection()