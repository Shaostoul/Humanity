---
name: release
description: Cut a HumanityOS release from the batch merged since the last tag (bump, web bundle, release build, runtime checks, commit, tag, archive, deliver). Use once or twice a day on the batch, never per feature (operator, 2026-10-22).
---

# Release the batch

A release costs 30 to 45 minutes of the one machine (release build ~8 min, lib tests ~7.5 min,
rigs ~9 min), which is why it is batched. Between releases, merges are pushed with only the checks
they touch (see the `merge-agent` skill). Work in `C:\Humanity` on `main`. Several sessions may
share this checkout: stage by name, never sweep.

## 0. Before starting

- `git fetch origin && git status -sb`: main must equal origin/main (push merged work first).
- `git status --short`: stage any untracked `.rs`/`.ron`/`.csv` of the batch BY NAME (CI builds a
  fresh checkout; untracked files compile here and fail there).
- If `Cargo.toml` is already bumped and uncommitted, that is another session's pending stamp: do
  not bump again; coordinate (CLAUDE.md "Version bumps collide").
- `node scripts/check-updates.js`: if a component update is due, do it in this quiet window first
  (docs/SOP.md "Keeping components up to date").

## 1. Bump, and read the version back

```bash
LAST=$(git describe --tags --abbrev=0)
git diff --quiet "$LAST"..HEAD -- src Cargo.toml && KIND=patch || KIND=minor   # Rust changed: minor
node scripts/bump-version.js $KIND
V=$(node -p "require('fs').readFileSync('Cargo.toml','utf8').match(/^version\s*=\s*\"(.+?)\"/m)[1]")
[ -n "$V" ] || exit 1
```

Never type the version from memory. The bump touches eight files (Cargo.toml, Cargo.lock, and the
six web stamps `scripts/bump-version.js` lists).

## 2. Web bundle and release build

```bash
node scripts/bundle-web.js                        # refreshes app/web, the desktop's offline copy
CARGO_BUILD_JOBS=4 cargo build --features native --release
cargo check --features relay --no-default-features  # the deploy feature set; also settles Cargo.lock
```

## 3. Checks (on the release exe)

- `just lints` and `just rig-tests`.
- The lib tests unless they already passed on this exact tree: `just test-lib`.
- `just verify-runtime` (boots the release exe, enters the world, fails on any PANIC). Mandatory
  when the batch touched the renderer, shaders, pipelines, bind groups or startup; run it anyway
  for any Rust release.
- The rigs the batch touched (the `game-checks` skill lists them).
- One HumanityOS instance at a time: `tasklist | grep -i humanityos`; close the operator's game
  gracefully if it is open (`taskkill //PID <pid>`, force only after ~15 s).

A failed check: fix it (or take the change out of the batch) before tagging. The bump can stay
uncommitted meanwhile; do not tag a red batch.

## 4. Commit, push, tag

Write the notes to a file in the scratchpad (what changed for a person using it, from
`git log --oneline $LAST..HEAD`; no em dashes; never a line about signing). Then:

```bash
git add -- Cargo.toml Cargo.lock web/shared/sw.js web/pages/settings-app.js web/pages/ops.html \
  web/shared/shell.js web/pages/download.html web/pages/index.html
git add -A -- app/web                              # scoped: only the bundle
git commit -F <msg-file>                           # never -m; ends with the Co-Authored-By line
git push origin main
node scripts/tag-release.js --release <notes-file> # reads the version from the committed Cargo.toml
gh release list --limit 3                          # the new tag is Latest, nothing stray
```

## 5. Deliver to the operator, record it

```bash
node scripts/archive-build.js      # v<version>_HumanityOS.exe, what `just launch` boots
node scripts/check-delivery.js     # must say the operator's exe is this release
```

Then a `recent_decisions` entry in `data/coordination/orchestrator_state.json`, PRIORITIES if
scope moved, and `docs/outreach/since-last-update.md`. Glance at `just ci` after the push: a red
deploy means the VPS is running stale code. Never tell the operator a release needs signing.
