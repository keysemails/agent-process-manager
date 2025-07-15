#!/bin/bash

# Label Management Script
# Utilities for managing GitHub labels

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_success() {
    echo -e "${GREEN}✅${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠️${NC} $1"
}

print_error() {
    echo -e "${RED}❌${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ️${NC} $1"
}

case "${1:-help}" in
    "list")
        echo "📋 Current labels:"
        echo ""
        gh label list --json name,description,color | jq -r '.[] | "• \(.name) (\(.color)) - \(.description)"' | sort
        echo ""
        echo "Total labels: $(gh label list --json name | jq length)"
        ;;
    "backup")
        backup_file="labels-backup-$(date +%Y%m%d-%H%M%S).json"
        print_info "Backing up labels to $backup_file..."
        gh label list --json name,description,color > "$backup_file"
        print_success "Labels backed up to $backup_file"
        
        # Also create a human-readable backup
        backup_txt="labels-backup-$(date +%Y%m%d-%H%M%S).txt"
        echo "# GitHub Labels Backup - $(date)" > "$backup_txt"
        echo "# Repository: $(gh repo view --json nameWithOwner --jq '.nameWithOwner')" >> "$backup_txt"
        echo "" >> "$backup_txt"
        gh label list --json name,description,color | jq -r '.[] | "Label: \(.name)\nColor: #\(.color)\nDescription: \(.description)\n"' >> "$backup_txt"
        print_success "Human-readable backup saved to $backup_txt"
        ;;
    "restore")
        if [ -z "$2" ]; then
            print_error "Please specify backup file: $0 restore <backup-file.json>"
            echo ""
            echo "Available backup files:"
            ls -la labels-backup-*.json 2>/dev/null || echo "No backup files found"
            exit 1
        fi
        
        if [ ! -f "$2" ]; then
            print_error "Backup file '$2' not found"
            exit 1
        fi
        
        print_info "Restoring labels from $2..."
        
        # Validate JSON format
        if ! jq empty "$2" 2>/dev/null; then
            print_error "Invalid JSON format in backup file"
            exit 1
        fi
        
        # Count labels to restore
        label_count=$(jq length "$2")
        print_info "Found $label_count labels to restore"
        
        echo "This will create/update labels. Continue? (y/N)"
        read -r confirmation
        if [[ ! $confirmation =~ ^[Yy]$ ]]; then
            echo "Operation cancelled."
            exit 0
        fi
        
        # Restore labels
        jq -r '.[] | "gh label create \"" + .name + "\" --color " + .color + " --description \"" + (.description // "") + "\" --force"' "$2" | while read -r cmd; do
            eval "$cmd" && echo "  ✓ Label restored" || echo "  ✗ Failed to restore label"
        done
        
        print_success "Label restoration complete!"
        ;;
    "clean")
        print_warning "⚠️  DANGER: This will delete ALL labels from the repository!"
        print_warning "This action cannot be undone!"
        echo ""
        echo "Repository: $(gh repo view --json nameWithOwner --jq '.nameWithOwner')"
        echo "Current label count: $(gh label list --json name | jq length)"
        echo ""
        echo "Type 'DELETE ALL LABELS' to confirm:"
        read -r confirmation
        
        if [ "$confirmation" = "DELETE ALL LABELS" ]; then
            print_info "Creating backup before deletion..."
            backup_file="labels-backup-before-clean-$(date +%Y%m%d-%H%M%S).json"
            gh label list --json name,description,color > "$backup_file"
            print_success "Backup created: $backup_file"
            
            print_info "Deleting all labels..."
            gh label list --json name --jq '.[].name' | while read -r label; do
                if gh label delete "$label" --yes 2>/dev/null; then
                    echo "  ✓ Deleted: $label"
                else
                    echo "  ✗ Failed to delete: $label"
                fi
            done
            print_success "All labels deleted!"
        else
            echo "Operation cancelled. (You must type exactly 'DELETE ALL LABELS')"
        fi
        ;;
    "sync")
        print_info "Syncing labels (creating missing standard labels)..."
        if [ -f "./scripts/create-labels.sh" ]; then
            ./scripts/create-labels.sh
        else
            print_error "create-labels.sh script not found"
            print_info "Creating basic labels manually..."
            
            # Create essential labels if they don't exist
            gh label create "priority: high" --color "ff9500" --description "Important features for next release" --force
            gh label create "type: bug" --color "d73a4a" --description "Something isn't working" --force
            gh label create "type: feature" --color "0e8a16" --description "New feature request" --force
            gh label create "status: ready" --color "0e8a16" --description "Ready for implementation" --force
            
            print_success "Basic labels created"
        fi
        ;;
    "missing")
        print_info "Checking for missing standard labels..."
        
        # Define expected labels
        expected_labels=(
            "priority: critical"
            "priority: high" 
            "priority: medium"
            "priority: low"
            "type: bug"
            "type: feature"
            "type: enhancement"
            "status: ready"
            "status: in-progress"
            "good-first-issue"
        )
        
        existing_labels=$(gh label list --json name --jq '.[].name')
        missing_count=0
        
        for label in "${expected_labels[@]}"; do
            if ! echo "$existing_labels" | grep -q "^$label$"; then
                if [ $missing_count -eq 0 ]; then
                    echo "Missing labels:"
                fi
                echo "  • $label"
                ((missing_count++))
            fi
        done
        
        if [ $missing_count -eq 0 ]; then
            print_success "All standard labels are present!"
        else
            print_warning "Found $missing_count missing labels"
            echo ""
            echo "Run '$0 sync' to create missing labels"
        fi
        ;;
    "stats")
        print_info "Label Statistics:"
        echo ""
        
        total_labels=$(gh label list --json name | jq length)
        echo "Total labels: $total_labels"
        echo ""
        
        echo "Label breakdown:"
        echo "Priority labels: $(gh label list --json name --jq '[.[].name | select(startswith("priority:"))] | length')"
        echo "Type labels: $(gh label list --json name --jq '[.[].name | select(startswith("type:"))] | length')"
        echo "Component labels: $(gh label list --json name --jq '[.[].name | select(startswith("component:"))] | length')"
        echo "Status labels: $(gh label list --json name --jq '[.[].name | select(startswith("status:"))] | length')"
        echo ""
        
        # Count issues using each label category
        echo "Issue usage:"
        priority_issues=$(gh issue list --label "priority: high" --state all --json number | jq length)
        bug_issues=$(gh issue list --label "type: bug" --state all --json number | jq length)
        feature_issues=$(gh issue list --label "type: feature" --state all --json number | jq length)
        
        echo "High priority issues: $priority_issues"
        echo "Bug reports: $bug_issues"
        echo "Feature requests: $feature_issues"
        ;;
    "help"|*)
        echo "Label Management Helper for Agent Process Manager"
        echo ""
        echo "Usage: $0 <command> [options]"
        echo ""
        echo "Commands:"
        echo "  📋 list                    List all current labels with descriptions"
        echo "  💾 backup                 Backup labels to timestamped files"
        echo "  🔄 restore <file.json>    Restore labels from backup file"
        echo "  🧹 clean                  Delete ALL labels (dangerous!)"
        echo "  🔄 sync                   Create missing standard labels"
        echo "  🔍 missing                Check for missing standard labels"
        echo "  📊 stats                  Show label statistics and usage"
        echo "  ❓ help                   Show this help message"
        echo ""
        echo "Examples:"
        echo "  $0 backup                           # Create backup"
        echo "  $0 restore labels-backup.json      # Restore from backup"
        echo "  $0 missing                          # Check missing labels"
        echo "  $0 sync                             # Add missing labels"
        echo ""
        echo "Safety Features:"
        echo "  • Automatic backups before destructive operations"
        echo "  • Confirmation prompts for dangerous operations"
        echo "  • JSON validation for restore operations"
        echo "  • Human-readable backup files"
        ;;
esac