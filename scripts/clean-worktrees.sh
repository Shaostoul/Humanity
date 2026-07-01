#!/usr/bin/env bash
# Clean stale git worktrees to prevent context rot.
#
# AI agents sometimes write to cached stale paths from old worktrees,
# causing edits to go to dead branches. This script removes worktrees
# except main and the currently active one -- but ONLY those that are
# safe to remove: no uncommitted changes, AND every commit on the
# worktree's branch already merged into main. A worktree holding real
# unmerged work is SKIPPED and reported, never silently destroyed.
#
# INCIDENT (2026-07-01, hit 3 times in one day): this script used to
# remove everything unconditionally once --yes was passed, with no
# content check at all. `just clean-worktrees` bakes in --yes. CLAUDE.md
# Step 0 tells every session to run `just clean-worktrees` first -- and a
# subagent told to "read CLAUDE.md first" (a routine review-agent
# instruction) reads that literally and runs it as ITS OWN first action,
# force-deleting every sibling worktree mid-review with zero recovery
# path (no stash, no reflog, no dangling branch -- git worktree remove
# --force deletes the branch's only ref along with the directory). This
# destroyed three different agents' completed, unmerged work the same
# day. The fix is structural, not a doc warning (a doc warning already
# failed twice): a worktree with real work now survives --yes by
# default; only --force-unmerged destroys it.
#
# Usage:
#   ./scripts/clean-worktrees.sh                  # Interactive (confirms before removing)
#   ./scripts/clean-worktrees.sh --yes            # Skip confirmation (still skips unsafe worktrees)
#   ./scripts/clean-worktrees.sh --all            # Also consider the current worktree (still safety-checked)
#   ./scripts/clean-worktrees.sh --force-unmerged # Also destroy worktrees with real unmerged/uncommitted work
#
# NEVER run this from inside a dispatched subagent's own task. It is an
# operator/orchestrator hygiene command for the START of a top-level
# session, not a step a delegated worktree task should take on itself --
# it cannot know whether sibling worktrees hold someone else's unmerged
# work.

set -e

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO_ROOT" ]; then
    echo "ERROR: Not in a git repository"
    exit 1
fi

cd "$REPO_ROOT"

MAIN_BRANCH="main"

# Find the current worktree (where we're running from)
CURRENT_WT=$(git rev-parse --show-toplevel)

AUTO_YES=false
REMOVE_ALL=false
FORCE_UNMERGED=false
for arg in "$@"; do
    case "$arg" in
        --yes|-y) AUTO_YES=true ;;
        --all) REMOVE_ALL=true ;;
        --force-unmerged) FORCE_UNMERGED=true ;;
    esac
done

# List all worktrees except main and the current one
STALE_WTS=$(git worktree list --porcelain | awk '/^worktree/ {print $2}' | \
    grep -v "^${REPO_ROOT}$" | \
    grep -v "^${CURRENT_WT}$" || true)

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

# Partition into SAFE (no uncommitted changes, branch fully merged into main)
# and UNSAFE (real work that would be permanently lost if removed).
SAFE_LIST=()
UNSAFE_LIST=()
while IFS= read -r wt; do
    [ -z "$wt" ] && continue
    dirty=$(git -C "$wt" status --porcelain 2>/dev/null | head -1)
    wt_head=$(git -C "$wt" rev-parse HEAD 2>/dev/null || echo "")
    merged=true
    if [ -n "$wt_head" ] && ! git merge-base --is-ancestor "$wt_head" "refs/heads/$MAIN_BRANCH" 2>/dev/null; then
        merged=false
    fi
    if [ -n "$dirty" ] || [ "$merged" = false ]; then
        UNSAFE_LIST+=("$wt")
    else
        SAFE_LIST+=("$wt")
    fi
done <<< "$STALE_WTS"

