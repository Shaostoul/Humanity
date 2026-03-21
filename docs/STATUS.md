# HumanityOS — Feature Status

> **Last updated:** 2026-03-21 | **Version:** v0.26.0
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

---

## Push Notifications

| Feature | Status | Details |
|---------|--------|---------|
| VAPID keys | ✅ | Server-side key pair configured |
| Service worker push handler | ✅ | Receives and displays push events |
| Subscription management | ✅ | Save, get, remove subscriptions |
| DM and @mention triggers | ✅ | Offline-only delivery to prevent duplicates |
| Stale subscription cleanup | ✅ | Auto-removes expired/invalid subscriptions |
| Notification preferences UI | ⚠️ | Settings page has toggles (master, DM, mentions, tasks, DND) — not yet wired to server |
| Notification actions | ❌ | No reply or mark-read buttons on notifications |

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
| Buyer-seller messaging | ❌ | No in-listing conversation thread |

---

## Wallet & Funding

| Feature | Status | Details |
|---------|--------|---------|
| Solana wallet | ✅ | Balance, send, receive — Ed25519 identity IS the Solana address (v0.25.0) |
| Token swaps | ✅ | Jupiter API integration, slippage settings, price impact warnings (v0.25.0) |
| Staking | ✅ | Validator picker, stake/unstake flows (v0.25.0) |
| NFT support | ✅ | Detection, Metaplex metadata, grid display with detail modals (v0.25.0) |
| Donation page | ✅ | Progress bar, source cards (GitHub/Solana/Bitcoin), FAQ (v0.25.0) |
| Server funding config | ✅ | data/server-config.json with owner_key, funding sources (v0.25.0) |
| Wallet settings | ✅ | Network selection, custom RPC URL, nav balance toggle (v0.25.0) |
| Wallet on profile | ✅ | Solana address and balance shown on profile cards (v0.25.0) |

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
| Engine sub-crates | ⚠️ | 19 crates exist with structure, most implementations are scaffolds |
| Terrain rendering | ❌ | No terrain/tile rendering yet |
| Game object rendering | ❌ | No trees, crops, buildings from data files |
| Player controller | ❌ | No movement + interaction system (engine-level) |

---

## Server & Infrastructure

| Feature | Status | Details |
|---------|--------|---------|
| Rust/axum/tokio server | ✅ | Production-ready relay |
| SQLite via rusqlite | ✅ | All data in relay.db |
| REST API | ✅ | 30+ endpoints (messages, tasks, projects, listings, reviews, members, etc.) |
| Federation Phase 1+2 | ✅ | Server registry, discovery, S2S WebSocket |
| GitHub webhook | ✅ | Deploy bot announces in chat |
| Admin system | ✅ | Roles, verify, lockdown, wipe, garbage collection |
| nginx + VPS pipeline | ✅ | Push to main triggers build + deploy |
| Server membership | ✅ | Auto-join on identify, member roster, paginated search (v0.25.0) |
| Server-info endpoint | ✅ | Description, owner_key, funding, member_count (v0.25.0) |

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

---

## Desktop App (Tauri v2)

| Feature | Status | Details |
|---------|--------|---------|
| Tauri v2 shell | ✅ | Desktop binary wrapping web UI |
| Local-first bundling | ✅ | All web UI bundled, works offline |
| Background web sync | ✅ | Checks /api/web-manifest for updates |
| External links | ✅ | Via tauri-plugin-shell |
| DevTools gating | ✅ | Gated behind environment variable |
| Service worker skip | ✅ | Skipped in Tauri WebView2 context |
| Auto-updater | ⚠️ | Signing keys + CI pipeline ready, end-to-end test pending |
| Multi-webview | ⚠️ | Infrastructure exists but gated due to rendering bugs |

---

## Local-First Storage

| Feature | Status | Details |
|---------|--------|---------|
| Storage module | ✅ | 622-line app/src/storage.rs |
| Save slots | ✅ | Profile, inventory, farm, quests, skills, world |
| Auto-rotating backups | ✅ | Keeps last 5 timestamped snapshots |
| USB drive detection | ✅ | Detects removable drives for export/import |
| Tiered sync config | ✅ | Configurable sync levels |
| Tauri commands | ✅ | 12 commands (list/create/delete/export/import saves, backups, sync config) |

---

## What to Build Next (Priority Order)

| # | Feature | Category | Why |
|---|---------|----------|-----|
| 1 | 🔜 Engine Phase 3: Terrain | Engine | Load tile data, render ground + water in 3D |
| 2 | 🔜 Engine Phase 4: Game objects | Engine | Trees, crops, buildings from data files |
| 3 | 🔜 Engine Phase 5: Player controller | Engine | Movement, interaction, collision |
| 4 | 🔜 Engine Phase 6: Game systems | Engine | Farming, inventory, day/night cycle in 3D |
| 5 | 🔜 Push notification wiring | Notifications | Connect settings toggles to server-side filtering |
| 6 | 🔜 Buyer-seller messaging | Marketplace | In-listing conversation thread |

---

## Summary

| Category | ✅ Built | ⚠️ Partial | ❌ Missing |
|----------|---------|-----------|-----------|
| Communication | 13 | 0 | 0 |
| Identity & Security | 7 | 0 | 0 |
| Push Notifications | 5 | 1 | 1 |
| Task Board | 6 | 0 | 0 |
| Marketplace | 10 | 0 | 1 |
| Wallet & Funding | 8 | 0 | 0 |
| Game Engine | 7 | 1 | 3 |
| Server & Infrastructure | 9 | 0 | 0 |
| Navigation & UX | 7 | 0 | 0 |
| Desktop App | 6 | 2 | 0 |
| Local-First Storage | 6 | 0 | 0 |
| **Total** | **84** | **4** | **5** |
