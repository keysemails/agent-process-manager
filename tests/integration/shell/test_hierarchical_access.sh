#!/bin/bash
# Test script for hierarchical access control

set -e

echo "=== Testing Hierarchical Access Control ==="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create test directory structure
TEST_ROOT="/tmp/apm_hierarchical_test"
rm -rf "$TEST_ROOT"
mkdir -p "$TEST_ROOT/project/backend"
mkdir -p "$TEST_ROOT/project/frontend"
mkdir -p "$TEST_ROOT/other_project"

echo -e "${YELLOW}Test directory structure:${NC}"
echo "$TEST_ROOT/"
echo "├── project/"
echo "│   ├── backend/"
echo "│   └── frontend/"
echo "└── other_project/"
echo

# Set APM binary path
APM="/Users/sunnya97/dev/sunnya97/vibe-workspace/agent-process-manager/target/debug/apm"

# Function to check if process is visible
check_process_visible() {
    local dir=$1
    local process_name=$2
    local should_see=$3
    
    cd "$dir"
    
    if $APM list 2>/dev/null | grep -q "^$process_name\\s"; then
        if [ "$should_see" = "yes" ]; then
            echo -e "${GREEN}✓${NC} From $dir: Can see $process_name (expected)"
        else
            echo -e "${RED}✗${NC} From $dir: Can see $process_name (NOT expected)"
            return 1
        fi
    else
        if [ "$should_see" = "no" ]; then
            echo -e "${GREEN}✓${NC} From $dir: Cannot see $process_name (expected)"
        else
            echo -e "${RED}✗${NC} From $dir: Cannot see $process_name (NOT expected)"
            return 1
        fi
    fi
}

# Build the project first
echo -e "${YELLOW}Building APM...${NC}"
cargo build
echo

# Kill any existing daemon and clean up database
echo -e "${YELLOW}Cleaning up any existing daemon and database...${NC}"
cd /Users/sunnya97/dev/sunnya97/vibe-workspace/agent-process-manager
kill -9 $(pgrep -f "target/debug/apm start") 2>/dev/null || true
sleep 1
rm -f apm.db apm.db-shm apm.db-wal

# Kill all APM tmux sessions to prevent orphans
echo -e "${YELLOW}Killing any orphaned tmux sessions...${NC}"
tmux list-sessions 2>/dev/null | grep "^apm-" | cut -d: -f1 | xargs -I {} tmux kill-session -t {} 2>/dev/null || true

# Start the daemon
echo -e "${YELLOW}Starting APM daemon...${NC}"
./target/debug/apm start &
DAEMON_PID=$!
sleep 3

# Cleanup function
cleanup() {
    echo -e "\n${YELLOW}Cleaning up...${NC}"
    # Stop all test processes
    cd "$TEST_ROOT"
    $APM stop --all backend-api 2>/dev/null || true
    $APM stop --all frontend-web 2>/dev/null || true
    $APM stop --all other-service 2>/dev/null || true
    
    # Kill daemon
    kill $DAEMON_PID 2>/dev/null || true
    
    # Clean up test directory
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

# Test 1: Spawn processes in different directories
echo -e "${YELLOW}Test 1: Spawning processes in different directories${NC}"

cd "$TEST_ROOT/project/backend"
$APM spawn backend-api echo "Backend API running"
echo -e "${GREEN}✓${NC} Spawned backend-api in backend/"

cd "$TEST_ROOT/project/frontend"
$APM spawn frontend-web echo "Frontend web running"
echo -e "${GREEN}✓${NC} Spawned frontend-web in frontend/"

cd "$TEST_ROOT/other_project"
$APM spawn other-service echo "Other service running"
echo -e "${GREEN}✓${NC} Spawned other-service in other_project/"

sleep 2
echo

# Test 2: Check visibility from different directories
echo -e "${YELLOW}Test 2: Testing hierarchical visibility${NC}"

# From backend directory (should only see backend-api)
check_process_visible "$TEST_ROOT/project/backend" "backend-api" "yes"
check_process_visible "$TEST_ROOT/project/backend" "frontend-web" "no"
check_process_visible "$TEST_ROOT/project/backend" "other-service" "no"
echo

# From frontend directory (should only see frontend-web)
check_process_visible "$TEST_ROOT/project/frontend" "frontend-web" "yes"
check_process_visible "$TEST_ROOT/project/frontend" "backend-api" "no"
check_process_visible "$TEST_ROOT/project/frontend" "other-service" "no"
echo

# From project directory (should see both backend and frontend, but not other)
check_process_visible "$TEST_ROOT/project" "backend-api" "yes"
check_process_visible "$TEST_ROOT/project" "frontend-web" "yes"
check_process_visible "$TEST_ROOT/project" "other-service" "no"
echo

# From test root (should see all)
check_process_visible "$TEST_ROOT" "backend-api" "yes"
check_process_visible "$TEST_ROOT" "frontend-web" "yes"
check_process_visible "$TEST_ROOT" "other-service" "yes"
echo

# Test 3: Test --all flag
echo -e "${YELLOW}Test 3: Testing --all flag${NC}"
cd "$TEST_ROOT/project/backend"
echo "From backend/ with --all flag:"
$APM list --all | grep -E "(backend-api|frontend-web|other-service)" || true
echo

# Test 4: Test cross-directory access control
echo -e "${YELLOW}Test 4: Testing access control${NC}"

# Try to stop frontend process from backend directory (should fail)
cd "$TEST_ROOT/project/backend"
if $APM stop frontend-web 2>&1 | grep -q "not found"; then
    echo -e "${GREEN}✓${NC} Cannot stop frontend-web from backend/ (expected)"
else
    echo -e "${RED}✗${NC} Was able to stop frontend-web from backend/ (NOT expected)"
fi

# Try to stop frontend process from project directory (should succeed)
cd "$TEST_ROOT/project"
if $APM stop frontend-web 2>&1 | grep -q "Stopped"; then
    echo -e "${GREEN}✓${NC} Can stop frontend-web from project/ (expected)"
else
    echo -e "${RED}✗${NC} Cannot stop frontend-web from project/ (NOT expected)"
fi

echo
echo -e "${GREEN}=== Hierarchical Access Tests Complete ===${NC}"