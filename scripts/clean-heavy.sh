#!/usr/bin/env bash
# clean-heavy: reclaim disk by wiping BUILD CACHES only.
#
# Two things quietly fill the drive over a long-lived project:
#   1. The main Rust `target/` -- it ballooned to ~1.1 TB once (never cleaned across
#      ~859 releases). `cargo build` repopulates it in ~3-4 min, so it is disposable.
#   2. `.claude/worktrees/*/target/` -- every dispatched agent that compiled left a
#      ~7 GB target/ in its worktree; 40 of them added up to ~274 GB.
#
# This removes ONLY those build-artifact directories. Source, git history, and any
# unmerged commits inside a worktree are left completely untouched (this does NOT
# remove worktrees -- use `just clean-worktrees` for that, which is separately
# hardened to refuse worktrees holding real work).
#
# Added 2026-07-15 after the main target/ filled a 2 TB drive.
set -u

echo "clean-heavy: reclaiming disk (build caches only, no source touched)..."

echo "  wiping main target/ ..."
rm -rf target 2>/dev/null || true

n=0
for d in .claude/worktrees/*/target; do
  if [ -d "$d" ]; then
    rm -rf "$d" 2>/dev/null && n=$((n + 1))
  fi
done
echo "  wiped target/ from $n worktree(s)"

# Also clear the throwaway screenshot captures the render loop drops.
rm -f debug/screenshot_*.png debug/screenshot_done.json 2>/dev/null || true

echo "clean-heavy: done. Free space reclaimed."
echo "Run 'cargo build --features native' (or 'just play') to repopulate target/ (~3-4 min)."
