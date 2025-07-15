#!/bin/bash

# Test script for database persistence

echo "Building APM..."
cargo build --release

# Clean up any existing daemon first
echo "Cleaning up any existing daemons..."
./target/release/apm stop-all --force 2>/dev/null || true
sleep 2

echo -e "\n=== Test 1: Start daemon and spawn a process ==="
echo "Starting APM daemon..."
RUST_LOG=agent_process_manager=info ./target/release/apm start &
APM_PID=$!

# Wait for daemon to start
sleep 2

echo "Spawning test process..."
./target/release/apm spawn test-server python3 -- -m http.server 8888

echo "Listing processes..."
./target/release/apm list

echo "Getting process ID..."
PROCESS_ID=$(./target/release/apm list | grep test-server | awk '{print $1}')
echo "Process ID: $PROCESS_ID"

echo -e "\n=== Test 2: Stop daemon (simulating crash) ==="
echo "Stopping daemon..."
kill $APM_PID
sleep 2

echo "Checking tmux sessions (should still exist)..."
tmux list-sessions | grep apm-

echo -e "\n=== Test 3: Restart daemon (should recover orphaned sessions) ==="
echo "Starting APM daemon again..."
RUST_LOG=agent_process_manager=info ./target/release/apm start &
APM_PID=$!
sleep 2

echo "Listing processes (should show recovered process)..."
./target/release/apm list

echo -e "\n=== Test 4: Clean up ==="
echo "Stopping test process..."
./target/release/apm stop test-server 2>/dev/null || true

echo "Stopping daemon..."
./target/release/apm stop-all --force 2>/dev/null || true
kill $APM_PID 2>/dev/null || true
sleep 1

echo -e "\nTest complete!"