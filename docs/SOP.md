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
3. **Check current version**: `gh release view --repo Shaostoul/Humanity --json tagName`
4. **Check local version**: look at `web/shared/shell.js` version string

## Version Sync Protocol

All version strings MUST stay in sync. Use the bump script:

```bash
node scripts/bump-version.js patch   # non-Rust changes (HTML/JS/CSS/docs)
node scripts/bump-version.js minor   # Rust code changed (requires recompile)
```

This updates all 7 locations automatically:
- `native/Cargo.toml` (version field)
- `web/shared/sw.js` (CACHE_NAME bump)
- `web/pages/settings-app.js` (version tag)
- `web/pages/ops.html` (debug version)
- `web/shared/shell.js` (version string)
- `app/Cargo.toml` (legacy, deprecated)
- `app/tauri.conf.json` (legacy, deprecated)

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
2. Search the codebase: `grep -r "feature_name" native/src/ web/ server/src/`
3. Check the changelog: `grep -i "feature_name" CHANGELOG.md`

If it exists, enhance it. Don't rebuild it.

## Code Structure

```
Humanity/
  server/     Rust backend (axum, SQLite, WebSocket relay)
  native/     Desktop application (Rust, wgpu, egui, rapier3d)
  web/        Website frontend (HTML, JS, CSS)
  data/       Shared game/config data (CSV, TOML, RON, JSON)
  assets/     Shared media (icons, shaders, models, textures)
  docs/       All documentation
  scripts/    Build and deploy tooling
```

**Never rename these directories without updating ALL references.** The v0.37.0 restructure (engine/ to native/, ui/ to web/) required updating 26+ files across 4 codebases.

## Web vs Native GUI

- **Web** (web/): HTML/JS/CSS served by the server, runs in browsers
- **Native** (native/src/gui/): egui immediate-mode UI, runs in the desktop app

Both show the same data but are separate codebases. When adding a feature:
- Build the web version in web/pages/
- The native egui version is secondary (built when the desktop app needs it)
- They connect to the same server API

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
