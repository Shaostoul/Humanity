# Development Loop

Standard cycle for continuous HumanityOS development. Follow these steps in order, then repeat.

## Step 1: Evaluate Current State
1. Read `docs/FEATURES.md` -- complete feature inventory with file paths
2. Read `docs/STATUS.md` -- what's built vs planned
3. Read `docs/BUGS.md` -- all bugs and their resolution status (NEVER re-fix a resolved bug)
4. Read `docs/SOP.md` -- version sync and deploy procedures
5. Check current version: look at `web/shared/shell.js` version string
6. Run `just status` to check git, CI, and live API health

## Step 2: Identify Work
Priority order:
1. Open bugs (from BUGS.md or user reports)
2. Broken features (things that exist but don't work)
3. Incomplete features (partially built)
4. New features (from roadmap or user requests)
5. Polish (UX improvements, performance, accessibility)
6. Data expansion (more items, recipes, planets, etc.)

Before proposing ANY feature: search FEATURES.md first. If it exists, enhance it. Never rebuild.

## Step 3: Develop
- Use subagents for parallel work on non-overlapping files
- All Rust code must pass: `cargo check` (native) AND `cargo check --target wasm32-unknown-unknown --features wasm --no-default-features` AND `cd server && cargo check`
- Follow existing code patterns (read before writing)
- No backward compatibility hacks until v1.0.0
- No em dashes in user-facing text

## Step 4: Sync
1. Bump version: `node scripts/bump-version.js patch|minor`
2. Commit with descriptive message
3. Push to GitHub (CI auto-deploys to VPS)
4. If CI fails: `just sync`
5. Create GitHub Release tag for notable versions

## Step 5: Update Docs
After EVERY development cycle, update:
- `docs/FEATURES.md` -- add new features with file paths
- `docs/STATUS.md` -- update counts and status
- `docs/BUGS.md` -- add new bugs found, mark fixed bugs
- `CHANGELOG.md` -- add version entry
- `CLAUDE.md` -- update if architecture changed
- Memory files -- update if patterns changed

## Step 6: Verify
- Check live site (united-humanity.us) for regressions
- Verify new pages load correctly
- Server compiles and runs
- No console errors on key pages

## Anti-Patterns (NEVER do these)
- Rebuild a feature that already exists (check FEATURES.md)
- Re-fix a bug marked as resolved (check BUGS.md)
- Create backward compatibility for pre-v1.0 code
- Use em dashes in text
- Skip version bumps
- Push without compiling
- Modify files without reading them first
