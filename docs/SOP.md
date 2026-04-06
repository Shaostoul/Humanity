# Standard Operating Procedure (SOP)

Single source of truth for how HumanityOS development stays in sync across all environments.

## Environments

| Environment | Purpose | Path |
|-------------|---------|------|
| **GitHub** | Source of truth for code | github.com/Shaostoul/Humanity |
| **VPS** | Production server + web hosting | server1.shaostoul.com (united-humanity.us) |
| **Local PC** | Development machine | C:\Humanity |

## Before Every Session

1. **Read `docs/STATUS.md`** first. It lists every built feature, what's partial, and what's next. This prevents re-researching or rebuilding existing features.
2. **Read `docs/BUGS.md`** to know what's been fixed. Never re-fix a resolved bug.
3. **Sync check** (MANDATORY): Run the version sync check below. If local and GitHub differ, push + tag before doing anything else.

## Version Sync Check (MANDATORY, every session start + before ending)

Local version must always match GitHub. Check and fix drift:

```bash
# 1. Read local version from Cargo.toml
node -p "require('fs').readFileSync('Cargo.toml','utf8').match(/^version\s*=\s*\"(.+?)\"/m)[1]"

# 2. Read latest GitHub release tag
gh release list --repo Shaostoul/Humanity --limit 1

# 3. If they differ: push all changes, tag, and create release
git add -A
git diff --cached --quiet || git commit -m "v<VERSION>: <description>"
git push origin main
git tag v<VERSION>
git push origin v<VERSION>
gh release create v<VERSION> --repo Shaostoul/Humanity --title "v<VERSION>" --notes "<summary of changes>"
```

**Rules:**
- If local version > GitHub version: push + tag + release immediately
- If local version < GitHub version: something is wrong, investigate (local should never be behind)
- If equal: no action needed, proceed with session
- **At session end**: If any changes were made during the session, ensure they are pushed and GitHub matches local
- External SSD backup planned at v1.0.0

## Version Sync Protocol

All version strings MUST stay in sync. Use the bump script:

```bash
node scripts/bump-version.js patch   # non-Rust changes (HTML/JS/CSS/docs)
node scripts/bump-version.js minor   # Rust code changed (requires recompile)
```

This updates all 6 locations automatically:
- `Cargo.toml` (root package version field)
- `web/shared/sw.js` (CACHE_NAME bump)
- `web/pages/settings-app.js` (version tag)
- `web/pages/ops.html` (debug version)
- `web/shared/shell.js` (version string)
- `web/pages/download.html` (fallback version badge + subtitle)

## After Every Push

CI automatically deploys to VPS. If CI fails:
```bash
just sync    # force-sync VPS (fetch, reset, rebuild, rsync, restart)
```

For web-only changes (faster):
```bash
just sync-web   # rsync assets only, no rebuild
```

## GitHub Release Tags

Create a release tag when shipping notable versions:
```bash
git tag v0.XX.0
git push origin v0.XX.0
```
This triggers the desktop app build workflow (Windows/Mac/Linux installers).

**Do NOT skip tags.** The download page pulls from GitHub Releases API. Stale tags mean users get old builds.

## Feature Tracking (STATUS.md)

`docs/STATUS.md` is the feature inventory. Update it when:
- A new feature is built (add row, mark as built)
- A feature changes scope (update description)
- A feature is removed (remove row)

Format per feature:
```
| Feature Name | Status | Version | Notes |
```

Status values: Built, Partial, Planned, Removed

## Bug Tracking (BUGS.md)

`docs/BUGS.md` tracks all known bugs and their resolution status.

Format:
```
### BUG-XXX: Short description
- **Status**: Fixed / Open / Won't Fix
- **Version Found**: v0.XX.X
- **Version Fixed**: v0.XX.X
- **Description**: What's wrong
- **Fix**: What was done (if fixed)
```

