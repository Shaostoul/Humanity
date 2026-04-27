# Sync Checklist

Everything that must stay in sync when making changes. Run through this before every push.

## Version Strings (automated)

Run `node scripts/bump-version.js patch|minor|major` to update all 6 active locations:
- [ ] `Cargo.toml` version (root)
- [ ] `web/shared/sw.js` CACHE_NAME
- [ ] `web/pages/settings-app.js` version tag
- [ ] `web/pages/ops.html` debug version
- [ ] `web/shared/shell.js` version string
- [ ] `web/pages/download.html` version badge

(The script also touches `web/activities/download.html` if it still exists — a
fallback for the old layout. Don't add new locations without also updating the
script.)

## Documentation (manual)

After adding features:
- [ ] `docs/FEATURES.md` -- add feature with file paths
- [ ] `docs/STATUS.md` -- update feature count and status
- [ ] `docs/BUGS.md` -- add any bugs found/fixed
- [ ] `CHANGELOG.md` -- add version entry
- [ ] `CLAUDE.md` -- update if architecture or key patterns changed

After fixing bugs:
- [ ] `docs/BUGS.md` -- mark as Fixed with version number
- [ ] `CHANGELOG.md` -- mention fix in version entry

## Code Sync Points

When adding a new web page:
- [ ] Create `web/pages/PAGE.html` + `web/pages/PAGE-app.js`
- [ ] Use absolute script path: `src="/pages/PAGE-app.js"`
- [ ] Load shell.js with `data-active=PAGE`
- [ ] Add URL detection in `web/shared/shell.js` if adding to nav
- [ ] Add to nav group (red/green/blue) in shell.js if user-facing
- [ ] Add to mobile drawer in shell.js
- [ ] Add to `docs/FEATURES.md`

When adding a new API endpoint:
- [ ] Add handler in `src/relay/api.rs` (or a new `src/relay/api_v2_*.rs` for `/api/v2/` endpoints)
- [ ] Wire route in `src/relay/mod.rs` (`router()`)
- [ ] Add storage module in `src/relay/storage/` if needed
- [ ] Add `mod NAME;` to `src/relay/storage/mod.rs`
- [ ] Run `cargo check --features relay --no-default-features`
- [ ] Document in `CLAUDE.md` REST routes section

When adding a new game system:
- [ ] Create in `src/systems/NAME/mod.rs`
- [ ] Add `pub mod NAME;` to `src/systems/mod.rs`
- [ ] Register in engine loop (`src/lib.rs` resumed())
- [ ] Add data files to `data/` if needed
- [ ] Run `cargo check` for both native and WASM targets
- [ ] Add to `docs/FEATURES.md`

When adding a new egui page:
- [ ] Create in `src/gui/pages/NAME.rs`
- [ ] Add to `src/gui/pages/mod.rs`
- [ ] Add `GuiPage::NAME` variant to `src/gui/mod.rs`
- [ ] Add draw call in engine loop match statement

When modifying shared data structures:
- [ ] Check both web (JS) and native (Rust) use the same field names
- [ ] Check API responses match what both clients expect

## Environments

After pushing to main:
- [ ] CI auto-deploys web files to VPS
- [ ] CI auto-rebuilds server binary on VPS (if Rust changed)
- [ ] Verify with `just status` or check CI run

After creating a release tag:
- [ ] CI builds desktop binaries (Windows/Mac/Linux)
- [ ] Download page auto-detects new version
- [ ] Desktop auto-updater detects new version

## Files That Reference Paths

If renaming directories, update ALL of these:
- [ ] `src/relay/api.rs` (asset/web manifest paths)
- [ ] `Justfile` (sync recipes)
- [ ] `scripts/bundle-web.js`
- [ ] `scripts/generate-asset-manifest.js`
- [ ] `.github/workflows/deploy.yml`
- [ ] `.github/workflows/deploy-pages.yml`
- [ ] `.github/workflows/build-desktop.yml`
- [ ] `CLAUDE.md` (file map, REST routes)
- [ ] `docs/FEATURES.md` (all file paths)
- [ ] `CONTRIBUTING.md` (project structure)
- [ ] `docs/SOP.md` (code structure)
