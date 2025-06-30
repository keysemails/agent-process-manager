#!/bin/bash

# Complete GitHub Issues Setup Script
# This script sets up the complete GitHub Issues workflow from scratch

set -e

echo "🚀 Setting up GitHub Issues workflow for Agent Process Manager..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check prerequisites
print_status "Checking prerequisites..."

if ! command -v gh &> /dev/null; then
    print_error "GitHub CLI (gh) is not installed."
    echo "Please install it from: https://cli.github.com/"
    exit 1
fi

if ! gh auth status &> /dev/null; then
    print_error "GitHub CLI is not authenticated."
    echo "Please run: gh auth login"
    exit 1
fi

if ! gh repo view &> /dev/null; then
    print_error "Not in a GitHub repository."
    echo "Please navigate to your GitHub repository directory."
    exit 1
fi

print_success "Prerequisites check passed!"

# Get repository information
REPO_OWNER=$(gh repo view --json owner --jq '.owner.login')
REPO_NAME=$(gh repo view --json name --jq '.name')
print_status "Repository: $REPO_OWNER/$REPO_NAME"

echo ""
print_status "This script will:"
echo "  1. Create comprehensive label system"
echo "  2. Set up issue templates"
echo "  3. Provide automation scripts"
echo "  4. Update documentation"
echo ""

read -p "Do you want to continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Setup cancelled."
    exit 0
fi

echo ""
print_status "Step 1: Creating label system..."

# Run the label creation script
if [ -f "./scripts/create-labels.sh" ]; then
    ./scripts/create-labels.sh
    print_success "Labels created successfully!"
else
    print_warning "Label creation script not found. Creating labels manually..."
    # Fallback to manual label creation
    gh label create "priority: critical" --color "d73a4a" --description "Urgent issues affecting core functionality" --force
    gh label create "priority: high" --color "ff9500" --description "Important features for next release" --force
    gh label create "priority: medium" --color "fbca04" --description "Nice-to-have improvements" --force
    gh label create "priority: low" --color "0075ca" --description "Future enhancements" --force
    print_success "Basic labels created!"
fi

echo ""
print_status "Step 2: Checking issue templates..."

if [ -d ".github/ISSUE_TEMPLATE" ]; then
    print_success "Issue templates directory exists!"
    
    template_count=$(find .github/ISSUE_TEMPLATE -name "*.yml" | wc -l)
    print_status "Found $template_count template files"
    
    if [ $template_count -eq 0 ]; then
        print_warning "No YAML templates found. Consider adding issue templates."
    fi
else
    print_warning "Issue templates directory not found."
    echo "Consider creating .github/ISSUE_TEMPLATE/ with:"
    echo "  - bug_report.yml"
    echo "  - feature_request.yml"
    echo "  - task.yml"
    echo "  - config.yml"
fi

echo ""
print_status "Step 3: Creating automation scripts..."

# Create issue query script
cat > scripts/query-issues.sh << 'EOF'
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
    "in-progress")
        echo "🔄 Issues currently in progress:"
        gh issue list --label "status: in-progress" --state open
        ;;
    "needs-review")
        echo "👀 Issues needing review:"
        gh issue list --label "status: needs-review" --state open
        ;;
    "all")
        echo "📋 All open issues:"
        gh issue list --state open
        ;;
    "stats")
        echo "📊 Issue Statistics:"
        echo ""
        echo "Total open issues: $(gh issue list --state open --json number | jq length)"
        echo "High priority: $(gh issue list --label "priority: high" --state open --json number | jq length)"
        echo "Ready for work: $(gh issue list --label "status: ready" --state open --json number | jq length)"
        echo "In progress: $(gh issue list --label "status: in-progress" --state open --json number | jq length)"
        echo ""
        echo "By component:"
        echo "  API: $(gh issue list --label "component: api" --state open --json number | jq length)"
        echo "  CLI: $(gh issue list --label "component: cli" --state open --json number | jq length)"
        echo "  Logs: $(gh issue list --label "component: logs" --state open --json number | jq length)"
        echo "  Process: $(gh issue list --label "component: process" --state open --json number | jq length)"
        ;;
    "help"|*)
        echo "GitHub Issues Query Helper"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  ready              Show issues ready for implementation"
        echo "  high-priority      Show high priority issues"
        echo "  good-first-issue   Show newcomer-friendly issues"
        echo "  api                Show API-related issues"
        echo "  cli                Show CLI-related issues"
        echo "  in-progress        Show issues currently being worked on"
        echo "  needs-review       Show issues needing review"
        echo "  all                Show all open issues"
        echo "  stats              Show issue statistics"
        echo "  help               Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 ready           # Find issues to work on"
        echo "  $0 stats           # Get overview of project status"
        echo "  $0 api             # Find API tasks"
        ;;
