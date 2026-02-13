# Humanity

**Wholesomely aiding humanity's betterment.**

An open-source cooperative project to end poverty through education and technology. Not charity — capability. Public domain. Built by volunteers. For everyone.

🌐 [united-humanity.us](https://united-humanity.us) · 💬 [Chat](https://united-humanity.us/chat) · 📦 [GitHub](https://github.com/Shaostoul/Humanity) · 💜 [Discord](https://discord.gg/9XxmmeQnWC)

---

## What's Live

### Humanity Hub
A tabbed interface with 11 sections — **Chat, Board, Reality, Fantasy, Market, Browse, Dashboard, Streams, Info, Source, Debug** — the foundation for everything we're building.

### Communication
- **Channels** — admin-created rooms with descriptions
- **E2E encrypted DMs** — ECDH P-256 key exchange + AES-256-GCM, server never sees plaintext
- **Threaded replies** — reply to any message with collapsible threads
- **Groups** — private group conversations (foundation)
- **Voice channels** — persistent, always-on WebRTC mesh rooms to join/leave
- **Voice & video calling** — WebRTC peer-to-peer 1-on-1 calls with audio and video
- **@mentions** with highlighting and notifications
- **Emoji reactions** (persistent, synced across sessions)
- **Message editing and deletion**
- **Message search** — full-text search across conversations
- **Image sharing** with lazy-loaded placeholders
- **Browser push notifications** and 6 notification sound options
- **Typing indicators** and unread markers

### Identity & Privacy
- **Ed25519 cryptographic identity** — keys stored in your browser, never on our server
- **Multi-device key linking** — use the same identity across devices
- **Device management** — list, label, and revoke linked keys
- **Key backup, export, and import** — own your identity completely
- **Encrypted user data sync** — settings, follows, and profile sync encrypted to server
- **No IP logging, no analytics, no tracking**
- **18+ only** by-entry confirmation — free speech platform

### Social
- **Follow/friend system** — mutual follow = friends, friends unlock DMs
- **Friend codes** — 8-character codes with 24-hour expiry, auto-mutual-follow
- **User profiles** with bio and social links
- **Unique pixel-art identicons** per user
- **Client-side user blocking**
- **Report system** with rate limiting
- **Pin system** — server pins (mod/admin) and personal pins (local)

### Hub Tools
- **Project board** — kanban-style task management
- **Marketplace** — peer-to-peer listings for goods and services, kiosks
- **Universal catalog** — 118 elements, 44 materials, processing chains
- **Browse tab** — web directory with 52 curated sites, Tranco ranks, RDAP domain info, uptime pings, collections, 4 sort modes
- **Dashboard tab** — 10 widget types with customizable drag-and-drop layout
- **Personal inventory** — track what you own
- **Notes** — private note-taking
- **Todos** — personal task lists
- **Garden tracker** — plan and track your garden

### Game & Creative
- **Fantasy tab** — character sheet, lore entries, world map, achievements
- **Streams tab** — local capture demo (real streaming coming soon)
- **Concept art** — multi-km spaceships, virtual malls, in-game spaces

### Platform
- **PWA installable** — works on mobile, add to homescreen
- **Desktop app** — [Tauri v2](https://tauri.app/) with auto-updater for Windows, macOS, Linux
- **Command palette** — quick access to everything
- **Settings panel** — accent colors, font sizes, theme customization
- **Auto-reload on deploy** — client updates instantly without manual refresh
- **Auto-login** — seamless reconnection with stored keys

### Moderation
- Role-based system: admin 👑, mod 🛡️, verified ✦, donor 💎
- Kick/ban with instant WebSocket disconnection
- Auto-lockdown when no mods online
- Invite codes for controlled access

### Federation
- **Phase 1** — server discovery, trust tiers, anyone can host a server
- **Phase 2** — cross-server identity and room directory
- Single binary, zero dependencies, under 10 minutes to set up
- Verified servers that adopt the [Humanity Accord](accord/humanity_accord.md) earn the highest trust tier

---

## Security

- Server-side Ed25519 signature verification on every message
- E2E encrypted DMs (ECDH P-256 + AES-256-GCM) — server never sees plaintext
- Encrypted user data sync — profile, settings, follows encrypted at rest
- Fibonacci rate limiting + new-account slow mode
- Content Security Policy (CSP), HSTS, TLS 1.2+ only
- No IP logging — we don't store what we don't need
- Per-session upload tokens with magic-byte validation
- HMAC-SHA256 webhook verification
- Non-root systemd service with hardened sandboxing

---

## Architecture

| Component | Technology |
|-----------|-----------|
| Server | Rust (axum + tokio) |
| Client | Single-file HTML/JS |
| Identity | Ed25519 (signing) + ECDH P-256 (encryption) |
| Storage | SQLite |
| Transport | WebSocket + WebRTC |
| Desktop | Tauri v2 |
| Layout | Cargo workspace |

The client is a single HTML file with no build step — open it in a browser and it works. The server is a Rust binary that handles WebSocket connections, message persistence, identity verification, and file uploads.

---

## What We're Building

### The Humanity Accord
Civilizational principles for cooperation at scale — across cultures, distances, and generations. A living, revisable framework.
→ [Read the Accord](accord/humanity_accord.md)

### The Humanity Network
Federated communication built on cryptographic identity. No central servers owning your data. Your identity is portable across all servers.
→ [Design specs](design/network/) · [Federation spec](design/network/server_federation.md)

### Project Universe
A free game teaching practical skills — homesteading, agriculture, building, health, survival. Learn to provide for yourself and your community.

---

## Desktop App

A native desktop app is available for Windows, macOS (ARM64 + x64), and Linux — built with [Tauri v2](https://tauri.app/). It wraps the web client in a native window with auto-updater, so you always get the latest version.

**[Download the latest release (v0.2.0) →](https://github.com/Shaostoul/Humanity/releases/latest)**

> **Windows users:** You may see a SmartScreen warning ("Unknown publisher"). This is normal for open-source software without a code signing certificate. Click "More info" → "Run anyway" to proceed.

To build from source, see [`desktop/README.md`](desktop/README.md).

---

## Host Your Own Server

Anyone can run a Humanity Network server. No permission needed.

1. Clone: `git clone https://github.com/Shaostoul/Humanity.git`
2. Build: `cargo build --release -p humanity-relay`
3. Run: `./target/release/humanity-relay`
4. Put it behind nginx with TLS (Let's Encrypt is free)
5. Share your URL — people connect with their existing keypair

→ **[Full self-hosting guide](SELF-HOSTING.md)** — production setup, nginx config, systemd, federation, admin commands

Want verified status? Contact [@Shaostoul on X](https://x.com/Shaostoul). Publicly adopt the [Humanity Accord](accord/humanity_accord.md) for highest trust tier.

→ [Federation & trust tiers](design/network/server_federation.md)

---

## Get Involved

- 💬 **Chat:** [united-humanity.us/chat](https://united-humanity.us/chat) — no account needed
- 💜 **Discord:** [discord.gg/9XxmmeQnWC](https://discord.gg/9XxmmeQnWC)
- 📦 **GitHub:** [github.com/Shaostoul/Humanity](https://github.com/Shaostoul/Humanity)
- 📖 **Docs:** [shaostoul.github.io/Humanity](https://shaostoul.github.io/Humanity)

**Contribute** — Writers, designers, developers, educators, translators. Check the issues or show up and ask what needs doing.

**Donate** — Servers and infrastructure cost money. Every dollar goes toward development and hosting.
→ [GitHub Sponsors](https://github.com/sponsors/Shaostoul) · [Ko-fi](https://ko-fi.com/shaostoul) · [Patreon](https://www.patreon.com/c/Shaostoul)

---

## Links

- 🎥 [YouTube](https://youtube.com/@Shaostoul) · 📺 [Twitch](https://twitch.tv/Shaostoul) · 🟢 [Rumble](https://rumble.com/user/Shaostoul)
- 𝕏 [X/Twitter](https://x.com/Shaostoul) · 📷 [Instagram](https://instagram.com/shaostoul) · 🔵 [Bluesky](https://bsky.app/profile/shaostoul.bsky.social)
- 🟠 [Reddit](https://reddit.com/user/Shaostoul) · 👤 [Facebook](https://www.facebook.com/2571477392923654) · 🎮 [Steam](https://steamcommunity.com/id/Shaostoul)

---

## License

This work is released into the **public domain** under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

No permission required. No attribution required. This belongs to humanity — present and future.
