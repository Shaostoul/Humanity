# Changelog

All notable changes to HumanityOS. Versions follow [semver](https://semver.org/):
`0.X.0` = Rust changes (server or desktop), `0.X.Y` = non-Rust changes (HTML/JS/CSS/docs).

---

## v0.34.0 — AI, Vehicles, Ecology, Quests & GLTF Loading (2026-03-21)

### Engine — Game Systems & Rendering
- **AI behavior system** — behavior trees, autonomy (off-screen simulation), flow-field pathfinding
- **Vehicle/mech system** — ship piloting, propulsion, seat occupancy (pilot/gunner/passenger)
- **Ecology system** — ecosystem simulation for biomes and wildlife
- **Quest system** — objectives, procedural generation from RON templates
- **GLTF/GLB model loading** — AssetManager.load_gltf() with mesh caching, flat normal generation, planar UV fallback
- **Instanced rendering** — InstanceBatch groups same-mesh objects for GPU-efficient draw calls
- **Combat system** — damage types, status effects
- **Economy system** — fleet-wide resource pools
- **Skills system** — learning-by-doing progression
- **Logistics system** — cargo management, shipping routes
- **Navigation system** — galaxy, system, orbital, surface scale transitions

## v0.33.0 — Ship Interiors (2026-03-21)

### Engine — Ship Layout System
- **Ship interior module** — `native/src/ship/` with layout parsing and room mesh generation
- **Layout parser** — reads ship definitions from RON data files (`data/ships/`)
- **Room types** — bridge, reactor, quarters, cargo bay with dimensions and connections
- **Ship data files** — bridge.ron, layout_medium.ron, reactor.ron, starter_fleet.ron

## v0.32.0 — Systems Wired & Construction (2026-03-21)

### Engine — System Integration
- **15 game systems registered** — all systems implement System trait and tick in the engine loop
- **Construction system** — CSG booleans, blueprint hierarchy, structural analysis, auto-routing
- **Crafting workstations** — workstation types with tool requirements
- **Farming automation** — sprinkler/harvester automation, soil chemistry, crop disease
- **Inventory containers** — volumetric containers with cubic meter capacity
- **Combat effects** — damage types (kinetic/thermal/radiation/explosive), status effects

## v0.31.0 — Massive Parallel Build (2026-03-21)

Four agents built simultaneously:

### Engine — Voxel Asteroids & Physics
- **Voxel asteroid system** — sparse octree, greedy meshing, ore veins (C/S/M-type), mining
- **Full rapier3d physics** — rigid bodies, colliders, raycasting, step simulation
- **Player controller** — WASD movement, gravity, jump, ground detection
- **Interaction system** — raycast from camera, find interactables within range
- **Day/night cycle** — GameTime with seasons, sun direction/color
- **Inventory system** — ItemStack slots, add/remove/transfer
- **Crafting system** — recipe matching from recipes.csv
- **Farming system** — growth timer, stage transitions, water/health simulation
- **InputState** — cross-system input sharing
- **Asteroid types data file** — data/asteroids/types.csv

### Server — Notifications & Marketplace Messaging
- **Notification preferences** — per-user DM/mention/task/DND settings, server-side storage module
- **Buyer-seller messaging** — listing_messages table, WebSocket send/history
- **Push notification action buttons** — reply and mark-read on notifications

## v0.30.0 — Icosphere Planet Terrain with LOD (2026-03-21)

- **Icosphere** — base icosahedron (20 faces) with recursive midpoint subdivision
- **PlanetDef** — radius, gravity, terrain_seed, atmosphere, biomes in RON format
- **PlanetRenderer** — LOD from billboard (>100,000km) to walkable surface (<10km)
- **Mesh::from_icosphere** — builds GPU mesh from icosphere data
- **Planet data files** — Earth, Mars, Moon (data/planets/*.ron)
- Planet renders as slowly rotating icosphere in the test scene

## v0.29.0 — ECS System Runner & Game Components (2026-03-21)

- **System trait** — `tick(world, dt, data)` signature
- **SystemRunner** — registers and ticks systems each frame
- **20 game components** — Renderable, Controllable, AIBehavior, CropInstance, GrowthStage, Interactable, VehicleSeat, Hardpoints, HardpointSlot, Harvestable, Faction, VoxelBody, PhysicsBody, plus existing Transform, Velocity, Health, Name
- **ECS tick wired into native render loop**
- **hot_reload module** — available on both native and WASM platforms

## v0.28.0 — Data Loading Foundation (2026-03-21)

- **AssetManager** — load_csv<T>, load_toml<T>, load_ron<T> methods
- **CSV/TOML/RON/JSON parsers** — using crate deps (csv, toml, ron, serde_json)
- **FileWatcher** — notify-based recursive directory monitoring (native only)
- **HotReloadCoordinator** — polls watcher and invalidates cache per frame
- **Engine loads data at startup** — items.csv, plants.csv, recipes.csv with logged counts

## v0.27.0 — Signed Profile Replication & Federated Persistence (2026-03-21)

- **signed_profiles table** — new storage module for replicated profiles
- **Profile gossip** — ProfileGossip message type for inter-server profile replication
- **Federated message persistence** — messages persisted with origin_server tag (survive restarts)
- **GET /api/profile/{key}** — profile lookup by public key
- **Identity architecture doc** — docs/design/identity.md (no home servers, key IS identity)
- Updated federation doc: removed home server concept, added gossip protocol

## v0.26.0 — Three-Mode Camera System (2026-03-21)

- **Three-mode camera** — first-person, third-person, orbit with smooth transitions
- Camera mode switching with configurable offsets and zoom

## v0.25.0 — Wallet, Projects, Marketplace Upgrades (2026-03-20)

- **Solana wallet** — balance, send, receive; Ed25519 identity IS the Solana address
- **Token swaps** — Jupiter API integration, slippage settings, price impact warnings
- **Staking** — validator picker, stake/unstake flows
- **NFT support** — detection, Metaplex metadata, grid display with detail modals
- **Donation page** — progress bar, source cards (GitHub/Solana/Bitcoin), FAQ
- **Server funding config** — data/server-config.json with owner_key, funding sources
- **Wallet settings** — network selection, custom RPC URL, nav balance toggle
- **Projects system** — project CRUD, color/icon picker, task filtering by project
- **Marketplace images** — upload (drag-and-drop), carousel gallery, thumbnails
- **Full-text search** — FTS5 MATCH + LIKE fallback for marketplace listings
- **Seller profiles** — clickable seller names, profile modal with listings and ratings
- **Ratings and reviews** — star ratings, review form, sort options, aggregate display
- **Server membership** — auto-join on identify, member roster, paginated search
- **Server-info endpoint** — description, owner_key, funding, member_count
- **Seed phrase recovery** — "Recover from Seed Phrase" button on login screen
- **Onboarding tour** — 8-step guided walkthrough for new users

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
- `ui/game/` → `web/activities/` (reality/fantasy agnostic tools)
- `assets/ui/icons/` → `assets/icons/` (shared between UI and engine)
- Updated 14+ docs/scripts with stale path references

### Local-first save system (`app/src/storage.rs`)
- **622-line storage module** — OS-standard data dir (`%APPDATA%\HumanityOS\`)
- Save slots: profile, inventory, farm, quests, skills, world
- Auto-rotating backups (keep last 5)
- USB drive detection for portable saves
- Tiered sync config: local-only → own server → trusted server → public recovery
- 12 Tauri commands: list/create/delete/export/import saves, detect drives, sync config, backups

### Data management page (`web/pages/data.html`)
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

## v0.5.0 — Push Notifications & Auto-Updater (2026-03-16)

- **Push notifications** — service worker notification support
- **Silent auto-updater** — desktop app checks for updates without interrupting
- **Icon tooling** — build pipeline for generating icons at multiple sizes
- **Seed/backup buttons moved** from Network to Settings page
- **Identity backup guidance** — clearer prompts for key backup
- **SPA navigation bug fixed** — pages now do full reload instead of broken SPA routing
- **Web bookmarks** — save/manage bookmarks in Home tab
- **ELI5/Expert system** — dual-audience explanations on all pages, toggle in Settings
- **Nav consolidation** — 21 pages trimmed to 12, 9 orphaned pages deleted
- **Major repo cleanup** — unified history docs, consolidated icons, clean root directory

## v0.4.1 — Deploy Fix (2026-03-15)

- Fix deploy missing `game/` directory
- Sync all version strings across codebase

## v0.4.0 — Settings & Studio (2026-03-15)

- **Download page overhaul** — module management, platform detection
- **Complete settings page** — all preferences in one place
- **Feature-complete studio panel** — camera/screen controls, stream management
- **ELI5 purpose hints** on all 13 standalone pages
- **System context awareness** — detect, store, and share hardware specs
- **Context-tinted messages** — background color matches chat context (DM/channel/group)
- **Monochromatic pinned icons** + studio pill controls + AFK RGB button
- **Admin sidebar** — flat icon-based user list, deduplication
- **CI improvements** — Windows-first build gate, chat notification on build complete
- Fixed: message history loads most recent (not oldest), rate-limit false positives, group persistence

## v0.3.1 — First Auto-Update Release (2026-03-14)

- **First auto-update capable desktop release**
- Fixed debug overlay width + service worker version string

## v0.3.0 — Voice, Vault & Feature Web (2026-03-14)

This was the largest pre-tag release — ~870 commits covering the full platform build-out.

### Communication
- **WebRTC voice calling** — 1-on-1 audio, group voice rooms, TURN server
- **Video calls** — camera selection, PiP overlay, gallery view
- **Screen sharing** — concurrent camera+screen layers with draggable PiP
- **Streaming system** — streamer dashboard, WebRTC relay, scenes/presets, chat overlay
- **DMs** — @mentions, notifications, E2E encrypted via ECDH P-256 + AES-256-GCM
- **Threaded replies** — thread view panel with reply indicators
- **Follow/friend system** — social graph, group foundation, friend codes
- **Federation Phase 1+2** — server registry, discovery, server-to-server WebSocket

### Identity & Security
- **Ed25519 message signing** — full client-side signature verification
- **Key rotation** — dual-signed certificates for key migration
- **Vault sync** — encrypted cross-device vault with auto-lock, clipboard clear
- **BIP39 seed phrase** — 24-word backup & restore, paper backup option
- **Device management** — list, label, revoke linked keys, QR code linking
- **Security audits** — XSS fixes, API auth, CORS restrictions, WS size limits, HSTS
- **Rate limiting** — Fibonacci backoff, DM rate limiting, typing throttle

### Task Board & Feature Web
- **Kanban task board** — create/edit/move/delete tasks, real-time WebSocket updates
- **Task comments** — API endpoints + WebSocket handler + detail drawer UI
- **Feature Web** — interactive node graph for project planning with:
  - Canvas-based visualization with drag, zoom, search, filters
  - Node types/statuses/domains with teach mode
  - Board-to-graph sync (linked nodes mirror task status)
  - Import/export JSON, seed packs, constellation/orbit layouts
  - 100+ keyboard shortcuts with help panel
  - Full accessibility: ARIA labels, live regions, keyboard navigation

### Pages & Navigation
- **13 standalone pages** — tasks, maps, skills, inventory, calendar, home, quests, logbook, learn, settings, profile, data, download
- **Hub navigation** — shell.js with nav bar, theme toggle, keyboard shortcuts
- **Mobile nav** — touch drawer menus, fixed overlay dropdowns
- **Dashboard** — weather widget, active tasks, recent DMs
- **Skill DNA** — living skill trees with Reality/Fantasy XP, peer verification
- **Maps** — Earth surface, weather, GPS/icosphere coords, celestial navigation
- **Marketplace** — P2P listings, store directory, inventory bridge

### Server & API
- **SQLite persistence** — messages, channels, profiles, tasks, follows, vault, uploads
- **REST API** — 20+ endpoints for messages, tasks, search, assets, federation, vault
- **Channels** — read-only, invite codes, auto-lockdown, channel ordering
- **Admin system** — roles, verify, lockdown, wipe, GC, user registry
- **Bot API** — HTTP endpoints for AI integration, webhook notifications
- **GitHub webhook** — deploy bot announces pushes to chat

### Engine & Game
- **19 engine sub-crates** — core, modules, persistence, renderer, audio, input, physics
- **30 WGSL shaders** — planets, PBR, procedural materials, atmosphere, water, terrain
- **Game data pipeline** — CSV/TOML/RON for items, plants, recipes, quests, blueprints
- **Game systems** — farming, construction, inventory, combat, crafting, trading
- **3D terrain rendering** — capsule stand-in, third-person camera, scenic scenes
- **Offline gameplay** — non-combat milestone loop, desync recovery sync core

### Client Polish
- **PWA support** — manifest, service worker, installable on mobile
- **Emoji reactions** — persistent, cross-platform via Twemoji
- **Message editing + pins** — server-side + client UI
- **Identicons** — unique visual identity per user
- **Markdown rendering** — in messages with collapsible quotes
- **Encrypted user data sync** — AES-256-GCM client-side encryption
- **Periodic table** — 118 elements + 40 materials catalog
- **FTS5 search** — full-text message search with LIKE fallback
- **Light/dark theme toggle** — CSS custom properties throughout

## v0.2.0 — Version Display & Data Sync (2026-02-12)

- **Version in title bar** — desktop app shows current version
- **User data sync** — resolves by name so all devices share data

## v0.1.0 — First Desktop Release (2026-02-12)

- **Tauri v2 desktop app** — Windows installer with GitHub Actions CI
- **Auto-updater** — Tauri updater plugin for seamless updates
- **Ed25519 identity** — cryptographic identity system
- **WebSocket relay** — real-time message routing with SQLite persistence
- **Chat client** — channels, DMs, replies, reactions, markdown, search
- **Admin tools** — roles, lockdown, verify, wipe
- **Voice calling** — WebRTC P2P audio
- **Device linking** — QR code device pairing

## Pre-release — Foundation (2026-01-16 → 2026-02-12)

### Documentation & Governance
- **Humanity Accord** — v4.0 governing document covering rights, responsibilities, atrocities, censorship
- **Design docs** — encryption, security, abuse handling, social communication
- **Website** — Jekyll-based GitHub Pages with dark theme
- **Contributing guide** + onboarding documentation

### Server Infrastructure
- **humanity-core crate** — canonical CBOR, BLAKE3, Ed25519
- **humanity-relay** — WebSocket relay server with web client
- **SQLite persistence** — message storage, channels, profiles
- **Bot HTTP API** — for AI integration
- **Webhook notifications** — for relay chat messages
- **nginx + VPS deploy pipeline** — GitHub Actions → SSH → build → rsync → restart

### Chat Platform (pre-Tauri, web-only)
- **Login + identity** — name registration, auto-login, key persistence
- **Channels** — create, switch, ordering, read-only, invite codes
- **Messaging** — markdown, replies, quoting, editing, pins, typing indicators
- **Notifications** — chime sounds, notification selector, auto-reload on update
- **Image upload** — with per-user FIFO, admin size limits
- **Emoji reactions** — persistent via Twemoji
- **User blocking** — server-side block list
- **Identicons** — unique visual identity per public key

---

*Spanning from initial commit (2026-01-16) through v0.34.0 (2026-03-21).*
