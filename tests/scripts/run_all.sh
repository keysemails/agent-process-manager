#!/bin/bash

echo "🧪 Running Agent Process Manager Test Suite"
echo "=========================================="

# Set test environment
export RUST_LOG=agent_process_manager=debug
export RUST_BACKTRACE=1

# Kill any existing APM daemons
pkill -f "apm start" 2>/dev/null || true
sleep 1

# Change to project root
cd "$(dirname "$0")/../.."

echo ""
echo "📋 Running unit tests..."
cargo test --lib -- --nocapture

echo ""
echo "🔧 Running unit test files..."
find tests/unit -name "*.rs" -exec basename {} .rs \; | while read test; do
    echo "  Running $test..."
    cargo test --test "$test" -- --nocapture
done

echo ""
echo "🔧 Running Rust integration tests..."
find tests/integration/rust -name "*.rs" -exec basename {} .rs \; | while read test; do
    echo "  Running $test..."
    cargo test --test "$test" -- --test-threads=1 --nocapture
done

echo ""
echo "🐍 Running Python integration tests..."
if command -v python3 &> /dev/null && python3 -c "import pytest" 2>/dev/null; then
    cd tests/integration/python
    python3 -m pytest -v
    cd ../../..
else
    echo "  ⚠️  Skipping Python tests (pytest not available)"
fi

echo ""
echo "🐚 Running shell integration tests..."
for script in tests/integration/shell/test_*.sh; do
    if [ -f "$script" ]; then
        echo "  Running $(basename "$script")..."
        bash "$script"
        
        # Clean up between tests to avoid port conflicts
        echo "  Cleaning up after $(basename "$script")..."
        pkill -f "apm start" 2>/dev/null || true
        if [ -f ./target/debug/apm ]; then
            ./target/debug/apm stop-all --force 2>/dev/null || true
        elif [ -f ./target/release/apm ]; then
            ./target/release/apm stop-all --force 2>/dev/null || true
        fi
        sleep 1
    fi
done

echo ""
echo "🌍 Running end-to-end tests..."
if [ -f "tests/e2e/cli_e2e_test.rs" ]; then
    cargo test --test cli_e2e_test -- --test-threads=1 --nocapture
fi

if command -v python3 &> /dev/null && python3 -c "import pytest" 2>/dev/null; then
    cd tests/e2e
    if ls *.py 1> /dev/null 2>&1; then
        python3 -m pytest -v *.py
    fi
    cd ../..
else
    echo "  ⚠️  Skipping Python e2e tests (pytest not available)"
fi

echo ""
echo "✅ All tests completed!"

# Clean up
pkill -f "apm start" 2>/dev/null || true