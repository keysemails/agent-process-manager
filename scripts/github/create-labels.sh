#!/bin/bash

# GitHub Labels Creation Script for Agent Process Manager
# This script creates a comprehensive label system for better issue organization

set -e

echo "🏷️  Creating GitHub labels for Agent Process Manager..."

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI (gh) is not installed. Please install it first."
    echo "   Visit: https://cli.github.com/"
    exit 1
fi

# Check if we're in a git repository with GitHub remote
if ! gh repo view &> /dev/null; then
    echo "❌ Not in a GitHub repository or not authenticated with gh CLI"
    echo "   Run 'gh auth login' first"
    exit 1
fi

echo "✅ GitHub CLI detected and authenticated"

# Function to create a label with error handling
create_label() {
    local name="$1"
    local color="$2"
    local description="$3"
    
    echo "Creating label: $name"
    if gh label create "$name" --color "$color" --description "$description" 2>/dev/null; then
        echo "  ✅ Created: $name"
    else
        echo "  ⚠️  Label '$name' may already exist or failed to create"
    fi
}

echo ""
echo "📋 Creating Priority Labels..."
create_label "priority: critical" "d73a4a" "Urgent issues affecting core functionality"
create_label "priority: high" "ff9500" "Important features for next release"
create_label "priority: medium" "fbca04" "Nice-to-have improvements"
create_label "priority: low" "0075ca" "Future enhancements"

echo ""
echo "🏗️  Creating Type Labels..."
create_label "type: bug" "d73a4a" "Something isn't working"
create_label "type: feature" "0e8a16" "New feature request"
create_label "type: enhancement" "1d76db" "Improvement to existing feature"
create_label "type: documentation" "5319e7" "Documentation updates"
create_label "type: refactor" "6c757d" "Code structure improvements"
create_label "type: chore" "e4e6ea" "Maintenance tasks"

echo ""
echo "🔧 Creating Component Labels..."
create_label "component: api" "1d76db" "REST API and handlers"
create_label "component: cli" "0e8a16" "Command line interface"
create_label "component: logs" "fbca04" "Log storage and pattern detection"
create_label "component: process" "ff9500" "Process management and supervision"
create_label "component: tmux" "5319e7" "Terminal integration"
create_label "component: websocket" "00d2d3" "Real-time streaming"
create_label "component: ai-agent" "e91e63" "AI agent API features"
create_label "component: config" "8b4513" "Configuration management"

echo ""
echo "📊 Creating Status Labels..."
create_label "status: needs-research" "fff2cc" "Requires investigation"
create_label "status: blocked" "d73a4a" "Cannot proceed due to dependencies"
create_label "status: ready" "0e8a16" "Ready for implementation"
create_label "status: in-progress" "1d76db" "Currently being worked on"
create_label "status: needs-review" "5319e7" "Awaiting code review"

echo ""
echo "⭐ Creating Special Labels..."
create_label "good-first-issue" "7057ff" "Good for newcomers"
create_label "breaking-change" "d73a4a" "Will require version bump"
create_label "security" "d73a4a" "Security-related improvements"
create_label "performance" "ff9500" "Performance optimization"
create_label "architecture" "5319e7" "Architectural decisions"

echo ""
echo "🎉 Label creation complete!"
echo ""
echo "📝 Next steps:"
echo "   1. Review created labels in your GitHub repository"
echo "   2. Run the issue migration script to convert TASKS.md"
echo "   3. Set up issue templates"
echo ""
echo "💡 You can view all labels with: gh label list"