if [ ${#UNSAFE_LIST[@]} -gt 0 ]; then
    echo "SKIPPING ${#UNSAFE_LIST[@]} worktree(s) with real unmerged/uncommitted work (not touched):"
    for wt in "${UNSAFE_LIST[@]}"; do
        echo "  - $wt  <-- has uncommitted changes and/or commits not merged into $MAIN_BRANCH"
    done
    if [ "$FORCE_UNMERGED" != true ]; then
        echo "  (pass --force-unmerged to remove these anyway -- this permanently destroys that work)"
    fi
    echo ""
fi

REMOVE_LIST=("${SAFE_LIST[@]}")
if [ "$FORCE_UNMERGED" = true ]; then
    REMOVE_LIST+=("${UNSAFE_LIST[@]}")
fi

if [ ${#REMOVE_LIST[@]} -eq 0 ]; then
    echo "No worktrees are safe to remove (all remaining worktrees hold real work)."
    exit 0
fi

echo "Found worktrees safe to remove:"
for wt in "${REMOVE_LIST[@]}"; do
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
for wt in "${REMOVE_LIST[@]}"; do
    git worktree remove --force "$wt" 2>&1 | head -1
done

# Prune git's internal worktree tracking
git worktree prune

# Delete orphaned branches (claude/* and worktree-agent-* that no longer have
# a worktree), but only if fully merged into main -- same protection as above.
echo ""
echo "Cleaning orphaned branches..."
current=$(git rev-parse --abbrev-ref HEAD)
for branch in $(git branch --format='%(refname:short)' | grep -E '^(claude/|worktree-agent-)'); do
    if [ "$branch" = "$current" ]; then
        continue
    fi
    if git worktree list --porcelain | grep -q "branch refs/heads/$branch"; then
        continue
    fi
    branch_head=$(git rev-parse "refs/heads/$branch" 2>/dev/null || echo "")
    if [ -n "$branch_head" ] && git merge-base --is-ancestor "$branch_head" "refs/heads/$MAIN_BRANCH" 2>/dev/null; then
        git branch -D "$branch" 2>&1 | head -1
    elif [ "$FORCE_UNMERGED" = true ]; then
        git branch -D "$branch" 2>&1 | head -1
    else
        echo "  skipping unmerged branch: $branch (pass --force-unmerged to delete anyway)"
    fi
done

# Clean up orphaned folders in .claude/worktrees/ that aren't registered as
# git worktrees at all -- but ONLY if they have no .git anchor (truly
# data-free) or, if they do, the same merged+clean check as above.
if [ -d "$REPO_ROOT/.claude/worktrees" ]; then
    for dir in "$REPO_ROOT/.claude/worktrees"/*/; do
        [ -d "$dir" ] || continue
        wt_name=$(basename "$dir")
        if git worktree list --porcelain | grep -q "$dir"; then
            continue
        fi
        if [ ! -e "$dir/.git" ]; then
            echo "Removing orphaned folder (no .git, no history to lose): $wt_name"
            rm -rf "$dir"
            continue
        fi
        dirty=$(git -C "$dir" status --porcelain 2>/dev/null | head -1)
        dir_head=$(git -C "$dir" rev-parse HEAD 2>/dev/null || echo "")
        merged=true
        if [ -n "$dir_head" ] && ! git merge-base --is-ancestor "$dir_head" "refs/heads/$MAIN_BRANCH" 2>/dev/null; then
            merged=false
        fi
        if [ -n "$dirty" ] || [ "$merged" = false ]; then
            if [ "$FORCE_UNMERGED" = true ]; then
                echo "Removing orphaned folder with unmerged work (--force-unmerged): $wt_name"
                rm -rf "$dir"
            else
                echo "SKIPPING orphaned folder with real unmerged/uncommitted work: $wt_name (pass --force-unmerged to remove anyway)"
            fi
        else
            echo "Removing orphaned folder (clean, fully merged): $wt_name"
            rm -rf "$dir"
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
