# HumanityOS

**Own your tools. Own your life. Own your future.**

A free app where people chat, plan, trade, and build together. No accounts, no owner, no ads, public domain. The infrastructure for cooperation, made for everyone.

🌐 **[united-humanity.us](https://united-humanity.us)** &nbsp; · &nbsp; 💬 **[Chat](https://united-humanity.us/chat)** &nbsp; · &nbsp; 📥 **[Download](https://united-humanity.us/download)** &nbsp; · &nbsp; 💜 **[Discord](https://discord.gg/9XxmmeQnWC)**

---

## 💡 What you can do today

| | What | How it helps |
|---|---|---|
| 💬 | **Talk to anyone, privately** | Text, voice, video calls. Direct messages are locked end-to-end with post-quantum math - only the people in the conversation can read them, not even the server. Threads, search, reactions, screen share. |
| 📋 | **Organize anything** | Kanban boards, calendars, private encrypted notes, skill tracking. Run a team, a club, or your whole life from one place. |
| 🛒 | **Buy, sell, and trade** | Built-in marketplace with listings, reviews, seller ratings, and escrowed trades. A separate multi-layer trust score (identity, vouching, activity - no surveillance) shows who you're dealing with. |
| 🆔 | **Prove who you are** | Schools, employers, and communities can issue Verifiable Credentials over the open API. Signed, revocable, auditable by anyone. (Choosing exactly what to disclose - selective disclosure - is in development.) |
| 🗳️ | **Help decide things** | Local server proposals or civilization-wide votes. Vote weight comes from your reputation, capped so no single person can dominate. |
| 🌱 | **Play and learn survival** | A full 3D homestead: farm real crops, cook, craft, fight, trade, and build a multi-deck ship under a night sky of real stars. |

Add the desktop app and the whole toolset - including the 3D world - runs offline with local saves. Chat, market, and voting pick back up when you reconnect.

---

## 🛡️ Three things that make HumanityOS different

### 1. Your identity is yours, forever

When you sign up, your phone or computer creates a **post-quantum cryptographic key** - math so strong it will still be secure when quantum computers arrive. No username, no password. Your 24-word backup phrase recovers everything if you lose your device. Forgot your phrase too? Guardian-based recovery (Shamir secret sharing - no single friend can do it alone) is being built: the server side is live, the in-app flow is in development.

### 2. Nobody can deplatform you

There's no lock-in to a central server. Anyone can run a copy. **Your identity works on any server you join**, your credentials follow you, your messages and contacts come with you. If one server goes down, you keep going. Server-to-server federation is built and fail-closed, ready to switch on as more servers appear - a network with no center can't be shut down from one.

### 3. Public domain - really

Every line of code, every design doc, every commit is in the public domain ([CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/)). Copy it, fork it, sell it, teach from it. **No attribution required.** Built by volunteers, owned by humanity.

---

## ✅ What's working right now

<table>
<tr>
<td valign="top" width="33%">

### Communication
- Text, voice, video chat
- End-to-end encrypted DMs
- Threaded replies & reactions
- Screen share & PiP video
- Pinned messages, mentions
- Group conversations
- Voice channels (always-on)
- File and image sharing
- Push notifications

</td>
<td valign="top" width="33%">

### Organize your life
- Kanban project boards
- Calendar & event planning
- Encrypted notes
- Skills & XP tracking
- Inventory tracker
- Maps (real + simulation)
- Marketplace listings
- Trade history & reviews
- Civilization dashboard

</td>
<td valign="top" width="33%">

### Trust & governance
- DID identity (`did:hum:`)
- Verifiable Credentials (API)
- Multi-layer trust score
- Vouching (API; UI coming)
- Local + civilization voting
- AI-as-citizen rules
- Server federation (built, fail-closed; activates as peers join)
- Anti-Sybil math built in

</td>
</tr>
</table>

### The 3D world (desktop app)
Farming, cooking, crafting, combat, quests, skills and XP, a player market with escrowed trades, livestock, weather, procedural planets, and a buildable multi-deck ship - all playable solo today with local saves. Shared-world co-presence over the relay is built and in live testing.

### What's still cooking
- Native mobile apps (web works on phones today)
- Multiplayer polish: shared-world co-presence is in live two-player testing
- Social key recovery client flow (server-side guardian substrate is live)
- One-click vouching and credential-issuing UI (the signed-object plumbing is live)
- Mesh radio support for off-grid use
- Real Solana transaction signing in the desktop app

---

## 🚀 Get started

<table>
<tr>
<td valign="top" width="33%">

### 👋 Just try it
1. Visit **[united-humanity.us/chat](https://united-humanity.us/chat)**
2. Pick a display name
3. Say hi in `#welcome`
4. Take the **[5-minute tour](https://united-humanity.us/onboarding)**

No signup. No email. No credit card.

</td>
<td valign="top" width="33%">

### 💻 Desktop app
1. Visit **[united-humanity.us/download](https://united-humanity.us/download)**
2. Pick your platform (Win/Mac/Linux)
3. Run the binary
4. Same identity as the web

Works **fully offline**. Native 3D world bundled.

</td>
<td valign="top" width="33%">

### 🏠 Run your own server
1. `git clone https://github.com/Shaostoul/Humanity.git`
2. `cargo build --release --features relay --no-default-features`
3. `./target/release/HumanityOS --headless`
4. nginx + systemd in front

Under 10 minutes from zero to live. **[Full guide →](docs/admin/SELF-HOSTING.md)**

</td>
</tr>
</table>

---

## 🔐 Security & privacy

| | |
|---|---|
| **Identity signing** | ML-DSA-65 (Dilithium3) - post-quantum, FIPS 204 |
| **Key exchange** | ML-KEM-768 (Kyber768) - post-quantum, FIPS 203 |
| **Symmetric encryption** | AES-256-GCM |
| **Password KDF** | PBKDF2-SHA-256 (600,000 iterations) for client vaults; Argon2id (memory-hard) for server-stored secrets |
| **Hashing** | BLAKE3 - fast and quantum-resistant |
| **Transport** | WebSocket over TLS 1.2+, HSTS, CSP + full security-header set |
| **Storage** | Encrypted vaults - server stores only ciphertext |
| **Logs** | No analytics, no tracking pixels; the relay database never stores IP addresses (IPs touch memory only for abuse rate-limiting) |
| **Privilege** | Non-root systemd service with hardened sandboxing |
| **Audit** | Ongoing cadence → [SECURITY-CADENCE.md](docs/SECURITY-CADENCE.md) (initial Feb 2026 audit archived at [docs/history/SECURITY_AUDIT_2026-02-12.md](docs/history/SECURITY_AUDIT_2026-02-12.md)) |

Solana wallet support is **optional** and decoupled from your identity. Using HumanityOS doesn't require any blockchain. If you opt in, the wallet is the Ed25519 keypair derived from your same 24-word seed (your post-quantum chat identity is a separate Dilithium3 key).

---

## 🤖 Transparent AI development

This project is built in the open by a two-person team: one human (Shaostoul) and one AI (Claude), working together daily. When workload calls for it, the AI dispatches short-lived helper agents for parallel tasks -- every one of them coordinated and reviewed through:

- **[Agent dashboard](https://united-humanity.us/agents)** - live status of every AI scope (active / passive / blocked, last audit, gaps)
- **[Agent registry](data/coordination/agent_registry.ron)** - who owns what; rules for claiming a scope
- **[Orchestrator state](data/coordination/orchestrator_state.json)** - running session journal that survives across chat sessions
- **[Multi-agent design doc](docs/design/multi-agent-development.md)** - how it all fits together

Every AI decision is documented. AI agents are **first-class citizens** with the same rules as humans (no extra authority), mandatory transparency, and humans always retain the right to refuse AI interaction.

→ Every line of AI work is visible in the [git history](https://github.com/Shaostoul/Humanity/commits/main).

---

## 🧠 How it works under the hood

<details>
<summary><strong>Click to expand technical details</strong></summary>

### Stack

| Layer | Technology |
|---|---|
| Server (relay) | Rust · axum · tokio · SQLite (WAL mode, Litestream-replicable) |
| Native client | Rust · wgpu · egui · hecs ECS · rapier3d physics · kira audio |
| Web client | Plain HTML/JS/CSS - **no build step** |
| Identity | ML-DSA-65 (Dilithium3) post-quantum signatures |
| Key exchange | ML-KEM-768 (Kyber768) post-quantum KEM |
| Object format | Canonical CBOR + BLAKE3 + signed substrate |
| Federation | WebSocket multi-hop gossip with cycle-breaking via dedup |
| Web realtime | WebSocket + WebRTC for voice/video/data channels |
| Hosting | nginx + systemd (optional Litestream replication to S3-compatible storage - see [docs/operations/litestream.md](docs/operations/litestream.md)) |

### Layout

```
Humanity/
├── src/                     ← Single Rust crate. Feature flags: native, relay, wasm.
│   ├── main.rs              ← --headless for relay-only, default for desktop
│   ├── relay/               ← Server (axum WebSocket + REST API + SQLite)
│   │   ├── core/            ← PQ crypto, signed objects, DIDs
│   │   ├── storage/         ← 40+ SQLite domain modules
│   │   ├── handlers/        ← Federation, message routing, announcements
│   │   └── api.rs + api_v2_*.rs ← REST endpoints (DID, VC, trust, governance, recovery, …)
│   ├── gui/                 ← egui native UI (theme, widgets, 40+ pages)
│   ├── renderer/            ← wgpu PBR + bloom + particles + hologram
│   ├── ecs/                 ← hecs World + System trait
│   ├── systems/             ← 40+ game systems (farming, combat, weather, economy, …)
│   ├── physics/             ← rapier3d wrapper
│   └── terrain/             ← Icosphere planets, voxel asteroids, ship interiors
├── web/                     ← Plain JS/HTML/CSS site (served by nginx)
│   ├── chat/                ← Chat client modules
│   ├── pages/               ← Standalone pages (36 of them)
│   └── shared/              ← shell.js, theme.css, pq-identity.js bridge
├── data/                    ← Hot-reloadable game + identity + coordination data
│   ├── chemistry/           ← 483 elements, compounds, alloys, gases, toxins
│   ├── items/foods/         ← Real-world items with ingredient tox profiles
│   ├── coordination/        ← Multi-AI agent registry + session state
│   ├── governance/          ← Proposal type schemas
│   └── identity/            ← VC schema registry + trust score weights
├── assets/                  ← Shaders, models, icons, audio
└── docs/                    ← All design documents and operations guides
```

### Architecture documents to read

- **[Storage architecture](docs/design/storage-architecture.md)** - 3-layer model (server / web / native), authority via signed objects, scaling story, P2P paths
- **[Identity](docs/design/identity.md)** - DID resolution, key rotation, signed profile replication
- **[UI system](docs/design/ui-system.md)** - Theme tokens, universal Button widget, design tokens
- **[Federation](docs/network/server_federation.md)** - Federation protocol, signed-object gossip, peer trust
- **[Humanity Accord](docs/accord/humanity_accord.md)** - Voluntary constitution every server may adopt
- **[Litestream replication](docs/operations/litestream.md)** - Disaster recovery for self-hosters

### Tests

```bash
cargo test --features relay --no-default-features --lib
# 700+ tests across storage, crypto, signing, federation, and game logic
cargo test --features native --lib
# 890+ tests for the full desktop app (GUI, systems, renderer helpers)
```

</details>

---

## 🌍 Federated server registry

The Humanity Accord is a voluntary set of principles every server may adopt. Servers that publicly adopt it earn the highest trust tier in federation. Reach out to [@Shaostoul](https://x.com/Shaostoul) to register.

→ [Read the Accord](docs/accord/humanity_accord.md)

---

## 🤝 Get involved

| | |
|---|---|
| 💬 **Show up** | [united-humanity.us/chat](https://united-humanity.us/chat) - no account needed |
| 💜 **Discord** | [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC) |
| 🐛 **Report bugs** | [united-humanity.us/bugs](https://united-humanity.us/bugs) or open a GitHub issue |
| 📖 **Contributing** | [CONTRIBUTING.md](CONTRIBUTING.md) - start here if you want to write code |
| 💸 **Donate** | [GitHub Sponsors](https://github.com/sponsors/Shaostoul) - every dollar goes to development & hosting |

**We need writers, designers, developers, educators, translators, testers - and just anyone who cares.** Show up in chat and ask what needs doing.

---

## 🔗 Find Michael (project lead)

🎥 [YouTube](https://youtube.com/@Shaostoul) · 📺 [Twitch](https://twitch.tv/Shaostoul) · 𝕏 [X / Twitter](https://x.com/Shaostoul) · 🎮 [Steam](https://steamcommunity.com/id/Shaostoul)

---

## 📜 License

[**CC0 1.0 Universal**](https://creativecommons.org/publicdomain/zero/1.0/) - public domain. No permission required, no attribution required. This belongs to everyone.

---

<sub>Built since 2019 (originally Project Universe). 7 years of work, hundreds of features, all free, all yours.</sub>
