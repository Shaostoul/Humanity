# Humanity — New Contributor Onboarding

Welcome. This document answers the questions that every newcomer has. Read it once, then keep [CONTRIBUTING.md](CONTRIBUTING.md) open as a reference.

---

## What Is This Project?

**Humanity** is an open-source platform with two interconnected layers:

### Layer 1: HumanityOS (the real-world platform)
A communication and life-management system you actually own. Think Discord + Notion + life-OS, but:
- No accounts — your identity is a cryptographic key stored in your browser
- No tracking, no ads, no central authority
- Federated — anyone can host a server; users keep their identity across all servers
- Public domain (CC0) — no permission required to use, fork, or deploy

What's live right now: chat channels, E2E encrypted DMs, voice/video calls, streaming, follow/friend system, project boards, marketplace, asset library, inventory tracking, skills, maps, calendar, dashboard, and more.

### Layer 2: Project Universe (the game)
A free game teaching practical skills — homesteading, agriculture, building, health, survival. The game uses the same data layer as the platform. Skills you develop in the game reflect real-world capability. The game is how people learn to use the tools for real.

### The Humanity Accord
A living document of civilizational principles — non-negotiable ethical foundations that all servers must adopt to earn verified status. Think of it as the constitution. Everything in this repo must conform to it.

---

## Why Does It Exist?

In 2017, Michael Boisson nearly died. That experience stripped away the noise and left one clear answer: help people become capable of helping themselves.

Poverty is not just lack of money — it's lack of capability. People trapped in systems they can't understand, knowledge they can't access, skills they never learned. The solution is education, tools, and community built at civilizational scale.

Everything here is public domain. This belongs to everyone, present and future.

---

## The State of the Project (March 2026)