**CRITICAL**: Always check BUGS.md before fixing a bug. If it's already marked Fixed, DO NOT re-fix it. Move on.

## Preventing Duplicate Features

Before building anything:
1. Read `docs/STATUS.md` feature list
2. Search the codebase: `grep -r "feature_name" src/ web/ server/src/`
3. Check the changelog: `grep -i "feature_name" CHANGELOG.md`

If it exists, enhance it. Don't rebuild it.

## Code Structure

```
Humanity/
  server/     Rust backend (axum, SQLite, WebSocket relay)
  src/        Desktop application (Rust, wgpu, egui, rapier3d)
  crates/     19 sub-crates (core, modules, persistence)
  web/        Website frontend (HTML, JS, CSS)
  data/       Shared game/config data (CSV, TOML, RON, JSON)
  assets/     Shared media (icons, shaders, models, textures)
  docs/       All documentation
  scripts/    Build and deploy tooling
```

**Never rename these directories without updating ALL references.** Past restructures (engine/ to native/ in v0.37.0, native/ eliminated in v0.88.0) required updating 26+ files.

## Web vs Native GUI

- **Web** (web/): HTML/JS/CSS served by the server, runs in browsers
- **Native** (src/gui/): egui immediate-mode UI, runs in the desktop app

Both show the same data but are separate codebases. When adding a feature:
- Build the web version in web/pages/
- The native egui version is secondary (built when the desktop app needs it)
- They connect to the same server API

## Button/Action Wiring Checklist

Every time a UI button or context menu action is added, verify ALL of the following before considering it complete:

1. **Click handler exists**: The `.clicked()` check (or pointer release check) is present and reachable
2. **Action is dispatched**: The handler actually does something (sends WS message, modifies state, etc.)
3. **Server supports it**: If the action sends a WebSocket message, confirm the server's relay.rs has a matching handler for that exact message type. Search `server/src/relay.rs` and `server/src/handlers/` for the message type string. If the server uses slash commands (e.g., `/kick`, `/ban`, `/deletechannel`), send as a chat message with the slash command, not a custom message type.
4. **Borrow checker safe**: In egui, ensure the click handler doesn't try to mutate state that's borrowed elsewhere in the same frame. Use deferred action patterns (collect actions in Phase 1, process in Phase 2) when rendering borrows state immutably.
5. **State updates propagate**: If the action modifies local state (e.g., removing a message), verify the UI will reflect the change on the next frame.
6. **Edge cases**: Test with the button's target in different states (e.g., Pin vs Unpin, own message vs others', connected vs disconnected).

Common failure patterns to avoid:
- Sending a custom WS message type that the server doesn't handle (always check server-side first)
- Using `allocate_ui_with_layout` for clickable rows (only returns `Sense::hover()`, use `allocate_exact_size` with `Sense::click()` instead)
- Adding `ui.interact()` that overlaps inner button rects (steals clicks from child widgets)
- Forgetting to close the menu after a context menu action (`ui.close_menu()`)

## Commit Standards

- Rust changes: bump minor version (0.X.0)
- Web/docs changes: bump patch version (0.X.Y)
- Include version in commit message: `v0.40.0: Feature description`
- Tag notable releases: `git tag v0.40.0 && git push origin v0.40.0`

## Deploy Pipeline

```
Push to main
  -> GitHub Actions CI
    -> SSH to VPS
      -> git pull
      -> cargo build (if Rust changed)
      -> rsync web files to /var/www/humanity/
      -> restart relay service
```

## Emergency Recovery

If the VPS is broken:
```bash
just sync     # nuclear option: git reset --hard, rebuild everything
just logs     # check what went wrong
just status   # verify git + CI + API health
```

If the database is corrupted:
- Auto-backups run every 6 hours to data/backups/
- Keep last 5 backups
- Restore: `cp data/backups/relay_LATEST.db data/relay.db && systemctl restart humanity-relay`
