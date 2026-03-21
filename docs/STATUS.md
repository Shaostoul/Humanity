# HumanityOS — Feature Status

> **Last updated:** 2026-03-20 | **Version:** v0.24.0
>
> This is the **single source of truth** for what is built, partial, or planned.
> Update this file every time features are added or status changes.

**Legend:** ✅ Built/working | ⚠️ Partial/needs work | ❌ Not yet built | 🔜 Next priority

---

## Architecture Overview

```
Client (browser / Tauri WebView2)
  ↕ WebSocket /ws         ← relay.rs  (~5800 LOC, message routing)
  ↕ HTTP /api/*           ← api.rs    (~1500 LOC, REST endpoints)

Server: Rust / axum / tokio, SQLite via rusqlite
Static HTML served by nginx from /var/www/humanity/
Desktop: Tauri v2 shell — local-first, background sync
```

---

## File Map

| Path | Role |
|------|------|
| `server/src/relay.rs` | WS message routing, rate limiting, auth |
| `server/src/api.rs` | REST API handlers |
| `server/src/main.rs` | Router setup, CSP middleware, axum config |
| `server/src/storage/` | 14 domain modules (messages, channels, tasks...) |
| `server/src/handlers/` | broadcast.rs, federation.rs, msg_handlers.rs, utils.rs |
| `engine/src/` | Game engine: renderer, ECS, physics, audio, input, hot-reload |
| `engine/src/systems/` | Game systems: farming, construction, inventory, combat, etc. |
| `engine/crates/` | 19 sub-crates (core, modules, persistence, etc.) |
| `app/src/main.rs` | Tauri shell — local-first + background sync |
| `app/src/storage.rs` | Local-first data persistence |
| `ui/chat/app.js` | Core chat logic (~1700 LOC) |
| `ui/chat/chat-*.js` | messages, dms, social, ui, voice, profile, p2p |
| `ui/chat/crypto.js` | Ed25519/ECDH/AES + BIP39 + backup helpers |
| `ui/shared/events.js` | Lightweight event bus (`hos.on/off/emit/gather`) |
| `ui/shared/shell.js` | Nav injection, theme toggle, update checker |
| `ui/shared/settings.js` | Settings panel + gear button |
| `ui/pages/*.html` | Standalone feature pages — tasks, maps, settings, etc. |
| `ui/activities/` | Game/real-world activities — gardening, download, etc. |
| `assets/` | All shared media — icons, shaders, models, textures, audio |
| `data/` | Hot-reloadable game data — CSV, TOML, RON, JSON |
| `docs/` | All documentation — design, accord, history, website |

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
| Recovery UX | ⚠️ | **Missing:** No "Recover from Seed Phrase" on login screen, no seed phrase shown at signup, no recovery wizard |

---

## Push Notifications

| Feature | Status | Details |
|---------|--------|---------|
| VAPID keys | ✅ | Server-side key pair configured |
| Service worker push handler | ✅ | Receives and displays push events |
| Subscription management | ✅ | Save, get, remove subscriptions |
| DM and @mention triggers | ✅ | Offline-only delivery to prevent duplicates |
| Stale subscription cleanup | ✅ | Auto-removes expired/invalid subscriptions |
| Notification preferences UI | ❌ | Users cannot choose which events trigger push |
| Do-not-disturb schedule | ❌ | No quiet hours or DND mode |
| Notification actions | ❌ | No reply or mark-read buttons on notifications |

---

## Auto-Updater (Desktop)

| Feature | Status | Details |
|---------|--------|---------|
| Signing keys generated | ✅ | Public key in tauri.conf.json |
| CI signs binaries | ✅ | GitHub Actions pipeline |
| Signatures in latest.json | ✅ | Embedded in GitHub Releases |
| Desktop signature verification | ✅ | App verifies before installing |
| GitHub Secrets verification | ⚠️ | Need to verify TAURI_SIGNING_PRIVATE_KEY + password exist |
| End-to-end release test | ⚠️ | Full cycle not yet tested in production |

---

## Task Board

| Feature | Status | Details |
|---------|--------|---------|
| Kanban board | ✅ | Create, edit, move, delete tasks |
| Real-time updates | ✅ | WebSocket sync across clients |
| Task comments | ✅ | REST API + WebSocket + detail drawer UI |
| REST API endpoints | ✅ | GET/POST /api/tasks, PATCH/DELETE /api/tasks/{id}, comments |
| Fibonacci scope system | ✅ | Civilization-scale task scoping |
| Projects system | ❌ | No project grouping, no multi-project filtering, no visibility controls |

---

## Marketplace

| Feature | Status | Details |
|---------|--------|---------|
| CRUD operations | ✅ | Create, read, update, delete listings |
| WebSocket real-time sync | ✅ | Live updates across clients |
| REST API | ✅ | GET/POST /api/listings |
| Role-based access | ✅ | Verified+ users can create listings |
| Category filtering | ✅ | Search, sort, filter by category |
| Create/edit/delete modals | ✅ | Full UI for listing management |
| Image support | ❌ | Listings cannot have images |
| Full-text search | ❌ | No FTS on listings |
| Seller profiles | ❌ | No dedicated seller pages |
| Buyer-seller messaging | ❌ | No in-listing conversation thread |
| Ratings and reviews | ❌ | No feedback system |