The platform is **live and actively used** at [united-humanity.us](https://united-humanity.us).

What's solid and working:
- Chat, DMs, voice/video, streaming — the full communication layer
- Cryptographic identity and E2E encryption
- Federation Phase 1 (server discovery, trust tiers)
- 18 standalone page stubs (some fully built, some placeholders)

What's actively being built:
- The standalone pages (home, profile, skills, etc.) — most exist as stubs that need content
- Federation Phase 2 — cross-server identity and room directory
- The game layer — game mechanics exist in `web/activities/` and `src/` but need more development
- Better onboarding for new users (which is why this document exists)

What just happened (this session):
- `app.js` was a 6,400-line monolith — split into 8 focused modules
- `style.css` was a 2,760-line monolith — split into 7 component CSS files
- `relay.rs` and `storage.rs` (Rust server) split into focused domain modules
- `game/index.html` was a 15,216-line monolith — split into JS modules + JSON data files
- Nav system unified across all 18 pages, 4 bugs fixed

---

## The Codebase in 5 Minutes

### The server (Rust)

```
server/src/
├── main.rs       ← Start here. Axum router, startup, static file serving.
├── relay.rs      ← The heart. One big handle_connection() function that routes
│                    WebSocket messages by type. Add a new message type here.
├── handlers/     ← Pure functions extracted from relay.rs
│   ├── broadcast.rs    broadcast_peer_list, build_channel_list, etc.
│   ├── federation.rs   server-to-server connection logic
│   └── utils.rs        is_private_ip, fetch_link_preview, html_decode, etc.
├── storage/      ← SQLite — each file = one domain
│   ├── mod.rs          Storage struct, open(), schema migrations
│   ├── messages.rs     store/load messages
│   ├── channels.rs     channel CRUD
│   ├── dms.rs          direct messages
│   ├── social.rs       follow/friend/group system
│   ├── profile.rs      user profiles
│   ├── reactions.rs    emoji reactions
│   ├── pins.rs         pinned messages
│   ├── marketplace.rs  listings + friend codes
│   ├── streams.rs      stream records
│   ├── board.rs        project board tasks
│   ├── assets.rs       asset library
│   ├── skill_dna.rs    skill tracking
│   └── misc.rs         server state, sync, federation, voice channels, etc.
└── api.rs        ← HTTP REST API (bot access, webhooks, stats)
```

**Key concept:** The server is stateless per-connection. Identity is verified by Ed25519 signature on every message. There are no user accounts — only public keys.

### The chat client (JavaScript)

```
web/chat/
├── index.html          ← The shell. Loads scripts in order. Don't reorder them.
├── crypto.js           ← Ed25519 + ECDH + AES. The crypto foundation.
├── app.js              ← Core: WebSocket connection, message dispatch, state globals
│                          (ws, myKey, myName, activeChannel, peerData, etc.)
├── chat-messages.js    ← Emoji reactions, edit, pins, typing, image upload, threads
├── chat-dms.js         ← DM state, openDmConversation, conversation list
├── chat-social.js      ← Follow/friend system, isFriend(), groups
├── chat-ui.js          ← Notifications, sidebar, search, command palette, unread
├── chat-voice.js       ← Voice rooms, 1-on-1 calls, video panel, right sidebar
├── chat-profile.js     ← Profile edit/view modals, avatar/banner
└── chat-p2p.js         ← Signed contact cards, WebRTC DataChannel
```

**Key concept:** No build step. All modules share global scope. `app.js` globals (`ws`, `myKey`, `handleMessage`, etc.) are accessible from every other module. To extend behavior without touching `app.js`, use the monkey-patch pattern:

```js
const _orig = handleMessage;
handleMessage = function(msg) {
  if (msg.type === 'new_type') { /* handle it */ return; }
  _orig(msg);
};
```

### The standalone pages

18 HTML files in the repo root. Each one:
- Loads `shell.js` with `data-active="key"` to highlight its nav button
- Has its own page content and any page-specific scripts
- Is served by nginx from `/var/www/humanity/`

Most pages are stubs that need content — this is where non-developer contributors can make a big impact.

### The nav system (`shared/shell.js`)

Shell.js injects the sticky nav bar on every page. The nav is defined in one place — one `navTab()` call per page. To add a page to the nav, you add one line here. Auto-detection maps URL paths to active keys, so if you link to `/yourpage` it will highlight the right button automatically.

---

## The Identity System

Understanding this unlocks the whole platform.

Every user has an **Ed25519 keypair** generated in their browser:
- **Private key** — never leaves the device, stored in IndexedDB
- **Public key** — their identity; also their "user ID" (displayed as a short hex string)

Every message is signed with the private key. The server verifies the signature before accepting the message. This means:
- No passwords, no accounts
- The server cannot impersonate users
- Users own their identity completely

For encrypted DMs, a second **ECDH P-256 keypair** handles key exchange. The actual message encryption uses AES-256-GCM. The server never sees DM content.

This is implemented in `web/chat/crypto.js` and verified in `server/src/relay.rs`.

---

## The Message Protocol

Messages between client and server are JSON over WebSocket. Every outbound message has the shape:

```json
{
  "type": "message",
  "from": "<public_key_hex>",
  "name": "Alice",
  "body": "Hello world",
  "channel": "general",
  "timestamp": 1741840000000,
  "sig": "<ed25519_signature_hex>"
}
```

The server routes by `type`. To add a new feature, you add a new message type, handle it in `relay.rs`'s match statement, and handle the response in `handleMessage()` on the client.

---

## The Accord (Read Before Proposing Changes)

The [Humanity Accord](accord/humanity_accord.md) defines what this project must never do. It's short. Read it.

Non-negotiable prohibitions include anything involving sexual violence, child exploitation, slavery, political coercion, and a handful of others. Every server that joins the network must adopt it to reach verified status.

The Accord isn't ideology — it's a minimal floor that allows people from radically different backgrounds to cooperate.

---

## Your First Contribution

Pick the thing that matches your skills:

**For developers:**
- Find a `TODO` comment: `grep -r "TODO\|FIXME" --include="*.js" --include="*.rs" .`
- Pick a GitHub issue labeled `good first issue`
- Look at any of the 18 standalone pages — most need their full feature implementation
- Run the server locally, open the chat, and fix something that annoys you

**For designers/UI:**
- The standalone pages (`home.html`, `profile.html`, `skills.html`, etc.) need proper UI
- Look at the CSS files — 7 component files, one per concern
- Mobile responsiveness is a constant need

**For writers:**
- The `docs/design/` directory has architecture docs that need updating
- Every page could use better help text and tooltips
- The Knowledge page (`knowledge.html`) needs actual content

**For everyone:**
- Join the chat: [united-humanity.us/chat](https://united-humanity.us/chat)
- Ask what needs doing — active contributors know the current priorities
- File issues for bugs you find

---

## How Decisions Are Made

This is not a democracy, but it's not a dictatorship either. The project has a clear authority hierarchy:

1. **The Accord** — inviolable principles. Nobody overrides this.
2. **Design specs** (`design/`) — architectural decisions. Changes need justification.
3. **Michael (Shaostoul)** — project lead and final decision-maker for direction.
4. **Active contributors** — people who show up and do the work shape the project.

The best way to get your idea implemented is to implement it and submit a PR. Good work speaks for itself.

---

## Getting Help

- **Chat** — [united-humanity.us/chat](https://united-humanity.us/chat) — real-time, best for quick questions
- **Discord** — [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC) — more structured discussion
- **GitHub Issues** — for bugs and feature proposals
- **This repo** — most architecture questions are answered in `docs/design/`

Don't overthink it. Show up, ask questions, start somewhere small. The codebase is large but the concepts are straightforward once you see the patterns.
