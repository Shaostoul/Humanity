---
layout: default
title: Status
---

# Project Status

**Last updated:** March 2026 · v0.4.1

---

## Current Phase: Live Platform

The chat and hub are **operational and in daily use** at [united-humanity.us/chat](https://united-humanity.us/chat). We're past spec phase — this is a real platform people use.

---

## What's Built

### Communication
- ✅ Channels (public rooms)
- ✅ E2E encrypted DMs — ECDH P-256 + AES-256-GCM, server never sees plaintext
- ✅ Groups (private group conversations)
- ✅ Voice & video calling — WebRTC P2P with camera, screen share, PiP, camera selection
- ✅ Persistent voice channels — always-on WebRTC mesh rooms
- ✅ Message search — full-text search across conversations
- ✅ @mentions, persistent emoji reactions, message editing
- ✅ Image sharing and file uploads
- ✅ Browser notifications with 6 sound options
- ✅ Pin system — server pins + personal pins

### Social System
- ✅ Follow/friend system (mutual follow = friends)
- ✅ Friend codes — 8-character codes, 24-hour expiry, auto-mutual-follow
- ✅ User profiles with bio and social links
- ✅ Identicons (generated avatars)
- ✅ Device management — list, label, and revoke linked keys
- ✅ Blocking and reporting

### Hub Tools (11 tabs total)
- ✅ Project board (kanban task management)
- ✅ Marketplace — P2P listings, kiosks, 3D model subcategories, donation pricing
- ✅ Asset library — file upload (drag-drop), browse by category, grid/list views, tags, search, preview modal
- ✅ Universal catalog (elements, materials, processing chains)
- ✅ Browse tab — web directory with 52 sites, Tranco ranks, RDAP, uptime pings, collections
- ✅ Dashboard tab — 10 widget types, customizable drag-and-drop layout
- ✅ Personal inventory tracker
- ✅ Notes (private)
- ✅ Todos (personal task lists)
- ✅ Garden tracker v2 — 5 growing methods, 12 metrics, optimal ranges

### Game & Creative
- ✅ Fantasy tab — character sheet, lore, world map, achievements
- ✅ Streams tab — local capture demo
- ✅ Concept art for in-game spaces (spaceships, virtual malls)

### Platform & Security
- ✅ Cryptographic identity (Ed25519) — no accounts, no passwords
- ✅ Key backup/export/import
- ✅ Encrypted user data sync — AES-256-GCM derived from private key
- ✅ PWA — installable on mobile
- ✅ Desktop app — Tauri v2 with auto-updater (Windows, macOS ARM64 + x64, Linux)
- ✅ Settings panel — accent colors, font size, themes
- ✅ Command palette
- ✅ Auto-reload on deploy, auto-login
- ✅ Admin/mod tools, lockdown, invite codes
- ✅ Rate limiting, upload validation, CSP headers, TLS 1.2+, HSTS
- ✅ Server federation Phase 1 — discovery, trust tiers
- ✅ Federation Phase 2 — server-to-server WebSocket with Ed25519 handshake

### Documentation
- ✅ Humanity Accord (civilizational framework)
- ✅ Full technical design specs
- ✅ Architecture decision records

---

## In Progress

- 🔄 **Local AI integration** — Ollama setup for client-side AI hosting
- 🔄 **Native game client** — Rust-based first-person client development
- 🔄 **Platform optimization** — performance improvements and token efficiency

---

## What's Planned

- ⏳ Native game client (Rust)
- ⏳ P2P game distribution via GitHub Releases
- ⏳ In-game commerce (virtual mall with real retailer kiosks)
- ⏳ Payment processing (Stripe Connect or crypto)
- ⏳ Content-addressed file sharing
- ⏳ Local AI integration (Ollama)

---

## Recent Milestones

- **Mar 2026** — v0.4.1: Download page with module management, settings overhaul, studio panel, system context awareness, desktop auto-updater fix
- **Feb 2026** — Persistent voice channels, garden tracker, fantasy tab, streams tab
- **Jan 2026** — Marketplace, universal catalog, project board, inventory system
- **Late 2025** — Voice chat (WebRTC P2P), follow/friend system, groups, user profiles
- **Mid 2025** — Core chat platform launch — channels, DMs, reactions, moderation
- **Early 2025** — Server federation Phase 1, PWA support, key backup/import

---

## How to Help

This is an open project. Contributions welcome at every level:

- **Developers** — Rust, JavaScript, Node.js, WebRTC
- **Writers** — improve docs and clarity
- **Designers** — UI/UX, concept art, 3D models
- **Testers** — use the platform, report bugs
- **Translators** — make this accessible worldwide

→ [Get Involved](/Humanity/get-involved)

---

*The future is constructed by those who show up.*
