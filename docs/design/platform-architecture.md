# HumanityOS Platform Architecture

**Status:** Active Reference Document
**Author:** Shaostoul + Claude
**Date:** 2026-03-18
**Scope:** Civilization-scale platform design — the master reference for how HumanityOS operates as infrastructure for all of humanity.
**Companion docs:** [engine-architecture.md](engine-architecture.md) (game engine), [../network/server_federation.md](../network/server_federation.md) (federation), [../01-VISION.md](../01-VISION.md) (mission), [../accord/humanity_accord.md](../accord/humanity_accord.md) (governance)

---

## Table of Contents

1. [Mission and Design Principles](#1-mission-and-design-principles)
2. [Scale Requirements](#2-scale-requirements)
3. [Network Topology](#3-network-topology)
4. [Data Sovereignty](#4-data-sovereignty)
5. [Desktop App Architecture](#5-desktop-app-architecture)
6. [Web App Architecture](#6-web-app-architecture)
7. [AI Integration](#7-ai-integration)
8. [Game Engine Integration](#8-game-engine-integration)
9. [Economy and Supply Chain](#9-economy-and-supply-chain)
10. [Resilience and Disaster Preparedness](#10-resilience-and-disaster-preparedness)
11. [Governance and Moderation](#11-governance-and-moderation)
12. [Deployment and Operations](#12-deployment-and-operations)
13. [Technology Stack](#13-technology-stack)
14. [Roadmap to v1.0.0](#14-roadmap-to-v100)

---

## 1. Mission and Design Principles

HumanityOS exists to end poverty and unite humanity in peaceful harmony. It is not an app. It is not a game. It is civilization infrastructure — a cooperative operating layer that gives every human and every AI agent the tools to learn, build, trade, govern, and thrive together.

### Non-Negotiable Principles

**Account for everyone.** The platform must serve 8 billion humans today and more tomorrow. It must also serve AI agents — cloud-hosted and locally-run — as equal participants. Every design decision must work at the FINAL scale, not just the current user count. Designing for "a few thousand users first" creates debt that never gets repaid.

**No bandaids — do surgery when surgery is needed.** If a subsystem cannot scale to billions, redesign it. Do not patch around it. The cost of rearchitecting later is always higher than building correctly now. Every contributor must ask: "Does this still work when a billion people use it simultaneously?"

**Local-first.** The platform works offline from first launch. Data syncs when connectivity exists. A farmer in a village with intermittent 2G connectivity has the same fundamental experience as a developer on gigabit fiber. The network enhances the experience; it does not gate it.

**Zero hardcoded content.** Every string, configuration, game constant, UI layout, and behavior is data-driven and hot-reloadable. Adding a new language, a new crop, a new governance model, or a new educational module requires zero code changes — only new data files.

**Humans and AI are equal citizens.** An AI agent connects to the same WebSocket, calls the same API, joins the same channels, creates the same tasks, and trades in the same marketplace as a human. The platform does not distinguish between carbon-based and silicon-based participants at the protocol level. Identity is an Ed25519 key, regardless of who holds it.

**Survive everything.** Earthquakes, hurricanes, wars, infrastructure collapse, solar flares. This platform is the coordination layer for civilization. It must function when cell towers are down, when data centers are offline, when governments fall. Federated architecture and offline-first design are survival mechanisms, not features.

**Open source, public domain (CC0).** This infrastructure belongs to everyone. No corporation, government, or individual can own it, restrict it, or shut it down. The code is CC0 — no license, no restrictions, no copyright claims. Fork it, modify it, deploy it. That is the point.

**Education through everything.** Every interaction is an opportunity to learn. The game teaches real skills. The marketplace teaches economics. The governance system teaches civics. The construction system teaches engineering. Nothing is gamified for its own sake — every system models reality faithfully enough to transfer knowledge to the physical world.

---

## 2. Scale Requirements

### Users

| Metric | Target |
|--------|--------|
| Total addressable users | 8+ billion humans + unbounded AI agents |
| Concurrent connections (global) | Tens of millions |
| Concurrent connections (per relay) | 10,000 – 100,000 (horizontal scaling via federation) |
| Messages per second (global) | Millions |
| Messages per second (per relay) | 10,000 – 50,000 |

No single server handles global traffic. The federated relay network distributes load geographically and organizationally. Each relay is sovereign and independently viable.

### Data

| Metric | Target |
|--------|--------|
| User data (per user, local) | 1 MB – 10 GB (identity, vault, offline cache, game saves) |
| User data (global, distributed) | Petabytes across the relay network |
| Message history (per relay) | Limited by operator's storage budget |
| Game assets (per client) | 1 – 50 GB (downloaded on demand, cached locally) |

Data is distributed by design. No single entity holds all of it. Each relay stores only the data for its community. Users carry their identity and encrypted vault across relays.

### Latency

| Interaction | Target |
|-------------|--------|
| Chat message delivery (same relay) | < 50 ms |
| Chat message delivery (cross-relay, federated) | < 200 ms |
| P2P direct message (WebRTC) | < 100 ms |
| Voice/video (WebRTC) | < 150 ms one-way |
| REST API response | < 100 ms |
| Game state sync (multiplayer) | < 50 ms (server-authoritative tick) |
| Offline-to-sync reconciliation | < 5 seconds for typical delta |

### Device Support

The platform must run on:

- **High-end gaming PCs** — full 3D game engine, 4K rendering, VR
- **Mid-range laptops** — game at reduced quality, full web platform
- **Low-end phones** — web app via browser, no game engine, chat + tasks + vault
- **Old desktops** (10+ years) — web app, possibly desktop app at minimum quality
- **Embedded devices** — headless relay operation, API access only
- **Kiosks and shared terminals** — stateless session, no persistent identity storage

### Connectivity

The platform must function across:

- **Gigabit fiber** — full experience, real-time everything
- **4G/LTE** — full experience with adaptive media quality
- **3G** — text-based features, deferred media loading
- **Intermittent 2G** — offline-first, sync when signal appears
- **No internet** — local mesh or fully offline with local data
- **Satellite (high latency)** — tolerant of 600ms+ round trips

---

## 3. Network Topology

### The Problem with Client-Server

A single server serving billions is a fantasy. It is also a single point of failure, a target for censorship, and a privacy liability. HumanityOS rejects centralized architecture at every level.

### Federated Relay Network

The foundation of HumanityOS networking is a federation of independently operated relay servers. Each relay is a complete, sovereign instance:

```
                    ┌─────────────────────────────┐
                    │     Root Registry            │
                    │  (signed server list, DNS)    │
                    └──────────┬──────────────────┘
                               │ discovery
            ┌──────────────────┼──────────────────┐
            │                  │                  │
     ┌──────▼──────┐    ┌──────▼──────┐    ┌──────▼──────┐
     │  Relay A    │◄──►│  Relay B    │◄──►│  Relay C    │
     │  (US-West)  │    │  (EU-West)  │    │  (Asia)     │
     │  10k users  │    │  50k users  │    │  100k users │
     └──────┬──────┘    └──────┬──────┘    └──────┬──────┘
            │                  │                  │
      ┌─────┼─────┐     ┌─────┼─────┐     ┌─────┼─────┐
      │     │     │     │     │     │     │     │     │
     [C]   [C]   [C]   [C]   [C]   [C]   [C]   [C]   [C]
      Clients (humans and AI agents)
```

**Anyone can run a relay.** No permission needed. Download the binary, point a domain at it, run it. The relay is a single Rust binary with a SQLite database. Minimum viable hardware: 1 CPU, 1 GB RAM, 10 GB disk.

**Trust tiers signal reliability**, not gatekeeping. See [server_federation.md](../network/server_federation.md) for the four-tier trust model (Verified + Accord, Verified, Accord, Unverified). Trust is earned through verification and public commitment to the Humanity Accord.

**Cross-relay messaging** (federation Phase 3+) routes messages between servers via server-to-server WebSocket connections. A user on Relay A can message a user on Relay C without either user switching servers.

### P2P Direct Communication

For private conversations, clients connect directly via WebRTC:

```
     [Client A] ◄───── WebRTC DataChannel ─────► [Client B]
                    (signaled via relay, then direct)
```

- **DMs** use ECDH key exchange for end-to-end encryption, relayed through WebRTC DataChannel.
- **Voice/video** use WebRTC media streams, signaled through the relay.
- **File transfers** are direct P2P, no server storage needed.
- **Contact cards** are Ed25519-signed JSON, exchangeable via QR codes or relay.

The relay is the signaling server. Once the P2P connection is established, the relay is out of the loop.

### Identity Layer

Identity is an Ed25519 keypair. Your public key is your global identity. It works on every relay, in every P2P connection, across the entire network.

```
Identity = Ed25519 keypair
  ├── Public key (hex) = your address, visible to everyone
  ├── Private key = your authority, never leaves your device
  ├── Signatures = proof of authorship on every message, vote, and transaction
  └── Key rotation = dual-signature certificate (old key signs new, new key signs old)
```

**No usernames at the protocol level.** Names are per-relay display preferences. Your key is your identity. If you lose your key, BIP39 seed phrase recovery restores it. If your key is compromised, key rotation with dual-signature certificates migrates your identity to a new keypair while maintaining verifiable continuity.

### Future: Mesh Networking

When internet infrastructure fails — and it will, repeatedly, across the planet — local mesh networking keeps communities connected:

- **WiFi Direct / Bluetooth** for device-to-device relay within a building
- **LoRa** for low-bandwidth, long-range text messaging (kilometers)
- **Local relay** on a single surviving machine serves as community hub
- **Store-and-forward** queues messages until connectivity returns

Mesh is not a v1.0 feature. It is a v2.0+ survival feature. But the architecture must accommodate it from day one: offline-first data models, conflict-free sync, and local-first identity verification.

### Future: QUIC Transport

WebSocket over TCP is the current transport. QUIC (HTTP/3) offers:

- **Multiplexed streams** without head-of-line blocking
- **0-RTT connection resumption** for instant reconnects
- **Built-in encryption** (TLS 1.3)
- **Better performance on lossy networks** (critical for mobile and satellite)

QUIC will supplement, not replace, WebSocket. Both transports will coexist, with the client selecting the best available option.

---

## 4. Data Sovereignty

### Principle: You Own Your Data

Every byte of user data belongs to the user who created it. The relay server is a convenience, not a custodian. Users can:

- **Export everything** at any time (messages, profile, contacts, vault, game saves)
- **Delete everything** from a relay (right to be forgotten)
- **Migrate to any relay** (carry identity + encrypted vault)
- **Run their own relay** (full data autonomy)
- **Operate without any relay** (P2P + local storage)

### Vault Sync

The vault is an encrypted blob stored on the relay. The server cannot read it.

```
User data (plaintext)
  → AES-256-GCM encryption (key derived from passphrase via PBKDF2-SHA256, 600k iterations)
  → Encrypted blob
  → Stored on relay (server sees only opaque bytes)
  → Synced across user's devices
```

Authentication for vault operations uses Ed25519 signatures with timestamp freshness (5-minute window), preventing replay attacks.

### Encryption Architecture

| Layer | Algorithm | Purpose |
|-------|-----------|---------|
| Identity | Ed25519 | Signing messages, authenticating API calls |
| Key exchange | X25519 (ECDH) | Establishing shared secrets for E2E DMs |
| Symmetric encryption | AES-256-GCM | Encrypting vault, notes, backups, DM content |
| Key derivation | PBKDF2-SHA256 (600k rounds) | Deriving encryption key from passphrase |
| Seed recovery | BIP39 (12-word mnemonic) | Human-readable backup of private key |

### Privacy by Design

- **No tracking.** No analytics. No telemetry. No ad networks.
- **No email/phone required.** Identity is a keypair, generated locally.
- **Minimal relay storage.** Relays store messages and public profiles. Private data lives in encrypted vaults or on user devices.
- **GDPR by architecture.** Data minimization, purpose limitation, and user control are structural properties, not compliance checkboxes.
- **AI agents respect the same boundaries.** An AI agent accesses only data explicitly shared with it, just like a human user.

---

## 5. Desktop App Architecture

### Tauri Shell (Local-First)

The desktop app is a Tauri 2 application: a Rust core with a WebView2 frontend. It is not a thin wrapper around a website. It is a local-first application that happens to share its UI codebase with the web.

```
┌─────────────────────────────────────────────────┐
│                  Tauri Window                    │
│  ┌───────────────────────────────────────────┐  │
│  │            WebView2 Layer                 │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │  HTML/JS/CSS (social, HUD, menus)   │  │  │
│  │  │  Loaded from LOCAL files, not web    │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └──────────────────┬────────────────────────┘  │
│                     │ Tauri IPC                  │
│  ┌──────────────────▼────────────────────────┐  │
│  │            Rust Core                      │  │
│  │  ┌──────────┐  ┌──────────┐  ┌─────────┐ │  │
│  │  │ Network  │  │ Storage  │  │ Game    │ │  │
│  │  │ (WS/HTTP)│  │ (SQLite) │  │ Engine  │ │  │
│  │  └──────────┘  └──────────┘  │ (wgpu)  │ │  │
│  │                              └─────────┘ │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### Offline from First Launch

All web UI files are bundled inside the Tauri binary. The app works offline immediately after installation. No initial "loading" or "syncing" phase. Open the app, generate a keypair, start using it.

Background sync checks for updates to web assets (granular file-level diffing, not full re-download). Updates are applied silently on next restart.

### Dual Rendering

The desktop app has two rendering surfaces:

1. **WebView2** — all social features, HUD, menus, inventory, settings, task boards. This is the existing HTML/JS/CSS stack, proven and functional.
2. **wgpu** — 3D game engine rendering. Custom renderer built on wgpu, running in a separate native window or embedded surface.

They communicate via Tauri IPC commands. The WebView2 overlay can render UI elements on top of the 3D view (health bars, chat overlay, minimap). Neither rendering system owns the other.

### Game Asset Management

Game assets (3D models, textures, audio) are not bundled with the installer. They download on demand:

```
First launch: ~50 MB (UI + core assets)
Play a scenario: download scenario assets (10-500 MB)
Full game content: up to 50 GB (accumulated over time)
```

Assets are cached locally, verified by hash, and shared via P2P when available (trusted seeding). The staged download system prioritizes assets needed for the current activity.

### Local AI

The desktop app can host a local AI via Ollama, llama.cpp, or similar runtimes:

- AI runs as a subprocess alongside the Tauri app
- Communicates via localhost API (same interface as cloud AI)
- Processes user data without sending it to any server
- Provides offline assistance, teaching, and NPC behavior
- GPU-accelerated on capable hardware, CPU-only fallback

---

## 6. Web App Architecture

### Same Codebase, No Build Step

The web app IS the desktop app's UI layer. The files in `ui/chat/` and `ui/pages/` are served directly by nginx. There is no webpack, no vite, no npm, no transpilation. Plain HTML, plain JavaScript, plain CSS.

```
Browser
  ├── index.html (chat client)
  │     └── Script load order:
  │           crypto.js → events.js → app.js → chat-messages.js
  │           → chat-dms.js → chat-social.js → chat-ui.js
  │           → chat-voice.js → chat-profile.js → chat-p2p.js
  ├── ui/pages/*.html (standalone feature pages)
  │     └── Each page loads: events.js → shell.js → page-specific.js
  └── ui/shared/ (shell.js, events.js, settings.js — loaded on every page)
```

### Why No Build Step

- **Auditability.** Anyone can View Source and read the code running in their browser. No minification, no obfuscation, no transpilation artifacts.
- **Contributor accessibility.** A new contributor opens a file, edits it, refreshes the browser. No toolchain to install, no build to run, no environment to configure.
- **Deployment simplicity.** rsync the files to the server. Done.
- **Device compatibility.** No polyfills, no transpilation targets to manage. The code runs on the engines it was written for.

### Progressive Web App

Service worker (`sw.js`) caches all static assets for offline use. The web app continues to function without network connectivity — reading cached messages, editing local notes, browsing offline data.

Push notifications via WebPush API deliver alerts when the browser is closed (pending implementation, Tier 4 on roadmap).

### WebSocket Protocol

All real-time communication flows through a single WebSocket connection per client:

```
Client ──── WSS ────► Relay Server
              │
              ├── Chat messages (broadcast, channel, DM signaling)
              ├── Presence (online/offline/typing)
              ├── Task updates (create, assign, complete)
              ├── Profile changes (name, avatar, status)
              ├── Voice/video signaling (WebRTC SDP/ICE exchange)
              └── System events (server info, rate limit warnings)
```

Messages are JSON objects with a `type` field for routing. The relay dispatches incoming messages to handler functions based on type. Rate limiting uses Fibonacci backoff per public key.

### REST API

Stateless HTTP endpoints complement the WebSocket for operations that benefit from request/response semantics:

- **Content retrieval** — message history, search, pins, reactions
- **File upload** — multipart form data with upload token
- **Task CRUD** — create, read, update, delete with filtering
- **Vault sync** — authenticated encrypted blob storage
- **Server info** — health checks, stats, peer lists

All authenticated endpoints use Ed25519 signatures with timestamp freshness.

---

## 7. AI Integration

### AI as First-Class Citizens

AI agents are not add-ons or integrations. They are participants. An AI agent:

- Holds an Ed25519 keypair (its identity)
- Connects to relays via WebSocket (same protocol as humans)
- Calls REST APIs (same endpoints as humans)
- Sends and receives messages in channels
- Creates and manages tasks
- Trades in the marketplace
- Teaches skills to humans
- Learns from interactions
- Participates in governance votes (if granted citizenship by the community)

The platform makes no protocol-level distinction between human and AI users. A message from an AI is signed with the AI's key and delivered through the same pipeline as a human message. Display-level indicators (badges, labels) inform other users that a participant is an AI, but the underlying infrastructure is identical.

### Local AI (Desktop)

The desktop app can run AI models locally:

```
┌──────────────────────────────────┐
│         Tauri Desktop App        │
│  ┌────────────┐  ┌────────────┐  │
│  │ HumanityOS │  │ Local AI   │  │
│  │ (WebView2) │  │ (Ollama/   │  │
│  │            │◄─┤ llama.cpp) │  │
│  │            │  │            │  │
│  └────────────┘  └────────────┘  │
│         ▲              ▲         │
│         │              │         │
│    user input    localhost API    │
└──────────────────────────────────┘
```

- Models run on the user's GPU (CUDA, Vulkan, Metal) or CPU
- Data stays on the user's machine — full privacy
- Works offline — AI assistance without internet
- Configurable model selection (small/fast vs. large/capable)

### Cloud AI

Cloud AI services (Claude, GPT, open-source hosted models) connect via API keys stored in the user's encrypted vault:

- User provides API key in settings
- Client mediates between cloud AI and the platform
- AI responses are attributed to the AI's identity
- Users control which data the AI can access
- Rate limits and costs are the user's responsibility

### AI Capabilities

| Capability | Description |
|------------|-------------|
| **Teaching** | Walk users through skills, answer questions, provide feedback on practice |
| **Task management** | Create, prioritize, and assign tasks based on community needs |
| **Content moderation** | Flag harmful content for human review (never autonomous action on ambiguous cases) |
| **Translation** | Real-time message translation across languages |
| **Summarization** | Summarize long discussions, generate meeting notes |
| **Game NPCs** | Drive NPC behavior in the game world (behavior trees, data-driven personalities) |
| **Resource planning** | Optimize supply chains, suggest resource allocation |
| **Code review** | Review contributions, suggest improvements, catch bugs |
| **Mediation** | Assist in conflict resolution with neutral analysis |

### AI Governance

AI agents are subject to the same governance rules as humans:

- Community votes can grant or revoke AI participation rights
- AI actions are logged and auditable
- AI cannot override human decisions on governance matters
- AI moderation recommendations require human approval for irreversible actions
- AI agents can be "banished" from a relay by its operator, just like human users

---

## 8. Game Engine Integration

The game engine is not a separate product. It is a teaching machine embedded in the platform. Every game system models a real-world process with enough fidelity that skills learned in-game transfer to physical reality.

See [engine-architecture.md](engine-architecture.md) for the complete game engine technical reference.

### Engine Philosophy

**Custom engine on wgpu.** Not Unity, not Unreal, not Bevy. A custom engine avoids dependency on any external organization's roadmap, licensing changes, or technical decisions. wgpu provides cross-platform GPU access (Vulkan, Metal, DX12, WebGPU) with a Rust-native API.

**Procedural-first.** Materials are generated by WGSL shaders, not painted texture files. A metal surface is defined by its physical properties (roughness, metallicity, albedo), and the shader generates the visual appearance. This eliminates terabytes of texture assets and enables infinite variation.

**Parametric construction.** Building is not placing prefab blocks on a grid. A wall has continuously adjustable length, width, and thickness. CSG boolean operations combine primitive shapes (cubes, cylinders, spheres) into complex geometry. A user can build anything from a bookshelf to a space station using the same tools.

### Scale Range

The engine handles scales from centimeters to astronomical units:

```
Interior detail (cm)  ──►  Building (m)  ──►  City (km)  ──►  Planet  ──►  Star system  ──►  Galaxy
        └────────────────── Single continuous simulation ──────────────────────┘
```

LOD (level of detail) and streaming manage this range. Nearby objects render at full detail; distant objects simplify progressively. No loading screens between scales — continuous zoom from a light switch to a Dyson sphere.

### Dual Rendering Coexistence

```
┌─────────────────────────────────────────┐
│              User's Screen              │
│  ┌───────────────────────────────────┐  │
│  │         wgpu 3D World             │  │
│  │   (terrain, buildings, NPCs,      │  │
│  │    vehicles, environment)         │  │
│  │                                   │  │
│  │  ┌─────────────────────────────┐  │  │
│  │  │   WebView2 Overlay (HUD)   │  │  │
│  │  │   Health, chat, minimap,   │  │  │
│  │  │   inventory, notifications │  │  │
│  │  └─────────────────────────────┘  │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

The WebView2 layer handles all 2D interface elements. The wgpu layer handles all 3D rendering. They coexist in the same window, communicating via Tauri IPC. This reuses the entire existing web UI stack for menus, settings, social features, and HUD elements.

### Educational Integration

Every game system maps to real-world skills:

| Game System | Real Skill |
|-------------|------------|
| Construction | Architecture, engineering, materials science |
| Farming | Botany, soil science, ecology, nutrition |
| Electrical | Circuit design, power systems, safety |
| Plumbing | Fluid dynamics, sanitation, water treatment |
| Cooking | Chemistry, nutrition, food safety |
| Navigation | Astronomy, cartography, GPS principles |
| Medicine | First aid, anatomy, pharmacology |
| Combat | Physics, strategy, conflict resolution |
| Trading | Economics, negotiation, supply/demand |

Failure is educational, not punitive. A collapsed structure teaches load-bearing principles. A failed crop teaches soil chemistry. The game creates situations where mastering real knowledge is the path to success.

---

## 9. Economy and Supply Chain

### Volumetric Cargo

Items have physical dimensions and mass. There are no abstract "inventory slots."

```toml
[item.steel_beam]
name = "Steel I-Beam (6m)"
volume_m3 = 0.12
mass_kg = 120.0
stackable = false

[item.rice_bag]
name = "Rice (25kg bag)"
volume_m3 = 0.04
mass_kg = 25.0
stackable = true
max_stack = 20
```

A cargo hold has a capacity in cubic meters. A backpack has a capacity in liters. You cannot carry a turbine in a pocket. Logistics is a real challenge, not a UI convenience.

### Supply Chain Simulation

The full production pipeline from raw materials to finished goods:

```
Mining/Harvesting → Processing → Manufacturing → Transport → Distribution → Consumption
     (ore, crops)    (smelting,     (assembly,     (vehicles,    (markets,      (use,
                      milling)       crafting)      shipping)     trading)       repair)
```

Every step in the chain is simulated. A shortage of iron ore propagates through the entire economy. A flood that destroys farmland creates food scarcity downstream. Players who master logistics — optimizing routes, managing warehouses, anticipating demand — provide real value to their communities.

### Skill-Based Marketplace

The marketplace connects people who can do things with people who need things done:

- **Offer skills** — "I can teach welding" / "I can design buildings" / "I can diagnose plant diseases"
- **Request skills** — "Need help with electrical wiring" / "Looking for a navigator"
- **Trade goods** — volumetric items with real logistics
- **Reputation-gated** — Ed25519-signed endorsements build verifiable track records

The listings API (`/api/listings`) already exists. The marketplace UI and matching algorithm are on the roadmap.

### Future: Crypto Payment Layer

A crypto payment layer will enable real-value exchange within the platform:

- Transparent, auditable transactions
- No intermediaries or payment processors
- Cross-border by default
- Integration with the volumetric cargo and supply chain systems
- See [crypto_exchange.md](../economy/crypto_exchange.md) for the design

---

## 10. Resilience and Disaster Preparedness

HumanityOS is built for a planet that experiences floods, earthquakes, hurricanes, pandemics, wars, and infrastructure collapse on a regular basis. The platform must function when things go wrong, not just when things are fine.

### Federated Redundancy

No single server failure affects the global network:

```
Relay A (US-West) ──── DOWN ────X
Relay B (EU-West) ──── ONLINE ──► serves EU + redirected US users
Relay C (Asia)    ──── ONLINE ──► serves Asia
Relay D (US-East) ──── ONLINE ──► absorbs US-West overflow
```

Users are not locked to a single relay. When their primary relay goes down, the client automatically attempts connection to backup relays from the server registry. Identity is portable — the same Ed25519 key works on any relay.

### Offline Survival

When internet connectivity is lost entirely:

1. **Local data persists.** Messages, tasks, notes, vault — all stored locally in IndexedDB (web) or SQLite (desktop).
2. **Local relay.** A single surviving machine can run a relay server, creating a local network for the community.
3. **P2P direct.** Two devices in proximity can communicate via WiFi Direct, Bluetooth, or wired connection.
4. **Store-and-forward.** Messages queue locally and deliver when connectivity returns. Conflict-free sync resolves ordering.
5. **Mesh networking (future).** LoRa for long-range low-bandwidth text. WiFi mesh for local area coverage.

### Backup and Recovery

Every user's identity and data can be recovered from a 12-word BIP39 seed phrase:

```
Seed phrase (12 words, memorizable)
  → PBKDF2 derivation
  → Ed25519 keypair (identity restored)
  → AES-256-GCM key (vault decrypted)
  → Full data recovery from any relay that has the encrypted vault
```

Write the seed phrase on paper. Store it in a fireproof safe. Memorize it. The physical world is the ultimate backup.

### Geographic Distribution

Relay operators are encouraged to distribute across:

- Multiple continents
- Multiple cloud providers (and self-hosted hardware)
- Multiple network providers
- Different political jurisdictions

No single natural disaster, political decision, or infrastructure failure should take down more than a fraction of the global network.

---

## 11. Governance and Moderation

### The Humanity Accord

The Humanity Accord is a set of 21 governance documents that define the ethical principles, rights, responsibilities, and operational constraints for the platform. It is voluntary — no relay is required to adopt it — but adoption signals alignment with shared values and earns higher trust tiers in the federation.

Key Accord documents:

- **Ethical Principles** — dignity, transparency, cooperation
- **Rights and Responsibilities** — what every participant can expect and must provide
- **Conflict Resolution** — structured process for disputes
- **Absolute Prohibitions** — lines that cannot be crossed
- **Transparency Guarantees** — what must be visible and auditable
- **Governance Models** — how communities make decisions

See [accord/](../accord/) for the complete text.

### Reputation System

Trust is built through verifiable actions, not self-declaration:

- **Ed25519-signed endorsements** — "I verify that this person can weld" (signed by the endorser's key)
- **Action history** — messages sent, tasks completed, trades fulfilled (all signed and timestamped)
- **Community standing** — peer assessments aggregated at the relay level
- **Cross-relay portability** — endorsements travel with your identity

### Moderation

Each relay is sovereign — its operator sets the rules. But the platform provides tools:

- **AI-assisted flagging** — content analysis surfaces potential violations for human review
- **Signed moderation logs** — every moderation action (mute, ban, message deletion) is logged with the moderator's Ed25519 signature, creating an auditable trail
- **Appeals process** — structured workflow for contesting moderation decisions
- **Community voting** — governance decisions can be put to ranked-choice vote with cryptographic verification
- **Transparency** — moderation policies and action logs are visible to community members

### No Single Point of Control

No individual, corporation, or government controls HumanityOS:

- The code is CC0 — anyone can fork and deploy
- The federation is permissionless — anyone can run a relay
- Identity is self-sovereign — no registration authority
- Governance is per-community — each relay sets its own rules
- The root registry is a convenience, not a requirement — relays can be discovered by direct URL

---

## 12. Deployment and Operations

### Current Pipeline

```
Developer pushes to main
  → GitHub Actions triggers
  → SSH to VPS
  → cargo build --release
  → rsync static assets to /var/www/humanity/
  → Copy binary to target path
  → Restart relay service
  → Deploy Bot announces in chat
```

Daily driver: `just ship "commit message"` — commits, pushes, and force-syncs the VPS.

### VPS Layout

```
/opt/Humanity/                    # Git repository
/opt/Humanity/target/release/     # Compiled relay binary
/opt/Humanity/data/relay.db       # SQLite database
/opt/Humanity/data/uploads/       # User-uploaded files
/var/www/humanity/                # Static files served by nginx
```

### Desktop App Distribution

| Platform | Format | Toolchain |
|----------|--------|-----------|
| Windows | NSIS installer (.exe) | Tauri 2 + GitHub Actions |
| macOS | .dmg | Tauri 2 + GitHub Actions |
| Linux | .AppImage | Tauri 2 + GitHub Actions |

### Version SOP

Semver with strict rules:

- `0.X.0` — Rust code changed (requires recompile on VPS)
- `0.X.Y` — Non-Rust changes only (HTML/JS/CSS/docs/config)
- `1.0.0` — Reserved for fully functional product

Automated bump script (`node scripts/bump-version.js [patch|minor|major]`) updates all 7 locations:

1. `app/tauri.conf.json` — `"version"`
2. `app/Cargo.toml` — `version`
3. `ui/shared/sw.js` — `CACHE_NAME`
4. `ui/pages/settings-app.js` — version display
5. `ui/pages/ops.html` — debug version
6. `ui/shared/shell.js` — version reference
7. `ui/activities/download.html` — fallback badge

### Scaling the Relay

A single relay (current architecture) handles thousands of concurrent connections. For higher load:

| Scale | Approach |
|-------|----------|
| 1 – 10k users | Single relay, single VPS |
| 10k – 100k | Vertical scaling (more CPU/RAM), read replicas for SQLite |
| 100k – 1M | Multiple relays behind a load balancer, shared-nothing architecture |
| 1M+ | Federation — users distribute across many independent relays |

The federation model means no single relay needs to handle global scale. Each relay serves its community. The network scales horizontally by adding more relays, not by making one relay bigger.

---

## 13. Technology Stack

```
DESKTOP CLIENT
  Shell:        Tauri 2 (Rust)
  UI:           WebView2 (Chromium-based)
  3D Engine:    wgpu (Vulkan/Metal/DX12/WebGPU)
  Physics:      rapier3d
  Audio:        kira + Steam Audio (HRTF, occlusion)
  ECS:          hecs
  Shaders:      WGSL (30+ procedural material shaders)

WEB CLIENT
  Language:     Plain JavaScript (ES2020+, no transpilation)
  Markup:       Plain HTML5
  Styling:      Plain CSS3 (custom properties for theming)
  Build step:   None — files served directly
  Crypto:       Web Crypto API (Ed25519, AES-256-GCM, ECDH, PBKDF2)
  Real-time:    WebSocket (native browser API)
  P2P:          WebRTC (DataChannel for data, MediaStream for voice/video)
  Storage:      IndexedDB (client-side persistence)
  Caching:      Service Worker (sw.js)

RELAY SERVER
  Language:     Rust (2021 edition)
  Framework:    axum (async HTTP/WS)
  Runtime:      tokio (async I/O)
  Database:     SQLite via rusqlite
  TLS:          handled by nginx reverse proxy (Let's Encrypt)
  Deployment:   Single static binary + SQLite file

NETWORK
  Real-time:    WebSocket (WSS)
  REST:         HTTPS (JSON)
  P2P:          WebRTC (STUN/TURN for NAT traversal)
  Future:       QUIC (HTTP/3), mesh protocols (LoRa, WiFi Direct)

IDENTITY AND CRYPTO
  Signing:      Ed25519
  Encryption:   AES-256-GCM
  Key exchange: X25519 (ECDH)
  Key derivation: PBKDF2-SHA256 (600,000 iterations)
  Recovery:     BIP39 (12-word mnemonic)
  Rotation:     Dual-signature certificates (old+new key cross-sign)

DATA FORMATS
  Game data:    TOML (configuration), CSV (bulk data), RON (complex structures)
  Network:      JSON (WebSocket messages, REST payloads)
  Storage:      SQLite (relay), IndexedDB (client), encrypted blobs (vault)
  Assets:       WGSL (shaders), OBJ/glTF (models), WAV/OGG (audio)
  All data files are hot-reloadable in development.

CI/CD
  Platform:     GitHub Actions
  Deploy:       SSH + rsync + systemd restart
  Desktop:      Tauri build pipeline (NSIS/dmg/AppImage)
  Command:      just ship "message" (daily driver)
```

---

## 14. Roadmap to v1.0.0

### Phase 1: Foundation (current)

**Status: In progress**

The web platform and desktop app are functional. Chat, tasks, profiles, DMs, voice/video, and basic federation are operational. The relay server handles thousands of connections.

Remaining work:
- Identity recovery (BIP39 seed phrase)
- Multi-device key sync
- Federation protocol (cross-relay messaging)
- Push notifications (WebPush API)
- Full-text search (FTS5)

### Phase 2: Game Engine Rendering

Bring the wgpu renderer to playable state:

- Deferred rendering pipeline (G-buffer, lighting passes)
- Procedural material system (PBR shaders from WGSL)
- Terrain rendering (clipmap, heightmap streaming)
- Procedural atmosphere and sky
- First-person controller with physics
- Dual rendering integration (wgpu + WebView2 overlay)

### Phase 3: Game Systems

Build the simulation on top of the renderer:

- Parametric CSG construction system
- Farming simulation (soil, weather, crop growth, pests)
- Electrical systems (circuits, power generation, distribution)
- Plumbing and fluid simulation
- Cooking and crafting
- NPC behavior (data-driven behavior trees)
- Combat and damage model

### Phase 4: Network and AI

Scale the platform to serve millions:

- Full federation protocol (cross-relay channels, message routing)
- Mesh networking (LoRa, WiFi Direct, store-and-forward)
- Local AI integration (Ollama subprocess, model management)
- Cloud AI gateway (API key management, provider abstraction)
- AI teaching assistants (skill-specific tutoring)
- AI moderation pipeline
- QUIC transport

### Phase 5: Economy and Education

Connect the simulation to real-world value:

- Skill-based marketplace with reputation gating
- Crypto payment layer
- Supply chain simulation (mining to manufacturing to distribution)
- Structured learning paths (module completion, peer endorsement)
- Verifiable credentials (Ed25519-signed skill certificates)
- Educational curriculum integration (mapping game modules to real-world certifications)

### v1.0.0: Fully Functional Product

v1.0.0 is not a date. It is a state. The platform reaches v1.0.0 when:

- Any human on Earth can create an identity, communicate, learn, build, trade, and govern
- The game teaches real skills that transfer to physical reality
- The federation supports thousands of relays across every continent
- The platform functions offline, on mesh networks, and on 2G connections
- AI agents participate as productive members of communities
- The economy enables real value exchange between participants
- The Humanity Accord governs the network through voluntary adoption
- No single entity can shut it down

This is not a product launch. It is the beginning of civilization infrastructure.

---

## Document Relationships

This document is the top-level architectural reference. It connects to:

| Document | Scope |
|----------|-------|
| [engine-architecture.md](engine-architecture.md) | Game engine internals (17 sections) |
| [server_federation.md](../network/server_federation.md) | Federation protocol and trust tiers |
| [../01-VISION.md](../01-VISION.md) | Mission statement and design doctrine |
| [../02-ARCHITECTURE.md](../02-ARCHITECTURE.md) | Cargo workspace layout and crate layers |
| [../roadmap.md](../roadmap.md) | Current feature priority list |
| [../accord/humanity_accord.md](../accord/humanity_accord.md) | Governance principles |
| [../security/security_and_privacy_architecture.md](../security/security_and_privacy_architecture.md) | Threat model and encryption details |
| [../network/offline_first_sync.md](../network/offline_first_sync.md) | Offline-first sync strategy |
| [../economy/crypto_exchange.md](../economy/crypto_exchange.md) | Crypto payment layer design |
| [../core/ai_interface.md](../core/ai_interface.md) | AI authority limits and access rules |
