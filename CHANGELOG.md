# Changelog

All notable changes to HumanityOS. Versions follow [semver](https://semver.org/):
`0.X.0` = Rust changes (server or desktop), `0.X.Y` = non-Rust changes (HTML/JS/CSS/docs).

---

## v0.24.0 — External Links & Download Fix (2026-03-20)

- **External links work in desktop app** — switched to `tauri-plugin-shell` for reliable URL opening on Windows
- **Download page button fixed** — checks GitHub API for latest release, opens direct installer download in system browser
- **Correct version display** — no more flickering between stale fallback versions
- In-app browser infrastructure preserved (Tauri multi-webview) but gated until API stabilizes

## v0.23.0 — In-App Browser (2026-03-20)

- **Native webview panels** — Tauri multi-webview API (`unstable` feature) for rendering external websites inside the app window
- Browser tab bar with close buttons
- Auto-resize on window resize
- Web fallback: external links open in new browser tab

## v0.22.0 — Direct Download & Version Fix (2026-03-20)

- **Download button downloads directly** — fetches installer URL from GitHub API instead of linking to releases page
- **Version display fixed** — fallback version matches current build (was hardcoded to `0.11.0`)
- **openExternal retry** — retries if `__TAURI__` not ready yet
- Bump script now updates JS fallback version in download.html

## v0.21.0 — Passphrase Modal & Download Nav (2026-03-20)

- **Passphrase input masked** — encrypted backup import shows proper password modal with Show/Hide toggle (was plain-text `prompt()`)
- **Download page nav fixed** — link changed from `/download` to `/activities/download` for Tauri compatibility

## v0.20.0 — DevTools Cleanup (2026-03-20)

- **DevTools gated behind env var** — no longer auto-opens on launch; set `HOS_DEVTOOLS=1` to enable
- F12 shortcut still works for on-demand access

## v0.19.2 — Desktop Import & SW Fix (2026-03-20)

- **Service worker skipped in Tauri** — no more `text/html` MIME type errors (Tauri serves missing files as SPA fallback)
- **Encrypted backup import works** — login screen handles both plain and encrypted backup files
- **Settings restore fixed** — was calling nonexistent `restoreFromEncryptedBackup()`, now uses `importIdentityBackup()`

## v0.19.1 — WebSocket Origin Fix (2026-03-20)

- **WebSocket 403 fixed** — added Tauri origins (`http://tauri.localhost`, `https://tauri.localhost`, `tauri://localhost`) to WS handshake allow-list in `ws_handler()`
- Desktop app can now connect to chat, load user list, server list, and channel history

## v0.19.0 — CORS Fix (2026-03-20)

- **CORS whitelist expanded** — added `http://tauri.localhost` to server's `CorsLayer` (was only `https://` variants)
- Tasks and roadmap now load in desktop app

## v0.18.3 — Auto-Open DevTools (2026-03-19)

- **DevTools auto-opens on launch** — temporary debug aid to diagnose desktop app issues
- Revealed actual Tauri origin (`http://tauri.localhost`) which led to CORS/WS fixes

## v0.18.2 — API Proxy Fallback (2026-03-19)

- **api_proxy catch fallback** — if Tauri IPC proxy fails, falls back to direct fetch
- Broadened Tauri capability permissions

## v0.18.1 — Blank Page Fix (2026-03-19)

- **Guard against Tauri IPC not ready** — `api_proxy` invoke wrapped in null check with fallback to direct fetch
- Fixed blank white screen on app launch

## v0.18.0 — Rust API Proxy (2026-03-19)

- **api_proxy Tauri command** — Rust-side HTTP proxy via reqwest that completely bypasses CORS
- Browser never makes cross-origin requests; all `/api/*` calls go through Rust

## v0.17.1 — CSP Revert (2026-03-19)

- **Reverted CSP in tauri.conf.json** — explicit CSP broke CSS/icon loading; removed in favor of api_proxy approach

## v0.17.0 — Tauri CORS Whitelist (2026-03-19)

- **Server CORS expanded** — added Tauri desktop origins to server's CORS allow-list

## v0.16.0 — Fetch Interceptor Fix (2026-03-19)

- **Desktop fetch interceptor** — fixed context binding and absolute URL handling for API calls

## v0.15.1 — Settings Backup Fix (2026-03-19)

- **myIdentity initialization** — settings page's backup dialog works now (was `myIdentity is not defined`)

## v0.15.0 — Desktop API/WS Proxy (2026-03-19)

- **INIT_SCRIPT** — Tauri initialization script overrides `fetch()` and `WebSocket` to route API calls to remote server
- Desktop app can now reach server endpoints

## v0.14.0 — Desktop Navigation Fix (2026-03-19)

- **bundle-web.js rewrite** — pages HTML goes to root level, JS to `pages/` subfolder (mirrors nginx layout)
- **rewriteForTauri()** — appends `.html` to extensionless paths for Tauri static file serving
- Desktop app navigation works: clicking nav tabs loads the correct pages

## v0.13.0 — Repo Restructure & Local-First Storage (2026-03-19)

### Repository restructure
- `desktop/src-tauri/` → `app/` (flatter, clearer)
- `ui/game/` → `ui/activities/` (reality/fantasy agnostic tools)
- `assets/ui/icons/` → `assets/icons/` (shared between UI and engine)
- Updated 14+ docs/scripts with stale path references

### Local-first save system (`app/src/storage.rs`)
- **622-line storage module** — OS-standard data dir (`%APPDATA%\HumanityOS\`)
- Save slots: profile, inventory, farm, quests, skills, world
- Auto-rotating backups (keep last 5)
- USB drive detection for portable saves
- Tiered sync config: local-only → own server → trusted server → public recovery
- 12 Tauri commands: list/create/delete/export/import saves, detect drives, sync config, backups

### Data management page (`ui/pages/data.html`)
- 5-tab UI: Overview, Saves, Backups, Sync Settings, USB/Portable
- Visual storage breakdown, backup management, sync tier toggles

## v0.12.0 — Engine & Search (2026-03-18)

- **wgpu renderer scaffold** — window creation, surface configuration, render loop
- **FTS5 full-text search** — SQLite FTS5 index for message search (upgrade from LIKE queries)
- **Database helper methods** — channel message counts, user activity stats
- **Voice system split** — `chat-voice.js` → `chat-voice.js` + `chat-voice-calls.js`
- **Gardening activity polish** — improved plant growth UI, harvest mechanics
- **Download page UX** — platform detection, version badges

## v0.11.0 — Local-First Desktop Architecture (2026-03-18)

- **Offline from first launch** — desktop app bundles all web UI locally
- **Background web sync** — checks `/api/web-manifest` for changed files, downloads granularly
- **bundle-web.js** — build script copies web files into `app/web/` with SHA-256 manifest
- **Asset manifest endpoint** — server exposes `/api/asset-manifest` for sync comparison
- Single codebase: `ui/` directory IS both the website AND the desktop app's local files

## v0.10.0 — External Links & Download UX (2026-03-18)

- **External links in Tauri** — intercept `target="_blank"` links, open in system browser
- **Download page improvements** — platform auto-detection, direct download links
- **Dual version display** — footer shows both web and desktop versions

## v0.9.0 — Tauri Link Handling (2026-03-18)

- **External link interception** — `open_external_url` Tauri command via `open` crate
- **Dual version footer** — shows web version vs desktop app version

## v0.8.1 — Desktop Update Flow (2026-03-18)

- **shell.js version bump** — added to automated version bump script
- **Desktop update flow** — improved update detection and notification

## v0.8.0 — Project Universe Engine Port (2026-03-17)

- **19 engine sub-crates** — core, modules, persistence, renderer, audio, input, physics, etc.
- **30 WGSL shaders** — planets, PBR, procedural materials, atmosphere, water, terrain
- **Game data pipeline** — CSV/TOML/RON for items, plants, recipes, quests, blueprints, ships
- **Engine architecture doc** — 17-section master reference (`docs/design/engine-architecture.md`)
- **Game systems** — farming, construction, inventory, combat, crafting, trading

## v0.7.0 — Maps, Gardening & Event Bus (2026-03-17)

- **Multi-scale maps** — world → region → local zoom levels with marker system
- **Gardening activity** — plant database, growth simulation, harvest mechanics
- **Event bus** (`shared/events.js`) — `hos.on/off/emit/gather` replaces monkey-patching
- **Dynamic asset API** — server-managed asset registry with upload/delete
- **Project Universe history docs** — 7 years of development context

## v0.6.1 — Dev Asset Browser (2026-03-17)

- **Asset browser rebuild** — 55 → 190 managed assets, search/filter/preview/upload/delete
- **Dev page** — icon testing, component gallery, theme preview

## v0.6.0 — Major Refactor (2026-03-16)

- **relay.rs split** — 5800 LOC → `relay.rs` (routing) + `handlers/msg_handlers.rs` (50 handler functions) + `handlers/broadcast.rs` + `handlers/federation.rs` + `handlers/utils.rs`
- **Unified settings** — merged 3 settings stores into one, extracted to `shared/settings.js`
- **Theme system** — replaced all hardcoded colors with CSS custom properties
- **Docs reorganization** — consolidated scattered docs into `docs/` tree

## v0.5.3 — JS Extraction & Roadmap (2026-03-16)

- **Inline JS extraction** — moved inline scripts from HTML pages into separate `.js` files
- **Roadmap page** — visual project roadmap with milestone tracking

## v0.5.2 — CSS Variable System (2026-03-16)

- **~1900 hardcoded spacing values replaced** with CSS variables (`--space-xs` through `--space-3xl`)
- **~350 hardcoded CSS values replaced** with theme variables across 11 JS files
- **Content width + line height controls** in settings
- **Compact mode** wired up

## v0.5.1 — Icon System Overhaul (2026-03-16)

- **Shared icon system** (`shared/icons.js`) — 35+ SVG icons with adjustable stroke weight
- **hosIcon() API** — `hosIcon('chat', 24)` returns inline SVG at any size
- **User-adjustable icon weight** — stored in localStorage, applied via CSS `--icon-weight`
- **Header nav converted** from PNG to inline SVG icons
- **Emoji replaced** with SVG icons across all pages and JS files
- **Dev workbench page** — icon thickness comparison, component gallery, color palette

---

*53 commits, 18 minor versions, 5 patch versions across 4 days of development.*
