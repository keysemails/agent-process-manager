#!/bin/bash

echo "🧪 Running Agent Process Manager Test Suite"
echo "=========================================="

# Set test environment
export RUST_LOG=agent_process_manager=debug
export RUST_BACKTRACE=1

# Kill any existing APM daemons
pkill -f "apm start" 2>/dev/null || true
sleep 1

# Run different test categories
echo ""
echo "📋 Running unit tests..."
cargo test --lib -- --nocapture

echo ""
echo "🔧 Running process supervisor tests..."
cargo test --test process_supervisor_test -- --test-threads=1 --nocapture

echo ""
echo "💾 Running log storage tests..."
cargo test --test log_storage_test -- --test-threads=1 --nocapture

echo ""
echo "🔍 Running pattern detection tests..."
cargo test --test log_patterns_test -- --nocapture

echo ""
echo "🌐 Running API integration tests..."
cargo test --test api_integration_test -- --test-threads=1 --nocapture

echo ""
echo "🖥️  Running CLI end-to-end tests..."
cargo test --test cli_e2e_test -- --test-threads=1 --nocapture

echo ""
echo "✅ All tests completed!"

# Clean up
pkill -f "apm start" 2>/dev/null || true