#!/bin/bash

# Issue Query Helper Script
# Provides quick filters for common issue queries

set -e

case "${1:-help}" in
    "ready")
        echo "🟢 Issues ready for implementation:"
        gh issue list --label "status: ready" --state open
        ;;
    "high-priority")
        echo "🔴 High priority issues:"
        gh issue list --label "priority: high" --state open
        ;;
    "good-first-issue")
        echo "👋 Good first issues for newcomers:"
        gh issue list --label "good-first-issue" --state open
        ;;
    "api")
        echo "🔌 API-related issues:"
        gh issue list --label "component: api" --state open
        ;;
    "cli")
        echo "💻 CLI-related issues:"
        gh issue list --label "component: cli" --state open
        ;;
    "logs")
        echo "📝 Log-related issues:"
        gh issue list --label "component: logs" --state open
        ;;
    "process")
        echo "⚙️  Process management issues:"
        gh issue list --label "component: process" --state open
        ;;
    "tmux")
        echo "🖥️  tmux integration issues:"
        gh issue list --label "component: tmux" --state open
        ;;
    "ai-agent")
        echo "🤖 AI agent issues:"
        gh issue list --label "component: ai-agent" --state open
        ;;
    "in-progress")
        echo "🔄 Issues currently in progress:"
        gh issue list --label "status: in-progress" --state open
        ;;
    "needs-review")
        echo "👀 Issues needing review:"
        gh issue list --label "status: needs-review" --state open
        ;;
    "blocked")
        echo "🚫 Blocked issues:"
        gh issue list --label "status: blocked" --state open
        ;;
    "bugs")
        echo "🐛 Bug reports:"
        gh issue list --label "type: bug" --state open
        ;;
    "features")
        echo "✨ Feature requests:"
        gh issue list --label "type: feature" --state open
        ;;
    "all")
        echo "📋 All open issues:"
        gh issue list --state open
        ;;
    "mine")
        echo "👤 Issues assigned to you:"
        gh issue list --assignee @me --state open
        ;;
    "stats")
        echo "📊 Issue Statistics:"
        echo ""
        total_open=$(gh issue list --state open --json number | jq length)
        total_closed=$(gh issue list --state closed --json number | jq length)
        echo "Total issues: $((total_open + total_closed)) (open: $total_open, closed: $total_closed)"
        echo ""
        echo "By Priority:"
        echo "  🔴 Critical: $(gh issue list --label "priority: critical" --state open --json number | jq length)"
        echo "  🟠 High: $(gh issue list --label "priority: high" --state open --json number | jq length)"
        echo "  🟡 Medium: $(gh issue list --label "priority: medium" --state open --json number | jq length)"
        echo "  🔵 Low: $(gh issue list --label "priority: low" --state open --json number | jq length)"
        echo ""
        echo "By Status:"
        echo "  ✅ Ready: $(gh issue list --label "status: ready" --state open --json number | jq length)"
        echo "  🔄 In Progress: $(gh issue list --label "status: in-progress" --state open --json number | jq length)"
        echo "  👀 Needs Review: $(gh issue list --label "status: needs-review" --state open --json number | jq length)"
        echo "  🚫 Blocked: $(gh issue list --label "status: blocked" --state open --json number | jq length)"
        echo "  🔍 Needs Research: $(gh issue list --label "status: needs-research" --state open --json number | jq length)"
        echo ""
        echo "By Component:"
        echo "  🔌 API: $(gh issue list --label "component: api" --state open --json number | jq length)"
        echo "  💻 CLI: $(gh issue list --label "component: cli" --state open --json number | jq length)"
        echo "  📝 Logs: $(gh issue list --label "component: logs" --state open --json number | jq length)"
        echo "  ⚙️  Process: $(gh issue list --label "component: process" --state open --json number | jq length)"
        echo "  🖥️  tmux: $(gh issue list --label "component: tmux" --state open --json number | jq length)"
        echo "  🤖 AI Agent: $(gh issue list --label "component: ai-agent" --state open --json number | jq length)"
        ;;
    "help"|*)
        echo "GitHub Issues Query Helper for Agent Process Manager"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "🎯 Status Filters:"
        echo "  ready              Show issues ready for implementation"
        echo "  in-progress        Show issues currently being worked on"
        echo "  needs-review       Show issues needing review"
        echo "  blocked            Show blocked issues"
        echo ""
        echo "🔥 Priority Filters:"
        echo "  high-priority      Show high priority issues"
        echo ""
        echo "🏗️  Component Filters:"
        echo "  api                Show API-related issues"
        echo "  cli                Show CLI-related issues"
        echo "  logs               Show log management issues"
        echo "  process            Show process management issues"
        echo "  tmux               Show tmux integration issues"
        echo "  ai-agent           Show AI agent issues"
        echo ""
        echo "📝 Type Filters:"
        echo "  bugs               Show bug reports"
        echo "  features           Show feature requests"
        echo ""
        echo "👤 Personal:"
        echo "  mine               Show issues assigned to you"
        echo "  good-first-issue   Show newcomer-friendly issues"
        echo ""
        echo "📊 Overview:"
        echo "  all                Show all open issues"
        echo "  stats              Show comprehensive statistics"
        echo "  help               Show this help message"
        echo ""
        echo "💡 Examples:"
        echo "  $0 ready           # Find issues to work on"
        echo "  $0 stats           # Get project overview"
        echo "  $0 api             # Find API tasks"
        echo "  $0 mine            # See your assigned issues"
        ;;
esac