---
layout: default
title: Status
---

# Project Status

**Last updated:** July 2026 · v0.930

---

## Current Phase: Live Platform + Growing Game Engine

The chat and hub are **operational and in daily use** at [united-humanity.us/chat](https://united-humanity.us/chat). Alongside the web platform, a native Rust game engine (`HumanityOS`, one desktop binary) is under active development, teaching real survival skills (farming, construction, off-grid power, water systems) through simulation. This page summarizes both halves; the full build-by-build inventory lives in the project's [STATUS](https://github.com/Shaostoul/Humanity/blob/main/docs/STATUS.md) doc.

---

## What's Built

### Communication (web chat)
- ✅ Channels (public rooms)
- ✅ E2E encrypted DMs, Kyber768 / ML-KEM-768 (post-quantum) to BLAKE3-KDF to AES-256-GCM, server never sees plaintext
- ✅ Groups (private group conversations)
- ✅ Voice & video calling, WebRTC P2P with camera, screen share, PiP, camera selection
- ✅ Persistent voice channels, always-on WebRTC mesh rooms; native voice (pure-Rust capture/Opus/WebRTC) bridges web and desktop
- ✅ Message search, full-text search across conversations
- ✅ @mentions, persistent emoji reactions, message editing
- ✅ Image sharing and file uploads
- ✅ Browser notifications
- ✅ Pin system, server pins + personal pins

### Social System
- ✅ Follow system, signed profiles replicated across federated servers
- ✅ User profiles with bio and social links
- ✅ Identicons (generated avatars)
- ✅ Device management, list, label, and revoke linked keys
- ✅ Blocking and reporting

### Hub Tools
- ✅ Project board (kanban task management)
- ✅ Marketplace, P2P listings, donation pricing
- ✅ Asset library, file upload, browse by category, search, preview
- ✅ Universal catalog (elements, materials, processing chains)
- ✅ Personal inventory tracker, notes, todos
- ✅ Garden tracker

### Game Engine (native Rust client)
- ✅ Data-driven ECS (hecs) with 15+ simulation systems: farming, crafting, vehicles, quests, weather, disasters, ecology
- ✅ PBR renderer (wgpu), voxel asteroids, ship interiors from data files
- ✅ Planet-scale rendering: a chunked-LOD Earth at true 1 m scale with real elevation and satellite imagery, live NASA weather, volumetric cloud families, wave-simulated oceans with real shorelines, physical atmospheric scattering, sun shadows, SSAO, and god rays; the Moon, Mars and Pluto get procedurally cratered terrain
- ✅ Home-building and construction, walls, utilities, electrical/solar/plumbing, mothership superstructure
- ✅ Rapier3d physics, spatial audio
- ✅ Multiplayer relay-backed world (co-presence client wiring shipping)

### Platform & Security
*(Canonical, current crypto details live in the project's `CLAUDE.md`; the bullets below summarize the post-quantum state.)*
- ✅ Cryptographic identity (Dilithium3 / ML-DSA-65, post-quantum, derived from a BIP39 seed), no accounts, no passwords
- ✅ Key backup/export/import
- ✅ Encrypted user data sync, AES-256-GCM
- ✅ PWA, installable on mobile
- ✅ Signed, auto-updating native desktop builds (Windows, macOS, Linux)
- ✅ Settings panel, themes
- ✅ Rate limiting, upload validation, CSP headers, TLS 1.2+, HSTS
- ✅ Server federation, discovery, signed-profile gossip between servers

### Documentation
- ✅ Humanity Accord (civilizational framework)
- ✅ Full technical design specs
- ✅ Architecture decision records

---

## In Progress

- 🔄 **Planet-scale rendering realism**, the current focus: physically-based
  atmosphere and oceans, vegetation level-of-detail, and the performance work to
  keep a true-scale planet smooth from orbit down to walking on the ground
- ⏸️ **Multiplayer co-presence + character selection** for the native client
  (client wiring shipped; paused behind the rendering arc, pending a live two-player test)
- ⏸️ **First Playable / live home sim depth**, vitals HUD, guided first day
- ⏸️ **Mothership superstructure**, zone editor, civic/market zones, multi-home power grids

---

## What's Planned

- ⏳ In-game commerce (virtual spaces with real retailer tie-ins)
- ⏳ Payment processing
- ⏳ Content-addressed file sharing
- ⏳ Local AI integration

The full, continuously-updated backlog is public: see the project's [ROADMAP](https://github.com/Shaostoul/Humanity/blob/main/docs/ROADMAP.md).

---

## Recent Milestones

- **Jul 2026**, v0.874-0.930: the planet-scale arc, a true-1 m-scale chunked-LOD Earth with live weather and cloud families, Gerstner oceans and physical shorelines, sun shadow mapping, SSAO and god rays, then a realism pass on atmosphere and water plus the performance work behind it
- **Jul 2026**, v0.639-0.873: everyday-use features, portable mode and an in-app file browser, chat daily-use parity (saved servers, unread, voice calls), typed containers, and the kit-to-factory-to-drone economy chain
- **Jun 2026**, v0.629-0.637: mothership superstructure (zones, conduits, rail transit), zone interactivity
- **May 2026**, Full post-quantum cryptography cutover (Dilithium3 identity, Kyber768 DMs), native voice chat
- **2026 (ongoing)**, Home-building/construction system, farming and crafting gameplay loop, aeroponic towers, seed economy
- **Late 2025 / early 2026**, Native Rust game engine bootstrapped (ECS, renderer, terrain); chat platform, marketplace, and project board matured on the web side

---

## How to Help

This is an open project. Contributions welcome at every level:

- **Developers**, Rust, JavaScript, Node.js, WebRTC
- **Writers**, improve docs and clarity
- **Designers**, UI/UX, concept art, 3D models
- **Testers**, use the platform, report bugs
- **Translators**, make this accessible worldwide

→ [Get Involved](/Humanity/get-involved)

---

*The future is constructed by those who show up.*
