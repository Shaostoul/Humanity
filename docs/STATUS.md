# HumanityOS — Feature Status

> **Last updated:** 2026-04-01 | **Version:** v0.88.0
>
> This is the **single source of truth** for what is built, partial, or planned.
> Update this file every time features are added or status changes.

**Legend:** ✅ Built/working | ⚠️ Partial/needs work | ❌ Not yet built | 🔜 Next priority

---

## Communication

Everything in this section is **built and working**.

| Feature | Status | Details |
|---------|--------|---------|
| WebSocket relay | ✅ | relay.rs ~5800 LOC, message routing, Fibonacci rate limiting, Ed25519 auth |
| Channels | ✅ | Create, switch, ordering, read-only, invite codes, auto-lockdown |
| Direct messages | ✅ | E2E encrypted (ECDH P-256 + AES-256-GCM), @mentions, notifications |
| Threaded replies | ✅ | Thread view panel, reply indicators, reply count tracking |
| Message editing | ✅ | Server-side edit history, client UI |
| Pins | ✅ | Server-side + client UI, per-channel |
| Emoji reactions | ✅ | Persistent storage, Twemoji rendering |
| Markdown rendering | ✅ | Collapsible quotes, code blocks |
| Message search | ✅ | FTS5 full-text search with LIKE fallback |
| WebRTC voice calls | ✅ | 1-on-1 audio, group voice rooms, TURN server |
| Video calls | ✅ | Camera selection, PiP overlay, gallery view |
| Screen sharing | ✅ | Concurrent camera+screen layers, draggable PiP |
| Streaming system | ✅ | Streamer dashboard, WebRTC relay, scenes/presets |
| Voice join/leave sounds | ✅ | Audio cues when users enter/leave voice channels (v0.35.1) |
| Role badges in sidebar | ✅ | Visual role indicators next to usernames in member lists (v0.35.1) |

---

## Identity & Security

| Feature | Status | Details |
|---------|--------|---------|
| Ed25519 identity | ✅ | Key generation, sign/verify on all messages |
| Key rotation | ✅ | Dual-signed certificates (old key + new key) |
| BIP39 seed phrase | ✅ | 24-word backup & restore |
| Encrypted backup | ✅ | AES-256-GCM + PBKDF2-SHA256 (600k iterations) |
| Device management | ✅ | List, label, revoke devices; QR code linking |
| Vault sync | ✅ | Encrypted cross-device sync, auto-lock, timestamp freshness |
| Seed phrase recovery | ✅ | "Recover from Seed Phrase" button on login screen (v0.25.0) |
| Security hardening | ✅ | Error boundary, pagination guards, env validation, automated DB backups (v0.35.0) |

---

## Push Notifications

| Feature | Status | Details |
|---------|--------|---------|
| VAPID keys | ✅ | Server-side key pair configured |
| Service worker push handler | ✅ | Receives and displays push events |
| Subscription management | ✅ | Save, get, remove subscriptions |
| DM and @mention triggers | ✅ | Offline-only delivery to prevent duplicates |
| Stale subscription cleanup | ✅ | Auto-removes expired/invalid subscriptions |
| Notification preferences | ✅ | Per-user DM/mention/task/DND settings, server-side storage (v0.31.0) |
| Notification actions | ✅ | Reply and mark-read buttons on push notifications (v0.31.0) |

---

## Task Board

| Feature | Status | Details |
|---------|--------|---------|
| Kanban board | ✅ | Create, edit, move, delete tasks |
| Real-time updates | ✅ | WebSocket sync across clients |
| Task comments | ✅ | REST API + WebSocket + detail drawer UI |
| REST API endpoints | ✅ | GET/POST /api/tasks, PATCH/DELETE /api/tasks/{id}, comments |
| Fibonacci scope system | ✅ | Civilization-scale task scoping |
| Projects system | ✅ | Project CRUD, color/icon picker, task filtering by project (v0.25.0) |

---

## Marketplace

| Feature | Status | Details |
|---------|--------|---------|
| CRUD operations | ✅ | Create, read, update, delete listings |
| WebSocket real-time sync | ✅ | Live updates across clients |
| REST API | ✅ | GET/POST /api/listings, FTS5 search via ?q= parameter |
| Role-based access | ✅ | Verified+ users can create listings |
| Category filtering | ✅ | Search, sort, filter by category |
| Create/edit/delete modals | ✅ | Full UI for listing management |
| Image support | ✅ | Upload (drag-and-drop), carousel gallery, thumbnails (v0.25.0) |
| Full-text search | ✅ | FTS5 MATCH + LIKE fallback (v0.25.0) |
| Seller profiles | ✅ | Clickable seller names, profile modal with listings and ratings (v0.25.0) |
| Ratings and reviews | ✅ | Star ratings, review form, sort options, aggregate display (v0.25.0) |
| Buyer-seller messaging | ✅ | listing_messages table, WebSocket send/history (v0.31.0) |
| P2P trading with escrow | ✅ | Peer-to-peer trade system with escrow protection (v0.40.0) |

