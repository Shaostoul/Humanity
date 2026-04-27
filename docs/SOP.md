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
2. Search the codebase: `grep -r "feature_name" src/ web/`
3. Check the changelog: `grep -i "feature_name" CHANGELOG.md`

If it exists, enhance it. Don't rebuild it.

## Code Structure

Single Rust crate at the repo root since v0.90.0. No workspace, no sub-crates.
Feature flags `native`, `relay`, and `wasm` select what gets compiled in.

```
Humanity/
  src/        Single Rust crate. Subdirs: relay/ (backend), gui/ (egui native UI),
              renderer/ (wgpu), ecs/, systems/ (game), terrain/, ship/, physics/,
              audio/, assets/ (loader), net/, mods/. main.rs picks --headless or
              full desktop based on the runtime flag.
  web/        Website frontend (HTML, JS, CSS) served by nginx.
              chat/ (WS client), pages/ (standalone), shared/ (shell, theme).
  data/       Hot-reloadable game and config data (CSV, TOML, RON, JSON).
  assets/     Shared media (icons, shaders, models, textures, audio).
  schemas/    TOML schema definitions for data files.
  docs/       All documentation, including design specs and the Humanity Accord.
  scripts/    Build/deploy/version tooling.
  Cargo.toml  Single root manifest. No workspace.
```

Binary output is `target/release/HumanityOS.exe`. Run with `--headless` for VPS
relay-only mode; default mode loads the full desktop client.

**Never rename these directories without updating ALL references.** Past restructures
(engine/ → native/ in v0.37.0, native/ eliminated and folded into src/ in v0.88.0,
server/ + crates/ folded into src/relay/ in v0.90.0) each required updating 20+ files.

## Desktop Build & Deploy (CRITICAL)

### Binary name

The release binary is **`target/release/HumanityOS.exe`** (defined in `[[bin]]` in Cargo.toml). There is NO `humanity-engine.exe` output. If you see one, it's a stale artifact from a previous build and MUST be ignored.

### Build commands

```bash
# Full release build (~3.5 min from clean, ~20s incremental)
cargo build --release --features native

# Copy to project root for easy launch
cp target/release/HumanityOS.exe ./HumanityOS.exe
```

### Before copying the exe: ALWAYS kill the running process first

```bash
powershell -Command "Stop-Process -Name HumanityOS -Force -ErrorAction SilentlyContinue"
```

Do NOT use `taskkill /F /IM` from bash (the `/F` flag gets mangled). Always use PowerShell `Stop-Process`.

### Target directory (build cache)

`target/` is Cargo's build cache. It contains:
- **Compiled dependencies** (~14GB): Every crate in the dependency tree compiled to `.rlib`/`.d` files, BOTH debug and release profiles, plus build script outputs. This is the bulk.
- **Incremental compilation data** (~1-3GB): Intermediate artifacts Cargo keeps to speed up recompilation. Only the changed code recompiles instead of everything.
- **Final binary** (~18MB): `HumanityOS.exe` — single binary. Run with `--headless` for relay-only mode.
- **Build metadata**: `.fingerprint` dirs, dep-info files, examples, tests.

A clean build produces ~1.4GB. After many builds with both debug and release profiles, it balloons to 15GB+ because Cargo never garbage-collects old incremental artifacts.

**To reclaim space:** `cargo clean` deletes everything. Next build is a full rebuild (~3.5 min). Only do this when disk space matters or stale artifacts cause confusion.

**To clean just one profile:** `cargo clean --release` or `cargo clean --profile dev`.

### wgpu backend (DX12 vs Vulkan)

The desktop app uses **DX12-only on Windows** (`src/renderer/mod.rs`). Vulkan is disabled at the backend selection level because:
- Steam and Epic Games inject overlay DLLs into the Vulkan loader
- wgpu unconditionally compiles Vulkan support (hardcoded in wgpu's Cargo.toml)
- Even with `Backends::DX12`, wgpu loads `vulkan-1.dll` and enumerates Vulkan adapters
- The overlay DLLs corrupt this enumeration, causing a segfault before our code runs

The `Backends::DX12` flag tells wgpu to PREFER DX12 for the actual adapter, but does NOT prevent Vulkan DLL loading. If Steam/Epic overlays cause crashes, the only permanent fix would be disabling the `vulkan` cargo feature on `wgpu-core` (currently impossible due to cargo feature unification with wgpu's `wgc` feature).

**Linux/macOS** use Vulkan and Metal respectively (no overlay issue there).

### egui font limitations

egui's default font (Ubuntu-Light + Hack) only supports:
- ASCII and extended Latin characters
- Basic symbols: arrows (U+25B6, U+25BC), geometric shapes (U+25A0, U+25A1), math operators
- Does NOT support emoji (U+1Fxxx range): no lock, mic, speaker, gear emojis

Use ASCII text or basic Unicode for UI icons. Custom icon fonts can be added via `egui::Context::fonts_mut()` in the future.

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
3. **Server supports it**: If the action sends a WebSocket message, confirm the server's relay.rs has a matching handler for that exact message type. Search `src/relay/relay.rs` and `src/relay/handlers/` for the message type string. If the server uses slash commands (e.g., `/kick`, `/ban`, `/deletechannel`), send as a chat message with the slash command, not a custom message type.
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
