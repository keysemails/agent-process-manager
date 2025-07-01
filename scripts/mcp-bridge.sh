#!/bin/bash
# MCP TCP-to-stdio bridge for Claude Desktop
# This is now replaced by the built-in 'apm mcp-bridge' command

# Use the built-in bridge command
exec "$(dirname "$0")/../target/release/apm" mcp-bridge "$@"