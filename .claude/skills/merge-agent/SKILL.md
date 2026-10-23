---
name: merge-agent
description: Review, verify and merge one finished agent's worktree branch into main in the shared HumanityOS checkout, keeping its rationale, then push. Use when an agent hands back its report.
---

# Merge an agent's work

An agent's report is a claim, not evidence. Its mistakes arrive in the same confident tone as its
findings. Verify the load-bearing parts yourself before anything lands on main.

## 1. See what is really there

```bash
WT=C:/Humanity/.claude/worktrees/agent-<id>; B=worktree-agent-<id>
git -C $WT log --oneline main..HEAD          # its commits
git -C $WT status --short                    # uncommitted work? have the agent commit it, never lose it
git diff --stat main...$B
cat $WT/.claude/handoff/*.md 2>/dev/null     # its own notes, if it wrote any
```

## 2. Review the diff (read it; do not skim the report instead)

- Removed or weakened tests: `git diff main...$B | grep -nE '^-\s*(#\[test\]|assert|it\(|test\()'`.
  Every one needs a reason in the report.
- Commits made with `--no-verify`, `cargo fmt` churn (200+ files), files outside its scope, other
  agents' work swept in.
- Rules: no hardcoded colours (theme tokens), no list literals of domain objects (data files),
  native and web kept in step, `#[cfg(feature = "native")]` on GUI or renderer modules, no new
  em dashes in docs, every persisted-format change has an old-shape test (migration-guard).
- Did it actually see its new tests fail first (red) before they passed? Ask if the report does
  not say.

## 3. Merge, keeping the rationale

In `C:\Humanity` on main (never in another agent's worktree):

```bash
git merge --squash $B                        # stages exactly the branch's changes
git commit -F <msg-file>                     # the agent's why, condensed; Co-Authored-By line last
```

A conflict with another session's uncommitted edits: stop and look, never stash or overwrite
their files. A shared file holding both: the hunk-selective recipe in
`docs/ai/orchestrator.md`, section 8.

## 4. Verify on main, only what the change touches

- Rust: `cargo check --features native` and `cargo check --features relay --no-default-features`.
- Its tests: `just test-lib <filter>`; relay changes: `just verify-relay`.
- `just lints`; `just rig-tests` if scripts changed; `just validate-data` if data changed.
- Renderer, shader, pipeline or startup changes: `just verify-runtime` now, or note it for the
  release batch if the change cannot reach the screen yet.
- Never re-run a battery that already passed on the same tree.

## 5. Push and tidy

```bash
git push origin main
rm -rf $WT/target                            # frees 8-10 GB; worktree and branch stay
```

Record it: FEATURES.md for a new feature, BUGS.md for a fix, PRIORITIES.md if scope moved, a
`recent_decisions` entry for a significant choice. Releases are batched (the `release` skill).
After a wave of parallel agents has merged, run a critic over the batch: systems built side by
side disagree at the seams.