---

## Funding & Donations

| Feature | Status | Details |
|---------|--------|---------|
| GitHub Sponsors link | ✅ | Link in README |
| Crypto exchange architecture | ✅ | Documented in docs/economy/crypto_exchange.md |
| Donation page | ❌ | No dedicated donation UI |
| Live funding progress bar | ❌ | No real-time funding tracker |
| Crypto wallet on profile | ❌ | Users cannot display wallet addresses |
| Multi-source aggregation | ❌ | No GitHub Sponsors + Solana + BTC aggregation |

---

## Game / Activities

| Feature | Status | Details |
|---------|--------|---------|
| Gardening activity | ✅ | Fully playable: isometric canvas farming, 6 crops, save/load |
| Game data files | ✅ | 23 crops (plants.csv), 111 items (items.csv), 35 recipes (recipes.csv) |
| Quest data | ✅ | Tutorial chain in quests/tutorial.ron |
| WGSL shaders | ✅ | 30 shaders (planets, PBR, procedural materials) |
| Engine sub-crates | ⚠️ | 19 crates exist with structure, most implementations are scaffolds |
| Rust engine systems | ⚠️ | Architecture exists; farming, construction, inventory, combat are stubs |
| Other playable activities | ❌ | Only gardening is playable so far |

---

## Navigation & Pages

| Feature | Status | Details |
|---------|--------|---------|
| shell.js hub navigation | ✅ | Injected on every page |
| Standalone pages | ✅ | 13 pages (tasks, maps, settings, etc.) |
| Mobile navigation | ✅ | Touch drawer menus |
| Light/dark theme | ✅ | Toggle in shell, persisted |
| PWA support | ✅ | Manifest + service worker |
| Keyboard shortcuts | ✅ | Global shortcuts via shell.js |

---

## Server & API

| Feature | Status | Details |
|---------|--------|---------|
| Rust/axum/tokio server | ✅ | Production-ready relay |
| SQLite via rusqlite | ✅ | All data in relay.db |
| REST API | ✅ | 20+ endpoints (see CLAUDE.md for full list) |
| Federation Phase 1+2 | ✅ | Server registry, discovery, S2S WebSocket |
| GitHub webhook | ✅ | Deploy bot announces in chat |
| Admin system | ✅ | Roles, verify, lockdown, wipe, garbage collection |
| nginx + VPS pipeline | ✅ | Push to main triggers build + deploy |
| Server membership model | ❌ | Users cannot "join" a server as home |
| Server-info funding config | ❌ | No funding metadata in server-info |

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

## Documentation

| Feature | Status | Details |
|---------|--------|---------|
| CLAUDE.md | ✅ | Full project context for AI agents |
| CHANGELOG.md | ✅ | Complete history, v0.1.0 through v0.24.0 |
| Engine architecture | ✅ | docs/design/engine-architecture.md |
| In-game browser architecture | ✅ | Tauri webview panel design doc |
| Crypto exchange architecture | ✅ | docs/economy/crypto_exchange.md |
| ONBOARDING.md | ✅ | New contributor guide |
| STATUS.md | ✅ | This file — single source of truth |

---

## What to Build Next (Priority Order)

| # | Feature | Category | Why |
|---|---------|----------|-----|
| 1 | 🔜 Projects system | Tasks | Group tasks into projects, multi-project filtering, public/private visibility |
| 2 | 🔜 Funding/donation page + tracker | Economy | Live progress bar, multi-source aggregation, crypto wallet display |
| 3 | 🔜 Identity recovery UX | Security | Seed phrase shown at signup, "Recover" button on login, guided wizard |
| 4 | 🔜 Server membership model | Server | Users can "join" a server as home, member roster, membership sync |
| 5 | 🔜 More playable activities | Game | Expand beyond gardening — construction, cooking, crafting |
| 6 | 🔜 Push notification preferences | Notifications | Per-event toggles, DND schedule, notification action buttons |

---

## Summary

| Category | ✅ Built | ⚠️ Partial | ❌ Missing |
|----------|---------|-----------|-----------|
| Communication | 13 | 0 | 0 |
| Identity & Security | 6 | 1 | 0 |
| Push Notifications | 5 | 0 | 3 |
| Auto-Updater | 4 | 2 | 0 |
| Task Board | 5 | 0 | 1 |
| Marketplace | 6 | 0 | 5 |
| Funding & Donations | 2 | 0 | 4 |
| Game / Activities | 4 | 2 | 1 |
| Navigation & Pages | 6 | 0 | 0 |
| Server & API | 7 | 0 | 2 |
| Desktop App | 6 | 1 | 0 |
| Local-First Storage | 6 | 0 | 0 |
| Documentation | 7 | 0 | 0 |
| **Total** | **77** | **6** | **16** |
