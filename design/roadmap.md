---
title: HumanityOS — Feature Roadmap
category: design
status: living document
updated: 2026-03-13
---

# HumanityOS Feature Roadmap

This document is the canonical priority list for HumanityOS development.
Update it when priorities shift. It lives in git so it survives any local failure.

---

## Tier 1 — Foundational (blocks real adoption)

### ① Identity Recovery
Users have no way to recover their Ed25519 identity if they lose their device.
This is the single largest trust barrier.

**Approach**: BIP39 12-word mnemonic → PBKDF2 stretch → AES-GCM wrap private key.
Store only the wrapped key. On recovery: enter seed → unwrap → restore identity.

**Files**: `crypto.js` (wrap/unwrap), `chat-profile.js` (recovery UI modal)
**Complexity**: Medium (2–3 days)

---

### ② Federation — Server Network
One server is not a civilization OS. Servers need to discover each other,
establish trust (Humanity Accord verification), and route messages between networks.

**Approach**: Server-signed identity cards, gossip-based discovery, tier-based trust.
Skeleton exists in `handlers/federation.rs` — needs protocol definition + UI.

**Files**: `handlers/federation.rs`, `relay.rs`, new `federation.html` page
**Complexity**: High (1–2 weeks)

---

### ③ Multi-Device Key Sync
Same identity on phone + desktop. ECDH-based secure channel between owned devices.
QR scan or short numeric code to authorize the second device.

**Files**: `crypto.js`, `chat-p2p.js` (reuse WebRTC DataChannel)
**Complexity**: Medium (3–4 days)

---

## Tier 2 — Core OS Layer

### ④ Task / Mission Control System *(in progress)*
Fibonacci-scoped kanban board for coordinating work at every civilization scale.
See `design/tasks/fibonacci-scope.md` for the full scope model.

**Status**: `tasks.html` built, `/api/tasks` functional.
**Remaining**: WebSocket task creation for regular users (not just bot API).

---

### ⑤ Calendar with RSVP
Time coordination is fundamental to any OS. `calendar.html` exists as stub.

**What**: Events, recurring schedules, group calendars, RSVP, reminders.
**Files**: `calendar.html`, new relay storage + WebSocket messages

---

### ⑥ Skills + Verifiable Reputation
Self-sovereign résumé. Skills claimed by user, endorsed by peers with Ed25519 signatures.
Turns the identity system into something economically useful.

**Files**: `skills.html`, relay profile storage extension

---

### ⑦ Personal Data Store
Notes, files, journal entries that travel with your identity.
Local-first, encrypted at rest with identity key.

**Files**: New `notes.html`, IndexedDB + optional relay sync

---

## Tier 3 — Civilization Scale

### ⑧ Group Governance / Voting
Turns chat groups into cooperatives. Proposals, ranked-choice votes, quorum rules.
All votes signed with Ed25519 — verifiable, tamper-evident.

---

### ⑨ Marketplace / Resource Exchange
Offer/request skills, goods, time. Reputation-gated.
Listings API already exists at `/api/listings`. Needs real UI.

---

### ⑩ Learning Paths
Structured pathways: complete module → peer-endorsed → unlock next.
Design detail in `design/education_model.md`.

---

## Tier 4 — Reach

### ⑪ Progressive Web App (PWA)
Service worker + manifest.json + offline cache + push notifications.
Gets HOS onto phones without a full native app. ~1 day of work.

### ⑫ Public API + Webhooks
User-facing API keys, webhook endpoints, rate limiting.
Enables third-party integrations and turns HOS into a platform.

### ⑬ Mobile App (Tauri Mobile / React Native)
Full native app. Lower priority than PWA given Tauri desktop already works.

---

## Completed

- ✅ Chat — voice, video, DMs (E2E encrypted), reactions, pins, threads, search
- ✅ P2P contact cards (Ed25519-signed, QR code, WebRTC DataChannel)
- ✅ Profile system with privacy controls
- ✅ Group system with roles
- ✅ Federation skeleton (handlers/federation.rs)
- ✅ Module splits: app.js → 8 modules, relay.rs → handlers/, storage.rs → 14 domains
- ✅ Automated CI/CD deploy pipeline (GitHub Actions → VPS)
- ✅ 18-page site with unified nav (shell.js)
- ✅ Desktop app (Tauri, Windows/Mac/Linux)
- ✅ Task API backend (/api/tasks CRUD + WebSocket broadcast)
- ✅ Fibonacci-scoped task board (tasks.html)