---

## Wallet & Funding

| Feature | Status | Details |
|---------|--------|---------|
| Solana wallet | ✅ | Balance, send, receive -- Ed25519 identity IS the Solana address (v0.25.0) |
| Token swaps | ✅ | Jupiter API integration, slippage settings, price impact warnings (v0.25.0) |
| Staking | ✅ | Validator picker, stake/unstake flows (v0.25.0) |
| NFT support | ✅ | Detection, Metaplex metadata, grid display with detail modals (v0.25.0) |
| Donation page | ✅ | Progress bar, dynamic multi-crypto address cards, FAQ (v0.25.0, enhanced v0.73.0) |
| Server funding config | ✅ | data/server-config.json with flexible addresses array supporting unlimited networks (v0.25.0, enhanced v0.73.0) |
| Wallet settings | ✅ | Network selection, custom RPC URL, nav balance toggle (v0.25.0) |
| Wallet on profile | ✅ | Solana address and balance shown on profile cards (v0.25.0) |
| Wallet guide | ✅ | 9-section beginner guide: wallet basics, send, receive, buy, sell, swap, backup, glossary (v0.73.0) |
| Admin donation addresses | ✅ | Add/edit/remove/reorder donation addresses in native settings, dynamic rendering in web+native (v0.73.0) |

---

## Game Engine

| Feature | Status | Details |
|---------|--------|---------|
| Rust/wgpu renderer | ✅ | PBR-lite pipeline, depth buffer, mesh/material system |
| Dual-target compilation | ✅ | Native (winit) + WASM (WebGPU) from same codebase (v0.25.0) |
| Three-mode camera | ✅ | First-person, third-person, orbit with smooth transitions (v0.26.0) |
| Platform abstraction | ✅ | platform.rs: logging, timing, asset loading across native/WASM |
| WGSL shaders | ✅ | 30 shaders (planets, PBR, procedural materials) |
| Game data files | ✅ | 23 crops, 111 items, 35 recipes, quest chains, blueprints |
| Gardening activity | ✅ | Playable 2D canvas farming (6 crops, save/load) |
| Data loading (AssetManager) | ✅ | load_csv/toml/ron/json, FileWatcher, HotReloadCoordinator (v0.28.0) |
| ECS system runner | ✅ | System trait, SystemRunner, 20 game components, per-frame tick (v0.29.0) |
| Icosphere planet terrain | ✅ | Icosahedron subdivision, PlanetDef (RON), LOD levels, PlanetRenderer (v0.30.0) |
| Voxel asteroid system | ✅ | Sparse octree, greedy meshing, ore veins (C/S/M-type), mining (v0.31.0) |
| Rapier3d physics | ✅ | Rigid bodies, colliders, raycasting, step simulation (v0.31.0) |
| Player controller | ✅ | WASD movement, gravity, jump, ground detection (v0.31.0) |
| Interaction system | ✅ | Raycast from camera, find interactables within range (v0.31.0) |
| Day/night cycle | ✅ | GameTime with seasons, sun direction/color (v0.31.0) |
| Inventory system | ✅ | ItemStack slots, add/remove/transfer (v0.31.0) |
| Crafting system | ✅ | Recipe matching from recipes.csv (v0.31.0) |
| Farming system | ✅ | Growth timer, stage transitions, water/health simulation (v0.31.0) |
| InputState | ✅ | Cross-system input sharing (v0.31.0) |
| Ship interior system | ✅ | ShipDef/DeckDef/RoomDef from RON, room mesh generation, BFS pathfinding (v0.33.0) |
| AI behavior system | ✅ | Passive/aggressive/herd/predator/guard state machines (v0.34.0) |
| Vehicle/mech system | ✅ | Enter/exit, Controllable transfer, torso twist, jump jets, heat (v0.34.0) |
| Ecology simulation | ✅ | Disease spread/recovery, population tracking, seasonal effects (v0.34.0) |
| Quest system | ✅ | Data-driven RON quests, step objectives, rewards (v0.34.0) |
| GLTF model loading | ✅ | Load .glb models via gltf crate, mesh caching in AssetManager (v0.34.0) |
| Instanced rendering | ✅ | InstanceBatch, pre-allocated uniform buffer, no per-frame GPU alloc (v0.34.0) |
| Global error boundary | ✅ | window.onerror + unhandledrejection, toast UI instead of white screen (v0.35.0) |
| Env var validation | ✅ | Fail-fast startup, clear messages for missing/invalid config (v0.35.0) |
| Automated DB backup | ✅ | SQLite backup every 6 hours, keep last 5, tokio background task (v0.35.0) |
| Weather system | ✅ | 7 weather conditions, seasonal variation, affects gameplay (v0.40.0) |
| Day/night sky renderer | ✅ | Procedural sky with stars, sun, moon, atmospheric scattering (v0.40.0) |
| Audio system | ✅ | kira crate, spatial 3D audio, music, SFX (v0.39.0) |
| Multiplayer networking | ✅ | WebSocket client, ECS state sync, server authority (v0.39.0) |
| Construction system | ✅ | Blueprints, snap grid, placement preview (v0.39.0) |
| Skills progression | ✅ | 20 skills, XP curves, level-up rewards (v0.39.0) |
| Mod support framework | ✅ | Mod manifest, load order, data override system (v0.40.0) |
| Heightmap terrain | ✅ | Procedural terrain generation with 16 biome types (v0.42.0) |
| Hydrological system | ✅ | Rain cycle, rivers, aquifers, contamination tracking (v0.42.0) |
| Atmospheric system | ✅ | Gas tracking, explosions, suffocation, pressure (v0.42.0) |
| Disaster system | ✅ | 21 disaster types, chain reactions, severity scaling (v0.42.0) |
| World persistence | ✅ | Save/load game world state, entities, terrain (v0.42.0) |
| Engine sub-crates | ⚠️ | 19 crates exist with structure, most implementations are scaffolds |

