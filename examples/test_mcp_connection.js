#!/usr/bin/env node
/**
 * Simple script to test MCP connection to APM using Node.js
 * 
 * Usage:
 * 1. Start APM with MCP enabled:
 *    APM_MCP_ENABLED=1 cargo run -- start
 * 
 * 2. Run this script:
 *    node examples/test_mcp_connection.js
 */

const net = require('net');

class MCPClient {
    constructor(host = '127.0.0.1', port = 7338) {
        this.host = host;
        this.port = port;
        this.socket = null;
        this.buffer = '';
        this.messageId = 0;
        this.pendingCallbacks = new Map();
    }

    connect() {
        return new Promise((resolve, reject) => {
            this.socket = net.createConnection({ host: this.host, port: this.port }, () => {
                console.log(`Connected to MCP server at ${this.host}:${this.port}`);
                resolve();
            });

            this.socket.on('data', (data) => {
                this.buffer += data.toString();
                this.processBuffer();
            });

            this.socket.on('error', reject);
            this.socket.on('close', () => {
                console.log('Connection closed');
            });
        });
    }

    processBuffer() {
        const lines = this.buffer.split('\n');
        this.buffer = lines.pop() || '';

        for (const line of lines) {
            if (line.trim()) {
                try {
                    const response = JSON.parse(line);
                    const callback = this.pendingCallbacks.get(response.id);
                    if (callback) {
                        this.pendingCallbacks.delete(response.id);
                        callback(response);
                    }
                } catch (e) {
                    console.error('Failed to parse response:', e);
                }
            }
        }
    }

    sendRequest(method, params = {}) {
        return new Promise((resolve) => {
            const id = ++this.messageId;
            const request = {
                jsonrpc: '2.0',
                method,
                params,
                id
            };

            this.pendingCallbacks.set(id, resolve);
            this.socket.write(JSON.stringify(request) + '\n');
        });
    }

    close() {
        if (this.socket) {
            this.socket.end();
        }
    }
}

async function testMCPConnection() {
    const client = new MCPClient();

    try {
        // Connect to server
        await client.connect();

        // 1. Initialize
        console.log('\n1. Initializing MCP connection...');
        const initResponse = await client.sendRequest('initialize', {
            protocolVersion: '0.1.0',
            capabilities: {},
            clientInfo: {
                name: 'node-test-client',
                version: '1.0.0'
            }
        });
        console.log('Response:', JSON.stringify(initResponse, null, 2));

        // Send initialized notification (required by MCP protocol)
        const initializedMsg = {
            jsonrpc: '2.0',
            method: 'notifications/initialized',
            params: {}
        };
        client.socket.write(JSON.stringify(initializedMsg) + '\n');
        console.log('\nSent initialized notification');

        // 2. List tools
        console.log('\n2. Listing available tools...');
        const toolsResponse = await client.sendRequest('tools/list', {});
        console.log('Response:', JSON.stringify(toolsResponse, null, 2));

        // 3. Spawn a test process
        console.log('\n3. Spawning a test process...');
        const spawnResponse = await client.sendRequest('tools/call', {
            name: 'spawn',
            arguments: {
                name: 'test-node-echo',
                command: 'echo',
                args: ['Hello from Node.js MCP client!']
            }
        });
        console.log('Response:', JSON.stringify(spawnResponse, null, 2));

        // 4. List processes
        console.log('\n4. Listing processes...');
        const listResponse = await client.sendRequest('tools/call', {
            name: 'list',
            arguments: {}
        });
        console.log('Response:', JSON.stringify(listResponse, null, 2));

        // 5. Query system overview
        console.log('\n5. Querying system overview...');
        const queryResponse = await client.sendRequest('tools/call', {
            name: 'query',
            arguments: {
                type: 'system_overview'
            }
        });
        console.log('Response:', JSON.stringify(queryResponse, null, 2));

        console.log('\nMCP connection test completed successfully!');

    } catch (error) {
        console.error('ERROR:', error.message);
        if (error.code === 'ECONNREFUSED') {
            console.error('Make sure APM is running with MCP enabled:');
            console.error('  APM_MCP_ENABLED=1 cargo run -- start');
        }
        process.exit(1);
    } finally {
        client.close();
    }
}

// Run the test
testMCPConnection();