#!/usr/bin/env bash
# Clean stale git worktrees to prevent context rot.
#
# AI agents sometimes write to cached stale paths from old worktrees,
# causing edits to go to dead branches. This script removes all
# worktrees except main and the currently active one.
#
# Usage:
#   ./scripts/clean-worktrees.sh            # Interactive (confirms before removing)
#   ./scripts/clean-worktrees.sh --yes      # Skip confirmation
#   ./scripts/clean-worktrees.sh --all      # Remove ALL worktrees including current
#
# Safe to run anytime. Never touches main, never touches the current worktree
# unless --all is passed.

set -e

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO_ROOT" ]; then
    echo "ERROR: Not in a git repository"
    exit 1
fi

cd "$REPO_ROOT"

# Find the current worktree (where we're running from)
CURRENT_WT=$(git rev-parse --show-toplevel)

# List all worktrees except main and the current one
STALE_WTS=$(git worktree list --porcelain | awk '/^worktree/ {print $2}' | \
    grep -v "^${REPO_ROOT}$" | \
    grep -v "^${CURRENT_WT}$" || true)

AUTO_YES=false
REMOVE_ALL=false
for arg in "$@"; do
    case "$arg" in
        --yes|-y) AUTO_YES=true ;;
        --all) REMOVE_ALL=true ;;
    esac
done

if [ "$REMOVE_ALL" = true ]; then
    # Include current worktree but not main
    STALE_WTS=$(git worktree list --porcelain | awk '/^worktree/ {print $2}' | \
        grep -v "^${REPO_ROOT}$" || true)
fi

if [ -z "$STALE_WTS" ]; then
    echo "No stale worktrees to clean."
    echo ""
    echo "Current worktrees:"
    git worktree list
    exit 0
fi

echo "Found stale worktrees to remove:"
echo "$STALE_WTS" | while read wt; do
    size=$(du -sh "$wt" 2>/dev/null | cut -f1)
    echo "  - $wt ($size)"
done

if [ "$AUTO_YES" != true ]; then
    echo ""
    read -p "Remove these worktrees? (y/N) " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo "Aborted."
        exit 0
    fi
fi

echo ""
echo "Removing worktrees..."
echo "$STALE_WTS" | while read wt; do
    if [ -n "$wt" ]; then
        git worktree remove --force "$wt" 2>&1 | head -1
    fi
done

# Prune git's internal worktree tracking
git worktree prune

# Delete orphaned branches (claude/* and worktree-agent-* that no longer have a worktree)
echo ""
echo "Cleaning orphaned branches..."
for branch in $(git branch --format='%(refname:short)' | grep -E '^(claude/|worktree-agent-)'); do
    # Skip the current branch
    current=$(git rev-parse --abbrev-ref HEAD)
    if [ "$branch" = "$current" ]; then
        continue
    fi
    # Check if branch has an active worktree
    if ! git worktree list --porcelain | grep -q "branch refs/heads/$branch"; then
        git branch -D "$branch" 2>&1 | head -1
    fi
done

# Clean up any orphaned folders in .claude/worktrees/ (folders that aren't git worktrees)
if [ -d "$REPO_ROOT/.claude/worktrees" ]; then
    for dir in "$REPO_ROOT/.claude/worktrees"/*/; do
        if [ -d "$dir" ]; then
            wt_name=$(basename "$dir")
            if ! git worktree list --porcelain | grep -q "$dir"; then
                echo "Removing orphaned folder: $wt_name"
                rm -rf "$dir"
            fi
        fi
    done
fi

echo ""
echo "Done. Remaining worktrees:"
git worktree list

# Show space recovered
if command -v du >/dev/null 2>&1; then
    remaining=$(du -sh "$REPO_ROOT/.claude/worktrees" 2>/dev/null | cut -f1)
    echo ""
    echo "Total worktree storage: $remaining"
fi