---

## Server & Infrastructure

| Feature | Status | Details |
|---------|--------|---------|
| Rust/axum/tokio server | ✅ | Production-ready relay |
| SQLite via rusqlite | ✅ | All data in relay.db |
| REST API | ✅ | 30+ endpoints (messages, tasks, projects, listings, reviews, members, etc.) |
| Federation Phase 1+2 | ✅ | Server registry, discovery, S2S WebSocket |
| Signed profile replication | ✅ | signed_profiles table, ProfileGossip between servers (v0.27.0) |
| Federated message persistence | ✅ | Messages persisted with origin_server tag, survive restarts (v0.27.0) |
| Profile lookup API | ✅ | GET /api/profile/{key} for public key lookup (v0.27.0) |
| GitHub webhook | ✅ | Deploy bot announces in chat |
| Admin system | ✅ | Roles, verify, lockdown, wipe, garbage collection |
| nginx + VPS pipeline | ✅ | Push to main triggers build + deploy |
| Server membership | ✅ | Auto-join on identify, member roster, paginated search (v0.25.0) |
| Server-info endpoint | ✅ | Description, owner_key, funding, member_count (v0.25.0) |
| Server game state authority | ✅ | Authoritative server for game state validation (v0.40.0) |
| Admin analytics dashboard | ✅ | Server metrics, user activity, system health monitoring (v0.40.0) |
| Guild system | ✅ | Create, join, search guilds with invite codes (v0.41.0) |
| Reputation system | ✅ | Points, levels, leaderboard for community standing (v0.41.0) |

---

## Navigation & UX

| Feature | Status | Details |
|---------|--------|---------|
| shell.js hub navigation | ✅ | Injected on every page, 20+ nav links |
| Standalone pages | ✅ | 20+ pages (tasks, maps, wallet, donate, settings, etc.) |
| Mobile navigation | ✅ | Touch drawer menus |
| Light/dark theme | ✅ | Toggle in shell, persisted |
| PWA support | ✅ | Manifest + service worker |
| Keyboard shortcuts | ✅ | Global shortcuts via shell.js |
| Onboarding tour | ✅ | 8-step guided walkthrough for new users (v0.25.0) |
| Real/Sim context toggle | ✅ | Global mode switch between real-life tools and simulation (v0.38.1) |
| Color-coded nav groups | ✅ | Red (identity), green (context-sensitive), blue (system) nav groups (v0.37.2) |
| Localization | ✅ | 5 language translations (v0.40.0) |
| Accessibility | ✅ | High contrast, colorblind modes, reduced motion support (v0.40.0) |

