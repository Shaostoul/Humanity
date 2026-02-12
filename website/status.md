---
layout: default
title: Status
---

# Project Status

**Last updated:** February 2026

---

## Current Phase: Live Platform

The chat and hub are **operational and in daily use** at [united-humanity.us/chat](https://united-humanity.us/chat). We're past spec phase — this is a real platform people use.

---

## What's Built

### Communication
- ✅ Channels (public rooms)
- ✅ Direct messages (friend requirement for privacy)
- ✅ Groups (private group conversations)
- ✅ Voice chat — WebRTC P2P 1-on-1 calls
- ✅ Persistent voice channels — always-on rooms
- ✅ @mentions, persistent emoji reactions, message editing
- ✅ Image sharing and file uploads
- ✅ Browser notifications with 6 sound options
- ✅ Pin system — server pins + personal pins

### Social System
- ✅ Follow/friend system (mutual follow = friends)
- ✅ User profiles with bio and social links
- ✅ Identicons (generated avatars)
- ✅ Blocking and reporting

### Hub Tools
- ✅ Project board (kanban task management)
- ✅ Marketplace (P2P listings)
- ✅ Universal catalog (elements, materials, processing chains)
- ✅ Personal inventory tracker
- ✅ Notes (private)
- ✅ Todos (personal task lists)
- ✅ Garden tracker

### Game & Creative
- ✅ Fantasy tab — character sheet, lore, world map, achievements
- ✅ Streams tab — local capture demo
- ✅ Concept art for in-game spaces (spaceships, virtual malls)

### Platform & Security
- ✅ Cryptographic identity (Ed25519) — no accounts, no passwords
- ✅ Key backup/export/import
- ✅ Auto-sync user data to server
- ✅ PWA — installable on mobile
- ✅ Settings panel — accent colors, font size, themes
- ✅ Command palette
- ✅ Admin/mod tools, lockdown, invite codes
- ✅ Rate limiting, upload validation, CSP headers, TLS 1.2+, HSTS
- ✅ Server federation Phase 1 — discovery, trust tiers

### Documentation
- ✅ Humanity Accord (civilizational framework)
- ✅ Full technical design specs
- ✅ Architecture decision records

---

## In Progress

- 🔄 **Reconnect loop fix** — intermittent connection cycling on some clients. Top priority.
- 🔄 **Voice/Video calling** — voice works, video support being added
- 🔄 **Federation Phase 2** — server-to-server messaging

---

## What's Planned

- ⏳ E2E encrypted DMs (X25519 + XChaCha20-Poly1305)
- ⏳ Desktop app (Tauri — Windows/Mac/Linux)
- ⏳ Video calls
- ⏳ Actual WebRTC streaming (peer-assisted mesh)
- ⏳ Client file split (separate HTML/CSS/JS for CSP hardening)
- ⏳ Encrypted user data sync
- ⏳ Asset library system
- ⏳ 3D model marketplace
- ⏳ Native game client (Rust)
- ⏳ P2P game distribution via GitHub Releases
- ⏳ In-game commerce (virtual mall with real retailer kiosks)
- ⏳ Payment processing (Stripe Connect or crypto)

---

## Recent Milestones

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