esac
EOF

chmod +x scripts/query-issues.sh
print_success "Created scripts/query-issues.sh"

# Create label management script
cat > scripts/manage-labels.sh << 'EOF'
#!/bin/bash

# Label Management Script
# Utilities for managing GitHub labels

set -e

case "${1:-help}" in
    "list")
        echo "📋 Current labels:"
        gh label list
        ;;
    "backup")
        echo "💾 Backing up labels to labels-backup.json..."
        gh label list --json name,description,color > labels-backup.json
        echo "Labels backed up successfully!"
        ;;
    "restore")
        if [ ! -f "labels-backup.json" ]; then
            echo "❌ No backup file found (labels-backup.json)"
            exit 1
        fi
        echo "🔄 Restoring labels from backup..."
        jq -r '.[] | "gh label create \"" + .name + "\" --color " + .color + " --description \"" + .description + "\" --force"' labels-backup.json | bash
        echo "Labels restored successfully!"
        ;;
    "clean")
        echo "🧹 This will delete ALL labels. Are you sure? (type 'yes' to confirm)"
        read -r confirmation
        if [ "$confirmation" = "yes" ]; then
            gh label list --json name --jq '.[].name' | xargs -I {} gh label delete {} --confirm
            echo "All labels deleted!"
        else
            echo "Operation cancelled."
        fi
        ;;
    "sync")
        echo "🔄 Syncing labels (creating missing ones)..."
        ./scripts/create-labels.sh
        ;;
    "help"|*)
        echo "Label Management Helper"
        echo ""
        echo "Usage: $0 <command>"
        echo ""
        echo "Commands:"
        echo "  list      List all current labels"
        echo "  backup    Backup labels to labels-backup.json"
        echo "  restore   Restore labels from backup file"
        echo "  clean     Delete all labels (dangerous!)"
        echo "  sync      Create missing labels from standard set"
        echo "  help      Show this help message"
        ;;
esac
EOF

chmod +x scripts/manage-labels.sh
print_success "Created scripts/manage-labels.sh"

echo ""
print_status "Step 4: Creating convenience aliases..."

# Create GitHub CLI aliases
gh alias set issues-ready "issue list --label 'status: ready' --state open"
gh alias set issues-mine "issue list --assignee @me --state open"
gh alias set issues-high "issue list --label 'priority: high' --state open"

print_success "Created GitHub CLI aliases:"
echo "  gh issues-ready    # Show ready issues"
echo "  gh issues-mine     # Show your assigned issues"
echo "  gh issues-high     # Show high priority issues"

echo ""
print_status "Step 5: Verification..."

# Verify setup
label_count=$(gh label list --json name | jq length)
print_status "Labels created: $label_count"

if [ -d ".github/ISSUE_TEMPLATE" ]; then
    template_count=$(find .github/ISSUE_TEMPLATE -name "*.yml" | wc -l)
    print_status "Issue templates: $template_count"
fi

issue_count=$(gh issue list --state open --json number | jq length)
print_status "Open issues: $issue_count"

echo ""
print_success "🎉 GitHub Issues setup complete!"
echo ""
echo "📝 Next steps:"
echo "  1. Review labels: gh label list"
echo "  2. Check issues: gh issue list"
echo "  3. Use query helper: ./scripts/query-issues.sh"
echo "  4. Manage labels: ./scripts/manage-labels.sh"
echo ""
echo "🔗 Quick links:"
echo "  Issues: https://github.com/$REPO_OWNER/$REPO_NAME/issues"
echo "  Labels: https://github.com/$REPO_OWNER/$REPO_NAME/labels"
echo "  New Issue: https://github.com/$REPO_OWNER/$REPO_NAME/issues/new/choose"
EOF

chmod +x scripts/github-setup.sh