---

## Native Desktop Client

| Feature | Status | Details |
|---------|--------|---------|
| Standalone Rust binary | ✅ | egui + wgpu desktop app, no Tauri dependency |
| egui GUI system | ✅ | Immediate-mode UI with theme.ron, reusable widgets (v0.36.0) |
| Main menu page | ✅ | Entry point with navigation to all features (v0.36.0) |
| Settings page | ✅ | Theme, display, controls configuration (v0.36.0) |
| Inventory page | ✅ | Item management UI (v0.36.0) |
| Chat overlay page | ✅ | In-game chat interface (v0.36.0) |
| HUD page | ✅ | Health, status, interaction prompts (v0.36.0) |
| Hot-reloadable theme | ✅ | theme.ron for colors, spacing, fonts; live reload (v0.36.0) |

> **Note:** Tauri v2 desktop wrapper (`app/`) is deprecated. The native Rust binary replaces it. Source lives in `src/` and `crates/` at the repo root.

---

## Web Tools & Utilities

| Feature | Status | Details |
|---------|--------|---------|
| Civilization dashboard | ✅ | Macro community/infrastructure view with live API data (v0.39.0) |
| File browser/editor | ✅ | Browse, view, and edit files with built-in viewers (v0.39.0) |
| Tools catalog | ✅ | 37 open-source apps across 11 categories (v0.39.0) |
| Calculator | ✅ | Basic, scientific, and unit converter modes (v0.39.0) |
| Calendar/planner | ✅ | Event creation, scheduling, and reminders (v0.39.0) |
| Notes/journal | ✅ | Markdown preview, encrypted notes, daily log (v0.39.0) |
| Resources page | ✅ | 45 curated real-world resource links across categories (v0.39.0) |
| Glossary system | ✅ | 150+ terms with definitions, searchable overlay (v0.41.0) |

---

## Local-First Storage

| Feature | Status | Details |
|---------|--------|---------|
| OS-standard data dir | ✅ | `%APPDATA%\HumanityOS\` with identity, saves, settings, cache, backups |
| Save slots | ✅ | Profile, inventory, farm, quests, skills, world |
| Auto-rotating backups | ✅ | Keeps last 5 timestamped snapshots |
| USB drive detection | ✅ | Detects removable drives for export/import |
| Tiered sync config | ✅ | Configurable sync levels |
| Data management UI | ✅ | web/pages/data.html with saves, backups, sync settings, USB tabs |

---

## Game Data

| Feature | Status | Details |
|---------|--------|---------|
| Chemistry database | ✅ | 118 elements, 59 alloys, 132 compounds, 35 gases, 52 toxins (v0.42.0) |
| Solar system database | ✅ | 70+ celestial bodies with orbital and physical data (v0.42.0) |
| Materials database | ✅ | 92 materials with properties (v0.42.0) |
| Components database | ✅ | 102 components for crafting/construction (v0.42.0) |
| Items and recipes | ✅ | 306 items, 227 recipes (expanded v0.42.0) |
| Platform brand SVGs | ✅ | Steam, Epic, GOG, PlayStation, Xbox icons (v0.41.0) |

---

## What to Build Next (Priority Order)

| # | Feature | Category | Why |
|---|---------|----------|-----|
| 1 | 🔜 Map rework | Web | Replace 2D canvas solar system with 3D engine orbit mode |
| 2 | 🔜 Advanced trading | Marketplace | Order books, automated matching, trade history |
| 3 | 🔜 Biome-specific gameplay | Engine | Weather/terrain/ecology integration per biome |
| 4 | 🔜 Multiplayer world sync | Engine | Full ECS state replication for shared worlds |

---

## Summary

| Category | ✅ Built | ⚠️ Partial | ❌ Missing |
|----------|---------|-----------|-----------|
| Communication | 15 | 0 | 0 |
| Identity & Security | 8 | 0 | 0 |
| Push Notifications | 7 | 0 | 0 |
| Task Board | 6 | 0 | 0 |
| Marketplace | 12 | 0 | 0 |
| Wallet & Funding | 8 | 0 | 0 |
| Game Engine | 41 | 1 | 0 |
| Server & Infrastructure | 16 | 0 | 0 |
| Navigation & UX | 11 | 0 | 0 |
| Native Desktop Client | 8 | 0 | 0 |
| Web Tools & Utilities | 8 | 0 | 0 |
| Local-First Storage | 6 | 0 | 0 |
| Game Data | 6 | 0 | 0 |
| **Total** | **152** | **1** | **0** |
